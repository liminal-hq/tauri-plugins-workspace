// Property tests for the compiler: whatever the pattern and device, the report matches the steps
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import fc from 'fast-check';
import { describe, expect, it } from 'vitest';
import { compilePattern } from '../../../plugins/haptics/guest-js/pattern/compile';
import { primitiveMs } from '../../../plugins/haptics/guest-js/pattern/tables';
import { PATTERN_FORMAT } from '../../../plugins/haptics/guest-js/pattern/types';
import type { Pattern, PatternEvent } from '../../../plugins/haptics/guest-js/pattern/types';
import { validatePattern } from '../../../plugins/haptics/guest-js/pattern/validate';
import type {
	Capabilities,
	CompiledStep,
	PrimitiveId,
	Tier,
} from '../../../plugins/haptics/guest-js/types';
import { checkRequest, checkSteps } from '../conformance/request-rules';
import { fixtures } from './__fixtures__/capabilities';

/** Fixed seed, so a failure reproduces; set HAPTICS_FC_RUNS for a deeper local run. */
const runs = Number(process.env.HAPTICS_FC_RUNS ?? 200);
const options = { seed: 0x5eed, numRuns: runs };

const unit = fc.double({ min: 0, max: 1, noNaN: true });

const curve = fc
	.array(fc.tuple(fc.integer({ min: 1, max: 98 }), unit), { minLength: 0, maxLength: 4 })
	.chain((mid) =>
		fc.tuple(unit, unit).map(([first, last]) => {
			const ts = [...new Set(mid.map(([t]) => t))].sort((a, b) => a - b);
			return [
				{ t: 0, v: first },
				...ts.map((t) => ({ t: t / 100, v: mid.find(([x]) => x === t)![1] })),
				{ t: 1, v: last },
			];
		})
	);

const level = fc.oneof(unit, curve);
/** Events placed inside `limit`, so most patterns are valid and crowd the end of the limit. */
const eventWithin = (limit: number): fc.Arbitrary<PatternEvent> => {
	const at = fc.oneof(
		fc.integer({ min: 0, max: limit - 1 }),
		fc.double({ min: 0, max: limit - 1, noNaN: true })
	);
	return fc.oneof(
		fc.record({ type: fc.constant('transient' as const), at, intensity: unit, sharpness: unit }),
		at.chain((start) =>
			fc.record({
				type: fc.constant('continuous' as const),
				at: fc.constant(start),
				duration: fc.integer({ min: 20, max: Math.max(20, Math.floor(limit - start)) }),
				intensity: level,
				sharpness: level,
			})
		)
	);
};

const patternWithin = (limit: number): fc.Arbitrary<Pattern> =>
	fc.array(eventWithin(limit), { minLength: 1, maxLength: 10 }).map((events) => ({
		format: PATTERN_FORMAT,
		events,
	}));

const PRIMITIVES: PrimitiveId[] = [
	'tick',
	'low_tick',
	'click',
	'thud',
	'spin',
	'quick_rise',
	'slow_rise',
];

/** A fixture device with a random duration limit and a random set of primitives switched off. */
const device: fc.Arbitrary<Capabilities> = fc
	.record({
		base: fc.constantFrom(...Object.values(fixtures)),
		amplitudeControl: fc.boolean(),
		maxDurationMs: fc.constantFrom(100, 300, 1000, 10_000),
		maxAmplitude: fc.constantFrom(255, 128),
		off: fc.subarray(PRIMITIVES),
	})
	.map(({ base: fixture, amplitudeControl, maxDurationMs, maxAmplitude, off }) => {
		// Only a device that can vary its strength has amplitude control; keep the tier consistent.
		const base =
			fixture.topTier >= 2 ? { ...fixture, hasAmplitudeControl: amplitudeControl } : fixture;
		const primitives = { ...base.primitives };
		for (const id of off) primitives[id] = { supported: false, durationMs: null };
		return { ...base, primitives, limits: { ...base.limits, maxDurationMs, maxAmplitude } };
	});

const tier = fc.option(fc.constantFrom<Tier>(0, 1, 2, 3, 4), { nil: undefined });

