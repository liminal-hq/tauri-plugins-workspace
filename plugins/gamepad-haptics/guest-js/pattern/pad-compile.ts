// Compiles a haptic pattern into motor frames for a pad's tier
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

// Pure TypeScript, like the rest of this folder. The Rust rules in `src/validate.rs` are the source
// of truth for what a frame list may hold; this compiler emits inside them.

import { MIN_CONTINUOUS_MS } from './types';
import type { ContinuousEvent, CurvePoint, Pattern, PatternEvent, Tier } from './types';

/** One step of motor levels, 0 to 1, held for `durationMs`. */
export type MotorFrame = {
	durationMs: number;
	heavy: number;
	light: number;
	leftTrigger?: number;
	rightTrigger?: number;
};

/** What a pad can play and how long a request may run. */
export type PadCaps = {
	topTier: Tier;
	maxDurationMs: number;
};

export type PadCompileOptions = {
	/** Compile for a lower tier than the pad's own. */
	maxTier?: Tier;
};

export type PadCompileReport = {
	frames: MotorFrame[];
	/** The tier the frames need: 3 with triggers, 2 when both motors are used, 1 otherwise, 0 when empty. */
	tier: Tier;
	/** Where the frames end, in ms. */
	estimatedMs: number;
	notes: string[];
};

export const MAX_FRAMES = 512;

/** Time between samples. Longer patterns widen it so they stay within `MAX_FRAMES`. */
export const SAMPLE_MS = 10;

/** A transient is stretched to this long, so a motor has time to spin up. */
export const MIN_TAP_MS = 40;

/** Levels that differ by less than this are the same frame. */
const SAME_LEVEL = 0.02;

const clamp01 = (v: number) => Math.min(1, Math.max(0, v));

/** The value of a constant or a `{t, v}` curve at `t`, 0..1 across the event. */
export function curveAt(curve: number | CurvePoint[], t: number): number {
	if (typeof curve === 'number') return clamp01(curve);
	if (curve.length === 0) return 0;
	const points = [...curve].sort((a, b) => a.t - b.t);
	if (t <= points[0].t) return clamp01(points[0].v);
	for (let i = 1; i < points.length; i++) {
		const a = points[i - 1];
		const b = points[i];
		if (t <= b.t) {
			const span = b.t - a.t;
			return clamp01(span === 0 ? b.v : a.v + ((b.v - a.v) * (t - a.t)) / span);
		}
	}
	return clamp01(points[points.length - 1].v);
}

/**
 * How an intensity and a sharpness split across the two body motors. Sharp is crisp and goes to the
 * light motor, dull is deep and goes to the heavy motor; the 1.25 keeps a mid sharpness from
 * halving both.
 */
export function mix(intensity: number, sharpness: number): { heavy: number; light: number } {
	const i = clamp01(intensity);
	const s = clamp01(sharpness);
	return {
		heavy: i * Math.min(1, 1.25 * (1 - s)),
		light: i * Math.min(1, 1.25 * s),
	};
}

function endOf(event: PatternEvent): number {
	return event.type === 'transient'
		? event.at + MIN_TAP_MS
		: event.at + Math.max(event.duration, MIN_CONTINUOUS_MS);
}

type Levels = { heavy: number; light: number; left: number; right: number };

/** The levels every event asks for at time `t`; overlapping events take the stronger level. */
function levelsAt(events: PatternEvent[], t: number, tier: Tier): Levels {
	const out: Levels = { heavy: 0, light: 0, left: 0, right: 0 };
	for (const event of events) {
		if (t < event.at || t >= endOf(event)) continue;
		let m: { heavy: number; light: number };
		let trigger: { left: number; right: number };
		if (event.type === 'transient') {
			m = mix(event.intensity, event.sharpness);
			trigger = { left: 0, right: clamp01(event.intensity) };
		} else {
			const c = event as ContinuousEvent;
			const p = (t - c.at) / Math.max(c.duration, MIN_CONTINUOUS_MS);
			const i = curveAt(c.intensity, p);
			m = mix(i, curveAt(c.sharpness, p));
			trigger = { left: i, right: i };
		}
		out.heavy = Math.max(out.heavy, m.heavy);
		out.light = Math.max(out.light, m.light);
		if (tier >= 3) {
			out.left = Math.max(out.left, trigger.left);
			out.right = Math.max(out.right, trigger.right);
		}
	}
	if (tier <= 1) {
		out.heavy = Math.max(out.heavy, out.light);
		out.light = 0;
	}
	return out;
}

