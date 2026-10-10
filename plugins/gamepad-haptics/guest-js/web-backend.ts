// Plays frames on a pad through the webview's Gamepad API, for when no native path can address it
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import type { MotorFrame } from './pattern/pad-compile';

type Actuator = {
	playEffect?: (type: string, params: Record<string, number>) => Promise<unknown>;
	reset?: () => Promise<unknown>;
};

type WebPad = {
	id: string;
	index: number;
	connected?: boolean;
	vibrationActuator?: Actuator | null;
};

/** The pads the webview exposes. The browser lists none until a button is pressed on the page. */
export function webPads(): WebPad[] {
	if (typeof navigator === 'undefined' || typeof navigator.getGamepads !== 'function') return [];
	return Array.from(navigator.getGamepads() as ArrayLike<WebPad | null>).filter(
		(p): p is WebPad => p !== null && p.connected !== false
	);
}

/** Whether a web pad can rumble: it needs a `dual-rumble` actuator. */
export function canRumble(pad: WebPad): boolean {
	return typeof pad.vibrationActuator?.playEffect === 'function';
}

const running = new Map<number, ReturnType<typeof setTimeout>[]>();

/** Stops one web pad, or every pad when `index` is omitted. */
export async function stopWeb(index?: number): Promise<void> {
	const targets = index === undefined ? [...running.keys()] : [index];
	for (const i of targets) {
		for (const timer of running.get(i) ?? []) clearTimeout(timer);
		running.delete(i);
		const pad = webPads().find((p) => p.index === i);
		await pad?.vibrationActuator?.reset?.().catch(() => undefined);
	}
}

/**
 * Plays `frames` on a web pad: one `dual-rumble` effect per frame, each started on its own timer
 * so a pad that drops one still ends silent. Returns the time the pattern takes.
 */
export async function playWeb(pad: WebPad, frames: MotorFrame[], scale = 1): Promise<number> {
	const actuator = pad.vibrationActuator;
	if (!actuator?.playEffect) throw new Error('This pad has no vibration actuator');
	await stopWeb(pad.index);

	const timers: ReturnType<typeof setTimeout>[] = [];
	running.set(pad.index, timers);
	let at = 0;
	for (const frame of frames) {
		const params = {
			startDelay: 0,
			duration: frame.durationMs,
			strongMagnitude: Math.min(1, frame.heavy * scale),
			weakMagnitude: Math.min(1, frame.light * scale),
		};
		if (at === 0) {
			await actuator.playEffect('dual-rumble', params);
		} else {
			timers.push(setTimeout(() => void actuator.playEffect?.('dual-rumble', params), at));
		}
		at += frame.durationMs;
	}
	timers.push(setTimeout(() => void stopWeb(pad.index), at + 50));
	return at;
}