const compiled = device
	.chain((caps) =>
		fc.record({
			pattern: patternWithin(caps.limits.maxDurationMs),
			caps: fc.constant(caps),
			tier,
			scale: unit,
		})
	)
	.filter(
		({ pattern: p, caps }) =>
			validatePattern(p, { maxDurationMs: caps.limits.maxDurationMs }).length === 0
	)
	.map((input) => ({
		...input,
		report: compilePattern(input.pattern, input.caps, { tier: input.tier, scale: input.scale }),
	}));

/** How long a step plays for, worked out from the request alone. */
function stepLength(step: CompiledStep, caps: Capabilities): number {
	const e = step.request.effect;
	switch (e.type) {
		case 'composition':
			return e.steps.reduce((t, s) => t + (s.delayMs ?? 0) + primitiveMs(caps, s.primitive), 0);
		case 'waveform':
			return e.timingsMs.reduce((t, d) => t + d, 0);
		case 'envelopeWaveform':
			return e.controlPoints.reduce((t, p) => t + p.durationMs, 0);
		case 'oneshot':
			return e.durationMs;
		case 'predefined':
			return 0;
	}
}

describe('compiler properties', () => {
	it('reports the duration the steps play for', () => {
		fc.assert(
			fc.property(compiled, ({ report, caps }) => {
				const end = report.steps.reduce((m, s) => Math.max(m, s.atMs + stepLength(s, caps)), 0);
				// A waveform plays no bar for an off phase or a zero amplitude, so its end can only be
				// earlier; compositions and envelopes play to the end.
				expect(report.estimatedMs).toBeLessThanOrEqual(end);
				if (report.tier === 4 || (report.tier === 3 && !report.mixed)) {
					expect(report.estimatedMs).toBe(end);
				}
			}),
			options
		);
	});

	it('keeps every step inside the duration limit, in order and apart', () => {
		fc.assert(
			fc.property(compiled, ({ report, caps }) => {
				let previousEnd = 0;
				for (const s of report.steps) {
					expect(s.atMs).toBeGreaterThanOrEqual(previousEnd);
					expect(s.atMs).toBeLessThan(caps.limits.maxDurationMs);
					previousEnd = s.atMs + stepLength(s, caps);
					expect(previousEnd).toBeLessThanOrEqual(caps.limits.maxDurationMs);
				}
			}),
			options
		);
	});

	it('emits no amplitude waveform for a device without amplitude control', () => {
		fc.assert(
			fc.property(compiled, ({ report, caps }) => {
				if (caps.hasAmplitudeControl) return;
				for (const s of report.steps) {
					const e = s.request.effect;
					if (e.type === 'waveform') expect(e.amplitudes).toBeUndefined();
				}
			}),
			options
		);
	});

	it('is deterministic', () => {
		fc.assert(
			fc.property(compiled, ({ pattern: p, caps, tier: t, scale, report }) => {
				expect(compilePattern(p, caps, { tier: t, scale })).toEqual(report);
			}),
			options
		);
	});

	it('only emits steps the plugin would accept', () => {
		fc.assert(
			fc.property(compiled, ({ report, caps }) => {
				if (report.steps.length === 0) return;
				expect(checkSteps(report.steps, caps.limits)).toBeNull();
				for (const s of report.steps) {
					expect(checkRequest(s.request, caps.limits.maxDurationMs - s.atMs)).toBeNull();
				}
			}),
			options
		);
	});

	it('sends whole milliseconds to native', () => {
		fc.assert(
			fc.property(compiled, ({ report }) => {
				for (const s of report.steps) {
					expect(Number.isInteger(s.atMs)).toBe(true);
					const e = s.request.effect;
					if (e.type === 'waveform') expect(e.timingsMs.every(Number.isInteger)).toBe(true);
					if (e.type === 'composition') {
						expect(e.steps.every((c) => Number.isInteger(c.delayMs ?? 0))).toBe(true);
					}
					if (e.type === 'envelopeWaveform') {
						expect(e.controlPoints.every((p) => Number.isInteger(p.durationMs))).toBe(true);
					}
				}
			}),
			options
		);
	});
});
