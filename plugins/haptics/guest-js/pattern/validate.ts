// Pattern validation that reports every problem at once, each with a path and a fix
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import { MIN_CONTINUOUS_MS, MIN_TRANSIENT_MS, PATTERN_FORMAT } from './types';
import type { Pattern } from './types';

export type ValidationIssue = {
	path: string; // for example `events[2].intensity`
	message: string; // a full sentence that says what to do
};

export type ValidateOptions = {
	/** Longest allowed pattern, in ms, measured to the end of the last event. */
	maxDurationMs?: number;
};

const USAGES = ['touch', 'notification', 'alarm', 'media'];
const POLICIES = ['interrupt', 'queue', 'drop-if-busy'];

/** Longest coalesce window; far above this a timer would be a bug, not a window. */
const MAX_COALESCE_MS = 1000;

function isRecord(v: unknown): v is Record<string, unknown> {
	return typeof v === 'object' && v !== null && !Array.isArray(v);
}

function isNumber(v: unknown): v is number {
	return typeof v === 'number' && Number.isFinite(v);
}

function show(v: unknown): string {
	return typeof v === 'number' || typeof v === 'string' ? String(v) : typeof v;
}

/** Checks a number that must sit in 0..1. */
function checkUnit(value: unknown, path: string, issues: ValidationIssue[]): void {
	if (!isNumber(value)) {
		issues.push({ path, message: `${show(value)} is not a number. Use 0..1.` });
	} else if (value > 1) {
		issues.push({ path, message: `${value} is above 1. Use 0..1.` });
	} else if (value < 0) {
		issues.push({ path, message: `${value} is below 0. Use 0..1.` });
	}
}

function checkCurve(points: unknown[], path: string, issues: ValidationIssue[]): void {
	if (points.length < 2) {
		issues.push({
			path,
			message: `${points.length} point${points.length === 1 ? '' : 's'}. A curve needs at least 2.`,
		});
		return;
	}
	let previous = -Infinity;
	points.forEach((point, i) => {
		const here = `${path}[${i}]`;
		if (!isRecord(point)) {
			issues.push({ path: here, message: 'Not a point. Use { t, v } with both in 0..1.' });
			return;
		}
		checkUnit(point.v, `${here}.v`, issues);
		if (!isNumber(point.t)) {
			issues.push({ path: `${here}.t`, message: `${show(point.t)} is not a number. Use 0..1.` });
			return;
		}
		if (point.t < 0 || point.t > 1) {
			issues.push({ path: `${here}.t`, message: `${point.t} is outside 0..1. Use 0..1.` });
		}
		if (point.t <= previous) {
			issues.push({
				path: `${here}.t`,
				message: `${point.t} does not come after ${previous}. Keep t ascending.`,
			});
		}
		previous = point.t;
	});
	const first = points[0];
	const last = points[points.length - 1];
	if (isRecord(first) && isNumber(first.t) && first.t !== 0) {
		issues.push({ path: `${path}[0].t`, message: `${first.t} is not 0. A curve starts at t = 0.` });
	}
	if (isRecord(last) && isNumber(last.t) && last.t !== 1) {
		issues.push({
			path: `${path}[${points.length - 1}].t`,
			message: `${last.t} is not 1. A curve ends at t = 1.`,
		});
	}
}

function checkLevel(value: unknown, path: string, issues: ValidationIssue[]): void {
	if (Array.isArray(value)) checkCurve(value, path, issues);
	else checkUnit(value, path, issues);
}

/**
 * Returns every problem with `input`, or an empty list when it is a valid pattern. Never throws.
 */
