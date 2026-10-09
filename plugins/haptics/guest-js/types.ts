// Request, result and capability types shared with the native plugin
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

export type HapticsUsage =
	| 'touch' // foreground UI interactions
	| 'notification' // attentional
	| 'alarm' // background-allowed style
	| 'media';

export type EffectRequest = {
	id?: string; // for lab UI: track what you played
	usage?: HapticsUsage; // default from plugin config
	respectSystemSettings?: boolean; // default true
	stopBeforePlay?: boolean; // default true

	// one of:
	effect: OneShot | Waveform | Composition | Predefined | EnvelopeWaveform;
};

export type OneShot = {
	type: 'oneshot';
	durationMs: number;
	amplitude?: number; // 1..255, omit for default
};

export type Waveform = {
	type: 'waveform';
	timingsMs: number[]; // alternates off/on durations; often start with 0
	amplitudes?: number[]; // 0..255; if omitted, becomes on/off waveform
	repeat?: number; // -1 no repeat, else index into timings
};

export type Composition = {
	type: 'composition';
	steps: Array<{ kind: 'primitive'; primitive: PrimitiveId; scale?: number; delayMs?: number }>;
};

export type Predefined = {
	type: 'predefined';
	effectId: PredefinedEffectId;
};

// Android 16+ (API 36) only when supported.
export type EnvelopeWaveform = {
	type: 'envelopeWaveform';
	initialFrequencyHz?: number; // must lie within envelopeInfo.frequencyProfile when present
	// amplitude is 0..1; durationMs is per-segment and bounded by envelopeInfo
	controlPoints: Array<{ amplitude: number; frequencyHz: number; durationMs: number }>;
};

export type Tier = 0 | 1 | 2 | 3 | 4;

export type Platform = 'android' | 'ios' | 'desktop' | 'web';

export type Support = 'yes' | 'no' | 'unknown';

export type PrimitiveSupport = {
	supported: boolean;
	durationMs: number | null; // measured on this motor (API 31+), null when unknown
};

export type EnvelopeInfo = {
	maxSize: number;
	minControlPointDurationMs: number;
	maxControlPointDurationMs: number;
	maxDurationMs: number;
	frequencyProfile?: {
		minHz: number;
		maxHz: number;
	};
};

export type Capabilities = {
	platform: Platform;
	sdkInt?: number; // Android only
	hasVibrator: boolean;
	hasAmplitudeControl: boolean;
	topTier: Tier; // 0 no vibrator, 4 envelope, 3 primitives, 2 amplitude, 1 on/off

	// Composition primitives, reported one by one
	compositionSupported: boolean;
	primitives: Record<PrimitiveId, PrimitiveSupport>;

	// Predefined effects (API 30+; 'unknown' below)
	effects: Record<PredefinedEffectId, Support>;

	// Envelope effects (API 36)
	envelopeSupported: boolean;
	envelopeInfo?: EnvelopeInfo;

	resonantHz?: number; // API 31
	qFactor?: number; // API 31

	// System toggle behind touch-usage and UI-lane haptics; null when unreadable
	touchFeedbackEnabled: boolean | null;
	/** @deprecated Use `touchFeedbackEnabled`. Kept for one release. */
	hapticFeedbackEnabled?: boolean;

	// Plugin limits, so pure-TS code can clamp without reading the config
	limits: {
		maxDurationMs: number;
		maxAmplitude: number;
		allowRepeatingWaveforms: boolean;
	};

	device: {
		manufacturer: string;
		model: string;
		release: string; // OS version, for example "16"
	};
};

export type PlayResult = {
	ok: true; // invalid input rejects; hardware limits never make this false
	tier: Tier; // the tier that played
	target: 'phone';
	estimatedMs: number;
	downgraded: boolean;
	reason?: string; // why, in one sentence; several reasons are joined with ' · '
	policy?: 'played' | 'queued' | 'dropped' | 'coalesced';
	/** @deprecated Use `reason`. Kept for one release. */
	downgradeReason?: string;
};

export type PrimitiveId =
	| 'tick'
	| 'low_tick'
	| 'click'
	| 'thud'
	| 'spin'
	| 'quick_rise'
	| 'slow_rise';

// `thud` and `pop` are not predefined effects; use the `thud` composition primitive.
export type PredefinedEffectId = 'click' | 'double_click' | 'tick' | 'heavy_click';

// System-style feedback that follows the touch-feedback setting (the UI lane).
export type UiKind = 'confirm' | 'reject' | 'tick' | 'toggle-on' | 'toggle-off' | 'drag-start';

// A request in a compiled pattern, started `atMs` after `playSteps` is called.
export type CompiledStep = { atMs: number; request: EffectRequest };
