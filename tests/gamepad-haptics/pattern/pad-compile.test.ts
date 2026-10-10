// Unit tests for compiling patterns to motor frames
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import { describe, expect, it } from 'vitest';
import {
	MAX_FRAMES,
	MIN_TAP_MS,
	compilePad,
	curveAt,
	framesTier,
	mix,
} from '../../../plugins/gamepad-haptics/guest-js/pattern/pad-compile';
import type { PadCaps } from '../../../plugins/gamepad-haptics/guest-js/pattern/pad-compile';
import { PATTERN_FORMAT } from '../../../plugins/gamepad-haptics/guest-js/pattern/types';
import type { PatternEvent } from '../../../plugins/gamepad-haptics/guest-js/pattern/types';
import { checkFrames } from '../conformance/frame-rules';

const caps = (topTier: 0 | 1 | 2 | 3 = 2, maxDurationMs = 3000): PadCaps => ({
	topTier,
	maxDurationMs,
});
const pattern = (...events: PatternEvent[]) => ({ format: PATTERN_FORMAT, events }) as const;
const tap = (at: number, intensity: number, sharpness: number): PatternEvent => ({
	type: 'transient',
	at,
	intensity,
	sharpness,
});

describe('mix', () => {
	it('sends a crisp hit to the light motor and a dull one to the heavy motor', () => {
		expect(mix(1, 1)).toEqual({ heavy: 0, light: 1 });
		expect(mix(1, 0)).toEqual({ heavy: 1, light: 0 });
		expect(mix(0.8, 0.5)).toEqual({ heavy: 0.5, light: 0.5 });
	});
});

describe('curveAt', () => {
	it('reads constants and interpolates curves', () => {
		expect(curveAt(0.4, 0.9)).toBe(0.4);
		const fall = [
			{ t: 0, v: 1 },
			{ t: 1, v: 0 },
		];
		expect(curveAt(fall, 0.25)).toBeCloseTo(0.75);
		expect(curveAt(fall, 2)).toBe(0);
	});
});

describe('compilePad', () => {
	it('plays a sharp tap on the light motor for at least the tap length', () => {
		const r = compilePad(pattern(tap(0, 1, 1)), caps());
		expect(r.frames).toEqual([{ durationMs: MIN_TAP_MS, heavy: 0, light: 1 }]);
		expect(r.tier).toBe(2);
	});

	it('keeps the gap before a later event as a silent frame', () => {
		const r = compilePad(pattern(tap(100, 1, 0)), caps());
		expect(r.frames[0]).toEqual({ durationMs: 100, heavy: 0, light: 0 });
		expect(r.estimatedMs).toBe(140);
	});

	it('follows a hum that falls from heavy to nothing', () => {
		const r = compilePad(
			pattern({
				type: 'continuous',
				at: 0,
				duration: 200,
				intensity: [
					{ t: 0, v: 1 },
					{ t: 1, v: 0 },
				],
				sharpness: 0,
			}),
			caps()
		);
		const heavy = r.frames.map((f) => f.heavy);
		expect(heavy[0]).toBeGreaterThan(0.9);
		expect(heavy[heavy.length - 1]).toBeLessThan(0.15);
		expect([...heavy].sort((a, b) => b - a)).toEqual(heavy);
	});

	it('merges both motors into one on a single-motor pad', () => {
		const r = compilePad(pattern(tap(0, 1, 1)), caps(1));
		expect(r.frames).toEqual([{ durationMs: MIN_TAP_MS, heavy: 1, light: 0 }]);
		expect(r.tier).toBe(1);
	});

	it('adds trigger levels only for a trigger pad', () => {
		const withTriggers = compilePad(pattern(tap(0, 0.5, 0.5)), caps(3));
		expect(withTriggers.frames[0].rightTrigger).toBe(0.5);
		expect(withTriggers.tier).toBe(3);
		const without = compilePad(pattern(tap(0, 0.5, 0.5)), caps(2));
		expect(without.frames[0].rightTrigger).toBeUndefined();
	});

	it('compiles for a lower tier when asked', () => {
		const r = compilePad(pattern(tap(0, 0.5, 0.5)), caps(3), { maxTier: 1 });
		expect(r.tier).toBe(1);
		expect(r.frames[0].rightTrigger).toBeUndefined();
	});

	it('plays nothing on a pad that cannot rumble', () => {
		const r = compilePad(pattern(tap(0, 1, 1)), caps(0));
		expect(r).toMatchObject({ frames: [], tier: 0 });
	});

	it('reports a pattern too faint to feel as tier 0', () => {
		const r = compilePad(pattern(tap(0, 0, 0.5)), caps());
		expect(r.tier).toBe(0);
		expect(r.notes.join(' ')).toContain('Nothing');
	});

	it('truncates at the duration limit and says so', () => {
		const r = compilePad(
			pattern({
				type: 'continuous',
				at: 0,
				duration: 5000,
				intensity: 1,
				sharpness: 0,
			}),
			caps(2, 1000)
		);
		expect(r.estimatedMs).toBe(1000);
		expect(r.notes.join(' ')).toContain('Truncated');
	});

	it('stays within the frame limit however long the limit is', () => {
		const events: PatternEvent[] = [];
		for (let i = 0; i < 400; i++) events.push(tap(i * 40, 1, i % 2));
		const r = compilePad(pattern(...events), caps(2, 60_000));
		expect(r.frames.length).toBeLessThanOrEqual(MAX_FRAMES);
		expect(
			checkFrames(r.frames, { maxDurationMs: 60_000, maxContinuousMs: 60_000 })
		).toBeUndefined();
	});

	it('derives the tier from the frames it emits', () => {
		expect(framesTier([])).toBe(0);
		expect(framesTier([{ durationMs: 10, heavy: 0, light: 0 }])).toBe(0);
		expect(framesTier([{ durationMs: 10, heavy: 0.5, light: 0.5 }])).toBe(1);
		expect(framesTier([{ durationMs: 10, heavy: 0.5, light: 0.2 }])).toBe(2);
		expect(framesTier([{ durationMs: 10, heavy: 0, light: 0, rightTrigger: 1 }])).toBe(3);
	});
});