export function validatePattern(input: unknown, opts: ValidateOptions = {}): ValidationIssue[] {
	const issues: ValidationIssue[] = [];

	if (!isRecord(input)) {
		return [{ path: '', message: 'Not a pattern. Use an object with a format and events.' }];
	}

	if (input.format !== PATTERN_FORMAT) {
		issues.push({
			path: 'format',
			message: `${show(input.format)} is not a known format. Use "${PATTERN_FORMAT}".`,
		});
	}
	if (input.id !== undefined && (typeof input.id !== 'string' || input.id.length === 0)) {
		issues.push({ path: 'id', message: 'The id must be a non-empty string.' });
	}
	if (input.usage !== undefined && !USAGES.includes(input.usage as string)) {
		issues.push({
			path: 'usage',
			message: `${show(input.usage)} is not a usage. Use ${USAGES.join(', ')}.`,
		});
	}
	checkPolicy(input.policy, issues);

	const events = input.events;
	if (!Array.isArray(events)) {
		issues.push({ path: 'events', message: 'Events must be a list.' });
		return issues;
	}
	if (events.length === 0) {
		issues.push({ path: 'events', message: 'No events. Add at least one.' });
		return issues;
	}

	let end = 0;
	events.forEach((event, i) => {
		const at = `events[${i}]`;
		if (!isRecord(event)) {
			issues.push({ path: at, message: 'Not an event. Use a transient or a continuous event.' });
			return;
		}
		if (!isNumber(event.at)) {
			issues.push({
				path: `${at}.at`,
				message: `${show(event.at)} is not a number. Use ms from 0.`,
			});
		} else if (event.at < 0) {
			issues.push({
				path: `${at}.at`,
				message: `${event.at} ms is before the start. Use 0 or more.`,
			});
		}
		const start = isNumber(event.at) && event.at >= 0 ? event.at : 0;

		if (event.type === 'transient') {
			checkUnit(event.intensity, `${at}.intensity`, issues);
			checkUnit(event.sharpness, `${at}.sharpness`, issues);
			end = Math.max(end, start + MIN_TRANSIENT_MS);
		} else if (event.type === 'continuous') {
			if (!isNumber(event.duration)) {
				issues.push({
					path: `${at}.duration`,
					message: `${show(event.duration)} is not a number. Use ms, at least ${MIN_CONTINUOUS_MS}.`,
				});
			} else if (event.duration < MIN_CONTINUOUS_MS) {
				issues.push({
					path: `${at}.duration`,
					message: `${event.duration} ms. Continuous events need at least ${MIN_CONTINUOUS_MS} ms.`,
				});
			} else {
				end = Math.max(end, start + event.duration);
			}
			checkLevel(event.intensity, `${at}.intensity`, issues);
			checkLevel(event.sharpness, `${at}.sharpness`, issues);
		} else {
			issues.push({
				path: `${at}.type`,
				message: `${show(event.type)} is not an event type. Use "transient" or "continuous".`,
			});
		}
	});

	if (opts.maxDurationMs !== undefined && end > opts.maxDurationMs) {
		issues.push({
			path: 'events',
			message: `The pattern runs for ${end} ms. The limit is ${opts.maxDurationMs} ms; shorten or move events earlier.`,
		});
	}

	return issues;
}

function checkPolicy(policy: unknown, issues: ValidationIssue[]): void {
	if (policy === undefined) return;
	if (typeof policy === 'string') {
		if (!POLICIES.includes(policy)) {
			issues.push({
				path: 'policy',
				message: `${policy} is not a policy. Use ${POLICIES.join(', ')} or { coalesce: ms }.`,
			});
		}
		return;
	}
	if (
		isRecord(policy) &&
		isNumber(policy.coalesce) &&
		policy.coalesce > 0 &&
		policy.coalesce <= MAX_COALESCE_MS
	) {
		return;
	}
	issues.push({
		path: 'policy',
		message: `Not a policy. Use interrupt, queue, drop-if-busy or { coalesce: ms } with ms from 1 to ${MAX_COALESCE_MS}.`,
	});
}

/** Type guard: true when `input` has no validation issues. */
export function isPattern(input: unknown, opts: ValidateOptions = {}): input is Pattern {
	return validatePattern(input, opts).length === 0;
}

/** One line per issue, for error messages and logs. */
export function formatIssues(issues: ValidationIssue[]): string {
	return issues.map((i) => (i.path ? `${i.path}: ${i.message}` : i.message)).join('\n');
}
