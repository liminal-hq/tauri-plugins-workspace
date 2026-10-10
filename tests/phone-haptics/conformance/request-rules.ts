// A test-only mirror of the plugin's request rules, checked against the shared corpus
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import {
	EFFECT_IDS,
	MAX_STEPS,
	PRIMITIVE_IDS,
} from '../../../plugins/phone-haptics/guest-js/pattern/tables';
import type { CompiledStep, EffectRequest } from '../../../plugins/phone-haptics/guest-js/types';

export type Limits = { maxDurationMs: number };

const inRange = (v: number, min: number, max: number) => v >= min && v <= max;

/** The reason a request is invalid, or `null` when it is valid within `budgetMs`. */
export function checkRequest(req: EffectRequest, budgetMs: number): string | null {
	const e = req.effect;
	switch (e.type) {
		case 'oneshot':
			if (!(e.durationMs > 0)) return 'durationMs must be positive';
			if (e.amplitude !== undefined && !inRange(e.amplitude, 1, 255)) {
				return 'amplitude must be within 1..255';
			}
			return null;
		case 'waveform': {
			if (e.timingsMs.length === 0) return 'timingsMs cannot be empty';
			if (e.timingsMs.every((t) => t === 0)) return 'at least one timing must be non-zero';
			if (e.amplitudes) {
				if (e.amplitudes.length !== e.timingsMs.length) {
					return 'amplitudes must have same length as timingsMs';
				}
				if (e.amplitudes.some((a) => a > 255)) return 'amplitudes must be within 0..255';
			}
			if (e.repeat !== undefined && (e.repeat < -1 || e.repeat >= e.timingsMs.length)) {
				return 'repeat must be -1 or an index into timingsMs';
			}
			return null;
		}
		case 'predefined':
			return EFFECT_IDS.includes(e.effectId.toLowerCase() as never)
				? null
				: `Unknown predefined effect \`${e.effectId.toLowerCase()}\``;
		case 'composition':
			for (const [i, step] of e.steps.entries()) {
				if (!PRIMITIVE_IDS.includes(step.primitive.toLowerCase() as never)) {
					return `steps[${i}]: unknown primitive \`${step.primitive.toLowerCase()}\``;
				}
				if (step.scale !== undefined && !inRange(step.scale, 0, 1)) {
					return `steps[${i}]: scale must be within 0..1`;
				}
			}
			return null;
		case 'envelopeWaveform': {
			if (
				e.initialFrequencyHz !== undefined &&
				!(Number.isFinite(e.initialFrequencyHz) && e.initialFrequencyHz > 0)
			) {
				return 'initialFrequencyHz must be positive';
			}
			if (e.controlPoints.length === 0) return 'controlPoints cannot be empty';
			let total = 0;
			for (const [i, p] of e.controlPoints.entries()) {
				if (!inRange(p.amplitude, 0, 1))
					return `controlPoints[${i}]: amplitude must be within 0..1`;
				if (!(Number.isFinite(p.frequencyHz) && p.frequencyHz > 0)) {
					return `controlPoints[${i}]: frequencyHz must be positive`;
				}
				if (!(p.durationMs > 0)) return `controlPoints[${i}]: durationMs must be positive`;
				total += p.durationMs;
			}
			return total > budgetMs
				? `envelope duration ${total} ms exceeds the limit of ${budgetMs} ms`
				: null;
		}
	}
}

/** The reason a step list is invalid, or `null` when every rule holds. */
export function checkSteps(steps: CompiledStep[], limits: Limits): string | null {
	if (steps.length === 0) return 'steps cannot be empty';
	if (steps.length > MAX_STEPS) return `steps exceeds the maximum of ${MAX_STEPS}`;
	for (const [i, step] of steps.entries()) {
		if (step.atMs >= limits.maxDurationMs) {
			return `steps[${i}]: atMs ${step.atMs} is not below the limit of ${limits.maxDurationMs} ms`;
		}
		const reason = checkRequest(step.request, limits.maxDurationMs - step.atMs);
		if (reason) return `steps[${i}]: ${reason}`;
	}
	return null;
}
