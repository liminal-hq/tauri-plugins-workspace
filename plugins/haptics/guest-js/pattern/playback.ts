// Models what the native side plays for a list of compiled steps, for previews and estimates
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import type { Capabilities, CompiledStep, Tier } from '../types';
import { primitiveMs } from './tables';

/** One bar of the compiled pattern, for previews: when, how long and how strong (0..1). */
export type CompiledSegment = {
	atMs: number;
	durationMs: number;
	amplitude: number;
	tier: Tier; // the tier this segment was compiled at (2 inside a mixed tier-3 pattern)
	label?: string; // the primitive, at tier 3
};

/** The segments one step plays, starting `at` ms into the pattern. */
function stepSegments(step: CompiledStep, caps: Capabilities): CompiledSegment[] {
	const e = step.request.effect;
	const out: CompiledSegment[] = [];
	let t = step.atMs;
	switch (e.type) {
		case 'composition':
			for (const s of e.steps) {
				t += s.delayMs ?? 0;
				const durationMs = primitiveMs(caps, s.primitive);
				out.push({
					atMs: t,
					durationMs,
					amplitude: s.scale ?? 1,
					tier: 3,
					label: s.primitive,
				});
				t += durationMs;
			}
			break;
		case 'waveform':
			// Timings alternate off and on. With amplitudes each timing has its own strength; without
			// them the odd timings are the full-strength "on" phases.
			e.timingsMs.forEach((durationMs, i) => {
				const amp = e.amplitudes ? e.amplitudes[i] : i % 2 === 1 ? 255 : 0;
				if (durationMs > 0 && amp > 0) {
					out.push({
						atMs: t,
						durationMs,
						amplitude: amp / 255,
						tier: e.amplitudes ? 2 : 1,
					});
				}
				t += durationMs;
			});
			break;
		case 'envelopeWaveform':
			for (const p of e.controlPoints) {
				out.push({ atMs: t, durationMs: p.durationMs, amplitude: p.amplitude, tier: 4 });
				t += p.durationMs;
			}
			break;
		case 'oneshot':
			out.push({
				atMs: t,
				durationMs: e.durationMs,
				amplitude: (e.amplitude ?? 255) / 255,
				tier: 2,
			});
			break;
		case 'predefined':
			break;
	}
	return out;
}

/**
 * Where each bar of a step list plays. A step replaces whatever is still playing when it starts,
 * so a bar is cut at the start of the step after it.
 */
export function placeSteps(steps: CompiledStep[], caps: Capabilities): CompiledSegment[] {
	const ordered = steps
		.map((step, i) => ({ step, i }))
		.sort((a, b) => a.step.atMs - b.step.atMs || a.i - b.i)
		.map((x) => x.step);

	const out: CompiledSegment[] = [];
	ordered.forEach((step, i) => {
		const next = ordered[i + 1]?.atMs;
		for (const seg of stepSegments(step, caps)) {
			if (next === undefined || seg.atMs + seg.durationMs <= next) {
				out.push(seg);
			} else if (seg.atMs < next) {
				out.push({ ...seg, durationMs: next - seg.atMs });
			}
		}
	});
	return out;
}

/** When the last bar ends, in whole milliseconds. */
export function playbackEnd(segments: CompiledSegment[]): number {
	return Math.round(segments.reduce((m, s) => Math.max(m, s.atMs + s.durationMs), 0));
}
