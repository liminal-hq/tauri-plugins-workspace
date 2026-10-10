// Public haptics API: capabilities, raw playback, the UI lane and the pattern registry
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import { invoke } from '@tauri-apps/api/core';
import { compilePattern } from './pattern/compile';
import type { CompileReport } from './pattern/compile';
import { PatternScheduler } from './pattern/schedule';
import type { Decision } from './pattern/schedule';
import { formatIssues, validatePattern } from './pattern/validate';
import type { Pattern, RegisterOptions, TriggerOptions } from './pattern/types';
import type { Capabilities, CompiledStep, EffectRequest, PlayResult, Tier, UiKind } from './types';

export * from './types';
export * from './pattern/types';
export { compilePattern } from './pattern/compile';
export type { CompileOptions, CompileReport, CompiledSegment } from './pattern/compile';
export { PatternScheduler } from './pattern/schedule';
export { formatIssues, isPattern, validatePattern } from './pattern/validate';
export type { ValidationIssue } from './pattern/validate';
export {
	AMPLITUDE_CEILING,
	NEIGHBOURS,
	PRIMITIVE_IDS,
	PRIMITIVE_MS,
	pickPrimitive,
	resolvePrimitive,
} from './pattern/tables';

/** A pattern together with the options it was registered with. */
export type PatternEntry = { pattern: Pattern; options?: RegisterOptions };

/** Invalid input rejects with one of these; hardware limits never do. */
export class HapticsError extends Error {
	constructor(
		readonly code: 'INVALID_EFFECT' | 'UNKNOWN_PATTERN',
		message: string
	) {
		super(message);
		this.name = 'HapticsError';
	}
}

// ── state ─────────────────────────────────────────────────────────────────────────────────────

let cached: Promise<Capabilities> | null = null;
let loaded: Capabilities | null = null;
let masterScale = 1;
let maxTier: Tier | null = null;
let stopCount = 0; // moves on with every stop(), so a trigger waiting on capabilities can tell
const patterns = new Map<string, PatternEntry>();
const scheduler = new PatternScheduler();

const clamp01 = (v: number) => Math.min(1, Math.max(0, v));

/** A 0..1 option, with a missing or non-finite value counting as full strength. */
const unit = (v: number | undefined): number => (Number.isFinite(v) ? clamp01(v as number) : 1);

function load(): Promise<Capabilities> {
	const next = invoke<Capabilities>('plugin:haptics|capabilities').then(
		(caps) => {
			if (cached === next) loaded = caps;
			return caps;
		},
		(err) => {
			if (cached === next) cached = null;
			throw err;
		}
	);
	cached = next;
	return next;
}

// The touch-feedback setting can change while the app is away, so read it again on return.
if (typeof document !== 'undefined') {
	document.addEventListener('visibilitychange', () => {
		if (document.visibilityState === 'visible' && cached) void load().catch(() => undefined);
	});
}

// ── capabilities ──────────────────────────────────────────────────────────────────────────────

/** Reads the device capabilities once and caches them; `refresh` reads them again. */
export function capabilities(opts?: { refresh?: boolean }): Promise<Capabilities> {
	if (opts?.refresh || !cached) return load();
	return cached;
}

// ── global controls ───────────────────────────────────────────────────────────────────────────

/** Multiplies every intensity, 0..1. Applies to patterns and raw `play()`. */
export function setMasterScale(v: number): void {
	masterScale = clamp01(Number.isFinite(v) ? v : 1);
}

/** Caps the tier for testing and previews; `null` uses the device's top tier. */
export function setMaxTier(t: Tier | null): void {
	maxTier = t;
}

/** Cancels the motor and clears every queue, pending merge and timer. */
export async function stop(): Promise<void> {
	stopCount++;
	scheduler.stop();
	await invoke('plugin:haptics|stop');
}

// ── patterns ──────────────────────────────────────────────────────────────────────────────────

function assertValid(pattern: unknown, caps: Capabilities): asserts pattern is Pattern {
	const issues = validatePattern(pattern, { maxDurationMs: caps.limits.maxDurationMs });
	if (issues.length) throw new HapticsError('INVALID_EFFECT', formatIssues(issues));
}

function requireCaps(): Capabilities {
	if (!loaded) {
		throw new Error('Capabilities are not loaded yet. Await capabilities() or register() first.');
	}
	return loaded;
}

