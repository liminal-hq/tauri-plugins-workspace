// Portable haptic pattern format, shared by the validator, compiler and scheduler
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

// This folder is pure TypeScript: it must not import from `@tauri-apps/*` or from `../index`. It
// may import types from `../types` until it is extracted into its own package.

import type { HapticsUsage, Tier } from '../types';

export const PATTERN_FORMAT = 'haptics-lab/pattern@1';

/** A point on a curve; `t` runs 0..1 across the event and `v` is 0..1. */
export type CurvePoint = { t: number; v: number };

export type TransientEvent = {
	type: 'transient';
	at: number; // ms from the start of the pattern
	intensity: number; // 0..1
	sharpness: number; // 0..1, dull to crisp
};

export type ContinuousEvent = {
	type: 'continuous';
	at: number; // ms from the start of the pattern
	duration: number; // ms
	intensity: number | CurvePoint[];
	sharpness: number | CurvePoint[];
};

export type PatternEvent = TransientEvent | ContinuousEvent;

/** What happens when a pattern is triggered while it is still playing. */
export type Policy = 'interrupt' | 'queue' | 'drop-if-busy' | { coalesce: number };

export type Pattern = {
	format: typeof PATTERN_FORMAT;
	id?: string;
	usage?: HapticsUsage; // default 'media' for patterns
	policy?: Policy; // default 'interrupt'
	events: PatternEvent[]; // any order; the compiler sorts by `at`
};

export type RegisterOptions = { tier?: Tier };

export type TriggerOptions = {
	scale?: number; // 0..1, multiplies with the master scale
	tier?: Tier; // force a lower tier for this call
	usage?: HapticsUsage; // override the pattern's usage
	respectSystemSettings?: boolean;
};

/** Shortest continuous event, in ms. */
export const MIN_CONTINUOUS_MS = 20;

/** A transient always plays for at least this long, so it needs that much room before the limit. */
export const MIN_TRANSIENT_MS = 1;
