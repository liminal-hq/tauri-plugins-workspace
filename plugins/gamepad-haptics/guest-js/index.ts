// Typed guest-side wrappers for the gamepad-haptics plugin
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import { invoke } from '@tauri-apps/api/core';
import { listen, type UnlistenFn } from '@tauri-apps/api/event';

const PREFIX = 'plugin:gamepad-haptics|';

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
	topTier: 0 | 1 | 2 | 3;
	reason?: string;
	backend: string;
}

/** Motor levels, 0 to 1, held for `durationMs`. */
export interface Frame {
	durationMs: number;
	heavy: number;
	light: number;
	leftTrigger?: number;
	rightTrigger?: number;
}

export interface PlayResult {
	ok: true;
	tier: number;
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

export function capabilities(): Promise<Capabilities> {
	return invoke<Capabilities>(`${PREFIX}capabilities`);
}

export function listPads(): Promise<PadInfo[]> {
	return invoke<PadInfo[]>(`${PREFIX}list_pads`);
}

export function playFrames(padId: string, frames: Frame[]): Promise<PlayResult> {
	return invoke<PlayResult>(`${PREFIX}play_frames`, { args: { padId, frames } });
}

/** Stops one pad, or every pad when `padId` is omitted. */
export function stop(padId?: string): Promise<void> {
	return invoke<void>(`${PREFIX}stop`, { padId });
}

/** Buzzes one pad in a pattern that tells it from the others, so a player can confirm which it is. */
export function identify(padId: string): Promise<PlayResult> {
	return invoke<PlayResult>(`${PREFIX}identify`, { padId });
}

export const PAD_CONNECTED_EVENT = 'gamepad-haptics://connected';
export const PAD_CHANGED_EVENT = 'gamepad-haptics://changed';
export const PAD_DISCONNECTED_EVENT = 'gamepad-haptics://disconnected';

export function onPadConnected(handler: (_pad: PadInfo) => void): Promise<UnlistenFn> {
	return listen<PadInfo>(PAD_CONNECTED_EVENT, (e) => handler(e.payload));
}

export function onPadChanged(handler: (_pad: PadInfo) => void): Promise<UnlistenFn> {
	return listen<PadInfo>(PAD_CHANGED_EVENT, (e) => handler(e.payload));
}

export function onPadDisconnected(
	handler: (_gone: { id: string; slot: number }) => void
): Promise<UnlistenFn> {
	return listen<{ id: string; slot: number }>(PAD_DISCONNECTED_EVENT, (e) => handler(e.payload));
}
