// Capability fixtures for the compiler tests: one device per tier plus the Pixel 8 Pro
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import type {
	Capabilities,
	PrimitiveId,
	PrimitiveSupport,
	Tier,
} from '../../../../plugins/haptics/guest-js/types';

const IDS: PrimitiveId[] = ['tick', 'low_tick', 'click', 'thud', 'spin', 'quick_rise', 'slow_rise'];

function prims(
	durations: Partial<Record<PrimitiveId, number>>
): Record<PrimitiveId, PrimitiveSupport> {
	const out = {} as Record<PrimitiveId, PrimitiveSupport>;
	for (const id of IDS) {
		const ms = durations[id];
		out[id] = ms ? { supported: true, durationMs: ms } : { supported: false, durationMs: null };
	}
	return out;
}

const limits = { maxDurationMs: 10_000, maxAmplitude: 255, allowRepeatingWaveforms: false };
const effects = { click: 'yes', double_click: 'yes', tick: 'yes', heavy_click: 'yes' } as const;

function device(topTier: Tier, patch: Partial<Capabilities>, model: string): Capabilities {
	return {
		platform: 'android',
		sdkInt: 36,
		hasVibrator: topTier > 0,
		hasAmplitudeControl: topTier >= 2,
		topTier,
		compositionSupported: false,
		primitives: prims({}),
		effects: { ...effects },
		envelopeSupported: false,
		touchFeedbackEnabled: true,
		limits: { ...limits },
		device: { manufacturer: 'Test', model, release: '16' },
		...patch,
	};
}

/** Tier 4: envelope support with a frequency profile and every primitive. */
export const envelopeDevice = device(
	4,
	{
		compositionSupported: true,
		primitives: prims({
			tick: 12,
			low_tick: 14,
			click: 16,
			thud: 31,
			spin: 88,
			quick_rise: 58,
			slow_rise: 146,
		}),
		envelopeSupported: true,
		envelopeInfo: {
			maxSize: 16,
			minControlPointDurationMs: 20,
			maxControlPointDurationMs: 1000,
			maxDurationMs: 5000,
			frequencyProfile: { minHz: 60, maxHz: 300 },
		},
		resonantHz: 160,
		qFactor: 8.4,
	},
	'Envelope phone'
);

/** Tier 3 and the test phone: primitives and amplitude control, no envelope. */
export const pixel8Pro = device(
	3,
	{
		sdkInt: 37,
		compositionSupported: true,
		primitives: prims({
			tick: 10,
			low_tick: 12,
			click: 15,
			thud: 30,
			spin: 90,
			quick_rise: 60,
			slow_rise: 150,
		}),
		resonantHz: 146.5,
	},
	'Pixel 8 Pro'
);

/** Tier 3 on a motor missing `low_tick` and `spin`. */
export const midRange = device(
	3,
	{
		sdkInt: 35,
		compositionSupported: true,
		primitives: prims({ tick: 10, click: 15, thud: 28, quick_rise: 62, slow_rise: 150 }),
	},
	'Mid-range phone'
);

/** Tier 2: amplitude control, no primitives. */
export const budgetAmplitude = device(2, { sdkInt: 33 }, 'Budget phone');

/** Tier 1: a motor that only switches on and off. */
export const onOffOnly = device(1, { sdkInt: 30, hasAmplitudeControl: false }, 'Budget tablet');

/** Tier 0: no vibrator, as on the desktop shell. */
export const desktop = device(
	0,
	{
		platform: 'desktop',
		sdkInt: undefined,
		effects: { click: 'no', double_click: 'no', tick: 'no', heavy_click: 'no' },
	},
	'linux'
);

export const fixtures = {
	envelopeDevice,
	pixel8Pro,
	midRange,
	budgetAmplitude,
	onOffOnly,
	desktop,
};
