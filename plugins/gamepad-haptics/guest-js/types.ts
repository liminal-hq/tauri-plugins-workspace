// Types shared by the guest API, identity matching and the web fallback
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import type { MotorFrame } from './pattern/pad-compile';
import type { Tier } from './pattern/types';

export type Transport = 'usb' | 'bluetooth' | 'unknown';

export interface PadInfo {
	id: string;
	slot: number;
	name: string;
	vendorId: number;
	productId: number;
	serial?: string;
	transport: Transport;
	guid: string;
	motors: 0 | 1 | 2;
	triggers: boolean;
	/** The light motor only switches on and off, so the plugin pulses it to approximate strengths. */
	lightBinary: boolean;
	topTier: Tier;
	reason?: string;
	backend: string;
}

/** Motor levels, 0 to 1, held for `durationMs`. */
export type Frame = MotorFrame;

export interface PlayResult {
	ok: true;
	tier: number;
	/** The pad that played, `gamepad:N` for a native pad or `web:N` for a webview pad. */
	target: string;
	downgraded: boolean;
	reason?: string;
}

export interface Capabilities {
	platform: string;
	backend: string;
	limits: { maxDurationMs: number; maxContinuousMs: number };
	pads: PadInfo[];
}