const near = (a: MotorFrame, b: MotorFrame) =>
	Math.abs(a.heavy - b.heavy) < SAME_LEVEL &&
	Math.abs(a.light - b.light) < SAME_LEVEL &&
	Math.abs((a.leftTrigger ?? 0) - (b.leftTrigger ?? 0)) < SAME_LEVEL &&
	Math.abs((a.rightTrigger ?? 0) - (b.rightTrigger ?? 0)) < SAME_LEVEL;

const isSilent = (f: MotorFrame) =>
	f.heavy === 0 && f.light === 0 && !(f.leftTrigger ?? 0) && !(f.rightTrigger ?? 0);

/**
 * The tier a frame list needs, which is what the Rust side reports back as its request tier: 3 with
 * triggers, 2 when both body motors are used and differ somewhere, else 1.
 */
export function framesTier(frames: MotorFrame[]): Tier {
	if (frames.length === 0 || frames.every(isSilent)) return 0;
	if (frames.some((f) => (f.leftTrigger ?? 0) > 0 || (f.rightTrigger ?? 0) > 0)) return 3;
	const heavy = frames.some((f) => f.heavy > 0);
	const light = frames.some((f) => f.light > 0);
	if (heavy && light && frames.some((f) => f.heavy !== f.light)) return 2;
	return 1;
}

const round = (v: number) => Math.round(v * 1000) / 1000;

export function compilePad(
	pattern: Pattern,
	caps: PadCaps,
	options: PadCompileOptions = {}
): PadCompileReport {
	const notes: string[] = [];
	const tier = Math.min(caps.topTier, options.maxTier ?? 3) as Tier;
	if (tier === 0) {
		return { frames: [], tier: 0, estimatedMs: 0, notes: ['This pad cannot play rumble'] };
	}
	if (tier < 3 && caps.topTier >= 3 && options.maxTier !== undefined) {
		notes.push(`Compiled for tier ${tier} as asked`);
	}

	const events = [...pattern.events].sort((a, b) => a.at - b.at);
	let end = Math.max(0, ...events.map(endOf));
	if (end > caps.maxDurationMs) {
		end = caps.maxDurationMs;
		notes.push(`Truncated at the limit of ${caps.maxDurationMs} ms`);
	}
	if (end === 0) return { frames: [], tier: 0, estimatedMs: 0, notes };

	// Widen the sample step when a long pattern would otherwise need more frames than allowed.
	const step = Math.max(SAMPLE_MS, Math.ceil(end / (MAX_FRAMES - 1)));
	if (step > SAMPLE_MS) notes.push(`Sampled every ${step} ms to stay within ${MAX_FRAMES} frames`);

	const frames: MotorFrame[] = [];
	for (let start = 0; start < end; start += step) {
		const durationMs = Math.min(step, end - start);
		const l = levelsAt(events, start + durationMs / 2, tier);
		const frame: MotorFrame = { durationMs, heavy: round(l.heavy), light: round(l.light) };
		if (tier >= 3) {
			frame.leftTrigger = round(l.left);
			frame.rightTrigger = round(l.right);
		}
		const last = frames[frames.length - 1];
		if (last && near(last, frame)) last.durationMs += durationMs;
		else frames.push(frame);
	}

	// A trailing silence carries no rumble.
	while (frames.length > 0 && isSilent(frames[frames.length - 1])) frames.pop();
	const estimatedMs = frames.reduce((sum, f) => sum + f.durationMs, 0);
	const needed = framesTier(frames);
	if (needed === 0) {
		notes.push('Nothing in the pattern is strong enough to feel');
		return { frames: [], tier: 0, estimatedMs: 0, notes };
	}
	return { frames, tier: Math.min(needed, tier) as Tier, estimatedMs, notes };
}