/**
 * Compiles a pattern for this device without playing it. Synchronous and pure, so the lab can
 * preview every tier. Needs the capabilities to have been loaded.
 */
export function compile(pattern: Pattern, opts?: { tier?: Tier }): CompileReport {
	const caps = requireCaps();
	assertValid(pattern, caps);
	return compilePattern(pattern, caps, { tier: opts?.tier, maxTier, scale: masterScale });
}

/** Validates a pattern, remembers it under `id` and reports how it compiles on this device. */
export async function register(
	id: string,
	pattern: Pattern,
	opts?: RegisterOptions
): Promise<CompileReport> {
	const caps = await capabilities();
	assertValid(pattern, caps);
	// Copy it, so editing the caller's object later can't change what is registered.
	const own: Pattern = { ...structuredClone(pattern), id };
	patterns.set(id, { pattern: own, options: opts });
	return compilePattern(own, caps, {
		tier: opts?.tier,
		maxTier,
		scale: masterScale,
	});
}

export async function registerAll(
	table: Record<string, Pattern | PatternEntry>
): Promise<Record<string, CompileReport>> {
	const reports: Record<string, CompileReport> = {};
	for (const [id, value] of Object.entries(table)) {
		const entry = 'pattern' in value ? value : { pattern: value };
		reports[id] = await register(id, entry.pattern, entry.options);
	}
	return reports;
}

export function unregister(id: string): void {
	patterns.delete(id);
	scheduler.cancel(id);
}

const REASON_NOTES = /missing on this motor|drops to tier 2|over the|Capped|Truncated|No envelope/;

function silent(reason: string, decision?: Decision, tier: Tier = 0): PlayResult {
	return {
		ok: true,
		tier,
		target: 'phone',
		estimatedMs: 0,
		downgraded: reason !== '',
		...(reason ? { reason, downgradeReason: reason } : {}),
		...(decision ? { policy: decision } : {}),
	};
}

async function playCompiled(report: CompileReport): Promise<PlayResult> {
	if (report.steps.length === 0) {
		return silent(report.notes[0] ?? 'Nothing to play', undefined, report.tier);
	}
	// Compiled output already carries the master scale and the tier cap, so it skips `play()`.
	if (report.request && report.steps.length === 1) return sendPlay(report.request);
	return sendSteps(report.steps);
}

/** Plays a registered pattern by id, following its policy. */
export async function trigger(id: string, opts: TriggerOptions = {}): Promise<PlayResult> {
	const entry = patterns.get(id);
	if (!entry) throw new HapticsError('UNKNOWN_PATTERN', `No pattern is registered as "${id}".`);

	const stoppedAt = stopCount;
	const caps = await capabilities();
	if (stopCount !== stoppedAt) return silent('Stopped before it played', 'dropped');
	const tier = Math.min(entry.options?.tier ?? 4, opts.tier ?? 4) as Tier;
	const compileFor = (scale: number): CompileReport =>
		compilePattern(entry.pattern, caps, {
			tier,
			maxTier,
			scale,
			usage: opts.usage,
			respectSystemSettings: opts.respectSystemSettings,
		});

	const triggerScale = unit(opts.scale);
	if (masterScale * triggerScale === 0) return silent('Scale is 0, so nothing plays');
	const first = compileFor(masterScale * triggerScale);
	const outcome = await scheduler.submit<PlayResult>({
		key: id,
		policy: entry.pattern.policy ?? 'interrupt',
		estimatedMs: first.estimatedMs,
		scale: triggerScale,
		run: (scale) => {
			// A merged group arrives with a higher scale, so compile again at that strength.
			const report = scale === triggerScale ? first : compileFor(masterScale * scale);
			return playCompiled(report).then((res) => withNotes(res, report, caps));
		},
	});

	if (outcome.result) return { ...outcome.result, policy: outcome.policy };
	return silent('', outcome.policy, first.tier);
}

function withNotes(res: PlayResult, report: CompileReport, caps: Capabilities): PlayResult {
	const reasons = res.reason ? res.reason.split(' · ') : [];
	if (report.tier < caps.topTier) {
		reasons.push(
			maxTier !== null && report.tier === maxTier
				? `Capped at tier ${maxTier} by setMaxTier`
				: `Compiled at tier ${report.tier}`
		);
	}
	for (const note of report.notes) if (REASON_NOTES.test(note)) reasons.push(note);
	const unique = [...new Set(reasons)];
	if (!unique.length) return res;
	const reason = unique.join(' · ');
	return { ...res, downgraded: true, reason, downgradeReason: reason };
}

