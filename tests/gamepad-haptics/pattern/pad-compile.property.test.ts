// Property tests: whatever pattern is compiled, the frames satisfy the request rules
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import fc from 'fast-check';
import { describe, expect, it } from 'vitest';
import {
	compilePad,
	framesTier,
} from '../../../plugins/gamepad-haptics/guest-js/pattern/pad-compile';
import type { PadCaps } from '../../../plugins/gamepad-haptics/guest-js/pattern/pad-compile';
import { PATTERN_FORMAT } from '../../../plugins/gamepad-haptics/guest-js/pattern/types';
import type { PatternEvent } from '../../../plugins/gamepad-haptics/guest-js/pattern/types';
import { checkFrames } from '../conformance/frame-rules';

const runs = Number(process.env.HAPTICS_FC_RUNS ?? 200);
const options = { seed: 0x5eed, numRuns: runs };

const unit = fc.integer({ min: 0, max: 100 }).map((n) => n / 100);
const curve = fc.oneof(
	unit,
	fc.array(fc.record({ t: unit, v: unit }), { minLength: 1, maxLength: 5 })
);
const event: fc.Arbitrary<PatternEvent> = fc.oneof(
	fc.record({
		type: fc.constant('transient' as const),
		at: fc.integer({ min: 0, max: 2500 }),
		intensity: unit,
		sharpness: unit,
	}),
	fc.record({
		type: fc.constant('continuous' as const),
		at: fc.integer({ min: 0, max: 2500 }),
		duration: fc.integer({ min: 20, max: 1500 }),
		intensity: curve,
		sharpness: curve,
	})
);
const patterns = fc.array(event, { minLength: 1, maxLength: 30 });
const tiers = fc.constantFrom(0, 1, 2, 3) as fc.Arbitrary<0 | 1 | 2 | 3>;
const limits = fc.integer({ min: 100, max: 8000 });

const pad = (topTier: 0 | 1 | 2 | 3, maxDurationMs: number): PadCaps => ({
	topTier,
	maxDurationMs,
});

describe('compilePad properties', () => {
	it('emits frames the Rust rules accept, within the limit', () => {
		fc.assert(
			fc.property(patterns, tiers, limits, (events, tier, max) => {
				const r = compilePad({ format: PATTERN_FORMAT, events }, pad(tier, max));
				if (r.frames.length === 0) return;
				expect(checkFrames(r.frames, { maxDurationMs: max, maxContinuousMs: max })).toBeUndefined();
			}),
			options
		);
	});

	it('reports the tier of the frames it emits, never above the pad', () => {
		fc.assert(
			fc.property(patterns, tiers, limits, (events, tier, max) => {
				const r = compilePad({ format: PATTERN_FORMAT, events }, pad(tier, max));
				expect(r.tier).toBeLessThanOrEqual(tier);
				expect(r.tier).toBe(Math.min(framesTier(r.frames), tier));
			}),
			options
		);
	});

	it('reports an end that equals the frames, and compiles the same twice', () => {
		fc.assert(
			fc.property(patterns, tiers, limits, (events, tier, max) => {
				const p = { format: PATTERN_FORMAT, events };
				const a = compilePad(p, pad(tier, max));
				expect(a.estimatedMs).toBe(a.frames.reduce((s, f) => s + f.durationMs, 0));
				expect(a.estimatedMs).toBeLessThanOrEqual(max);
				expect(compilePad(p, pad(tier, max))).toEqual(a);
			}),
			options
		);
	});

	it('never uses a motor the pad does not have', () => {
		fc.assert(
			fc.property(patterns, limits, (events, max) => {
				const dual = compilePad({ format: PATTERN_FORMAT, events }, pad(2, max));
				expect(dual.frames.every((f) => f.leftTrigger === undefined)).toBe(true);
				const single = compilePad({ format: PATTERN_FORMAT, events }, pad(1, max));
				expect(single.frames.every((f) => f.light === 0)).toBe(true);
			}),
			options
		);
	});
});
