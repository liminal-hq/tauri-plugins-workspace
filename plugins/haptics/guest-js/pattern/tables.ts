// Primitive tables shared by the compiler: durations, amplitude ceilings and neighbours
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import type { Capabilities, PrimitiveId } from '../types';
import type { ContinuousEvent, PatternEvent } from './types';

export const PRIMITIVE_IDS: PrimitiveId[] = [
	'tick',
	'low_tick',
	'click',
	'thud',
	'spin',
	'quick_rise',
	'slow_rise',
];

/** Built-in primitive durations in ms, used when the motor does not report its own. */
export const PRIMITIVE_MS: Record<PrimitiveId, number> = {
	tick: 10,
	low_tick: 12,
	click: 15,
	thud: 30,
	quick_rise: 60,
	slow_rise: 150,
	spin: 90,
};

/** The amplitude (0..255) a full-strength primitive stands in for at tier 2. */
export const AMPLITUDE_CEILING: Record<PrimitiveId, number> = {
	tick: 140,
	low_tick: 120,
	click: 200,
	thud: 255,
	quick_rise: 220,
	slow_rise: 220,
	spin: 200,
};

/** Nearest stand-ins, tried in order, when a motor lacks a primitive. */
export const NEIGHBOURS: Record<PrimitiveId, PrimitiveId[]> = {
	low_tick: ['tick', 'click'],
	tick: ['click'],
	thud: ['click'],
	spin: ['quick_rise'],
	slow_rise: ['quick_rise'],
	quick_rise: [],
	click: [],
};

/** The measured duration of a primitive on this motor, or the built-in value. */
export function primitiveMs(caps: Capabilities, id: PrimitiveId): number {
	return caps.primitives[id]?.durationMs ?? PRIMITIVE_MS[id];
}

export function isPrimitiveSupported(caps: Capabilities, id: PrimitiveId): boolean {
	return caps.primitives[id]?.supported === true;
}

/**
 * The primitive a supported one stands in for: `id` itself, else its first supported neighbour.
 * Returns `null` when neither exists.
 */
export function resolvePrimitive(
	caps: Capabilities,
	id: PrimitiveId
): { id: PrimitiveId; note?: string } | null {
	if (isPrimitiveSupported(caps, id)) return { id };
	for (const next of NEIGHBOURS[id]) {
		if (isPrimitiveSupported(caps, next)) {
			return { id: next, note: `${id} missing on this motor → ${next}` };
		}
	}
	return null;
}

/** The first and last value of an event's intensity, whether it is flat or a curve. */
function intensityEnds(ev: ContinuousEvent): [number, number] {
	if (typeof ev.intensity === 'number') return [ev.intensity, ev.intensity];
	return [ev.intensity[0].v, ev.intensity[ev.intensity.length - 1].v];
}

/** Which primitive an event would use, before any neighbour substitution. */
export function pickPrimitive(ev: PatternEvent): PrimitiveId {
	if (ev.type === 'transient') {
		if (ev.sharpness >= 0.6) return ev.intensity < 0.4 ? 'tick' : 'click';
		if (ev.sharpness < 0.4) return ev.intensity < 0.4 ? 'low_tick' : 'thud';
		return 'click';
	}
	const [first, last] = intensityEnds(ev);
	if (last > first + 0.05) return ev.duration < 150 ? 'quick_rise' : 'slow_rise';
	return 'spin';
}
