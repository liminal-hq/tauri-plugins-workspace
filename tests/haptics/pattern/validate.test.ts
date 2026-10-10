// Unit tests for pattern validation messages
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import { describe, it, expect } from 'vitest';
import {
	formatIssues,
	isPattern,
	validatePattern,
} from '../../../plugins/haptics/guest-js/pattern/validate';
import { PATTERN_FORMAT } from '../../../plugins/haptics/guest-js/pattern/types';

const ok = {
	format: PATTERN_FORMAT,
	events: [
		{ type: 'transient', at: 0, intensity: 0.8, sharpness: 0.9 },
		{ type: 'continuous', at: 100, duration: 200, intensity: 0.5, sharpness: 0.4 },
	],
};

const withEvent = (event: object) => ({ ...ok, events: [event] });
const messages = (input: unknown, opts = {}) =>
	validatePattern(input, opts).map((i) => `${i.path}: ${i.message}`);

describe('validatePattern', () => {
	it('accepts a valid pattern', () => {
		expect(validatePattern(ok)).toEqual([]);
		expect(isPattern(ok)).toBe(true);
	});

	it('rejects a value that is not an object', () => {
		expect(validatePattern(null)).toHaveLength(1);
		expect(validatePattern([])).toHaveLength(1);
		expect(isPattern('click')).toBe(false);
	});

	it('names the format it expects', () => {
		expect(messages({ ...ok, format: 'other' })).toEqual([
			'format: other is not a known format. Use "haptics-lab/pattern@1".',
		]);
	});

	it('reports an intensity above 1 with its path and the fix', () => {
		const event = { type: 'transient', at: 0, intensity: 1.4, sharpness: 0.5 };
		expect(messages(withEvent(event))).toEqual(['events[0].intensity: 1.4 is above 1. Use 0..1.']);
	});

	it('reports a negative sharpness', () => {
		const event = { type: 'transient', at: 0, intensity: 0.5, sharpness: -0.2 };
		expect(messages(withEvent(event))).toEqual(['events[0].sharpness: -0.2 is below 0. Use 0..1.']);
	});

	it('requires continuous events to last at least 20 ms', () => {
		const event = { type: 'continuous', at: 0, duration: 0, intensity: 0.5, sharpness: 0.5 };
		expect(messages(withEvent(event))).toEqual([
			'events[0].duration: 0 ms. Continuous events need at least 20 ms.',
		]);
	});

	it('reports every problem at once', () => {
		const events = [
			{ type: 'transient', at: -5, intensity: 2, sharpness: 'x' },
			{ type: 'blip', at: 0 },
		];
		const found = messages({ ...ok, events });
		expect(found).toHaveLength(4);
		expect(found[0]).toContain('events[0].at');
		expect(found[3]).toContain('events[1].type');
	});

	it('rejects an empty or missing event list', () => {
		expect(messages({ ...ok, events: [] })).toEqual(['events: No events. Add at least one.']);
		expect(messages({ ...ok, events: 3 })).toEqual(['events: Events must be a list.']);
	});

	it('checks curves: at least 2 points, ascending t, from 0 to 1', () => {
		const curve = (intensity: unknown) =>
			withEvent({ type: 'continuous', at: 0, duration: 100, intensity, sharpness: 0.5 });

		expect(messages(curve([{ t: 0, v: 0.2 }]))).toEqual([
			'events[0].intensity: 1 point. A curve needs at least 2.',
		]);
		expect(
			messages(
				curve([
					{ t: 0, v: 0 },
					{ t: 0.5, v: 1 },
					{ t: 0.4, v: 0.5 },
					{ t: 1, v: 0 },
				])
			)
		).toEqual(['events[0].intensity[2].t: 0.4 does not come after 0.5. Keep t ascending.']);
		expect(
			messages(
				curve([
					{ t: 0.1, v: 0 },
					{ t: 0.9, v: 1 },
				])
			)
		).toEqual([
			'events[0].intensity[0].t: 0.1 is not 0. A curve starts at t = 0.',
			'events[0].intensity[1].t: 0.9 is not 1. A curve ends at t = 1.',
		]);
		expect(
			messages(
				curve([
					{ t: 0, v: 0 },
					{ t: 1, v: 1.5 },
				])
			)
		).toEqual(['events[0].intensity[1].v: 1.5 is above 1. Use 0..1.']);
	});

	it('accepts valid curves for intensity and sharpness', () => {
		const pts = [
			{ t: 0, v: 0 },
			{ t: 1, v: 1 },
		];
		const event = { type: 'continuous', at: 0, duration: 100, intensity: pts, sharpness: pts };
		expect(validatePattern(withEvent(event))).toEqual([]);
	});

	it('gives a transient room to play before the limit', () => {
		const transient = (at: number) => ({
			type: 'transient',
			at,
			intensity: 0.5,
			sharpness: 0.5,
		});
		expect(messages(withEvent(transient(99)), { maxDurationMs: 100 })).toEqual([]);
		expect(messages(withEvent(transient(100)), { maxDurationMs: 100 })).toEqual([
			'events: The pattern runs for 101 ms. The limit is 100 ms; shorten or move events earlier.',
		]);
		expect(messages(withEvent(transient(99.6)), { maxDurationMs: 100 })).toHaveLength(1);
	});

	it('enforces the total length against the configured limit', () => {
		const event = { type: 'continuous', at: 9_900, duration: 500, intensity: 1, sharpness: 1 };
		expect(messages(withEvent(event), { maxDurationMs: 10_000 })).toEqual([
			'events: The pattern runs for 10400 ms. The limit is 10000 ms; shorten or move events earlier.',
		]);
		expect(validatePattern(withEvent(event))).toEqual([]);
	});

	it('validates usage and policy', () => {
		expect(messages({ ...ok, usage: 'loud' })).toEqual([
			'usage: loud is not a usage. Use touch, notification, alarm, media.',
		]);
		expect(validatePattern({ ...ok, policy: { coalesce: 40 } })).toEqual([]);
		expect(validatePattern({ ...ok, policy: 'queue' })).toEqual([]);
		expect(messages({ ...ok, policy: 'spam' })[0]).toContain('policy: spam is not a policy');
		expect(messages({ ...ok, policy: { coalesce: 0 } })[0]).toContain('policy: Not a policy');
		expect(messages({ ...ok, policy: { coalesce: 5000 } })[0]).toContain('from 1 to 1000');
	});

	it('formats issues one per line', () => {
		const issues = validatePattern({ ...ok, usage: 'loud', events: [] });
		expect(formatIssues(issues).split('\n')).toHaveLength(2);
	});
});