// ── raw ───────────────────────────────────────────────────────────────────────────────────────

function effectTier(req: EffectRequest, caps: Capabilities): Tier {
	switch (req.effect.type) {
		case 'envelopeWaveform':
			return 4;
		case 'composition':
			return 3;
		case 'predefined':
			return Math.min(caps.topTier, 3) as Tier;
		default:
			return caps.hasAmplitudeControl ? 2 : 1;
	}
}

/** Applies the master scale to a raw request's amplitude fields. */
function scaled(req: EffectRequest): EffectRequest {
	if (masterScale === 1) return req;
	const e = req.effect;
	switch (e.type) {
		case 'oneshot': {
			// An amplitude outside 1..255 is left for the native check to reject, not scaled into range.
			const given = e.amplitude;
			if (given !== undefined && !(Number.isInteger(given) && given >= 1 && given <= 255))
				return req;
			return {
				...req,
				effect: { ...e, amplitude: Math.max(1, Math.round((given ?? 255) * masterScale)) },
			};
		}
		case 'waveform':
			if (!e.amplitudes) return req;
			return {
				...req,
				effect: { ...e, amplitudes: e.amplitudes.map((a) => Math.round(a * masterScale)) },
			};
		case 'composition':
			return {
				...req,
				effect: {
					...e,
					steps: e.steps.map((s) => ({ ...s, scale: clamp01((s.scale ?? 1) * masterScale) })),
				},
			};
		case 'envelopeWaveform':
			return {
				...req,
				effect: {
					...e,
					controlPoints: e.controlPoints.map((p) => ({
						...p,
						amplitude: clamp01(p.amplitude * masterScale),
					})),
				},
			};
		default:
			return req;
	}
}

/**
 * The raw escape hatch: no compiler, no policies. It still applies the plugin limits, the master
 * scale and the `setMaxTier` cap, which resolves a request above the cap at tier 0.
 */
export async function play(req: EffectRequest): Promise<PlayResult> {
	if (masterScale === 0) return silent('Master scale is 0, so nothing plays');
	if (maxTier !== null) {
		const caps = await capabilities();
		if (effectTier(req, caps) > maxTier) {
			return silent(`Capped at tier ${maxTier} by setMaxTier`);
		}
	}
	return sendPlay(scaled(req));
}

/**
 * Hardware limits never make a play call reject, so a rejection means the native side refused the
 * input. Native errors arrive as plain strings; wrap them so callers always get `{ code, message }`.
 */
async function invalidInput<T>(call: Promise<T>): Promise<T> {
	try {
		return await call;
	} catch (err) {
		if (err instanceof HapticsError) throw err;
		throw new HapticsError('INVALID_EFFECT', err instanceof Error ? err.message : String(err));
	}
}

function sendPlay(req: EffectRequest): Promise<PlayResult> {
	return invalidInput(invoke<PlayResult>('plugin:haptics|play', { req }));
}

function sendSteps(steps: CompiledStep[]): Promise<PlayResult> {
	return invalidInput(invoke<PlayResult>('plugin:haptics|play_steps', { steps }));
}

/** Plays `{ atMs, request }` steps scheduled natively from one start time. */
export async function playSteps(steps: CompiledStep[]): Promise<PlayResult> {
	if (masterScale === 0) return silent('Master scale is 0, so nothing plays');
	if (maxTier !== null) {
		const caps = await capabilities();
		if (steps.some((s) => effectTier(s.request, caps) > maxTier!)) {
			return silent(`Capped at tier ${maxTier} by setMaxTier`);
		}
	}
	return sendSteps(steps.map((s) => ({ ...s, request: scaled(s.request) })));
}

// ── UI lane ───────────────────────────────────────────────────────────────────────────────────

/** System-style feedback that follows the touch-feedback setting. Not affected by the controls above. */
export function ui(kind: UiKind): Promise<PlayResult> {
	return invalidInput(invoke<PlayResult>('plugin:haptics|ui', { kind }));
}

export type { Decision };
