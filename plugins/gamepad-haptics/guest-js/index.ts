// Typed guest-side wrappers for the gamepad-haptics plugin
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import { invoke } from '@tauri-apps/api/core';
import { listen, type UnlistenFn } from '@tauri-apps/api/event';
import { resolvePad } from './identity';
import type { PadHint } from './identity';
import { compilePad } from './pattern/pad-compile';
import type { PadCompileReport } from './pattern/pad-compile';
import { PatternScheduler } from './pattern/schedule';
import type { Pattern, Tier } from './pattern/types';
import { formatIssues, validatePattern } from './pattern/validate';
import type { Capabilities, Frame, PadInfo, PlayResult } from './types';
import { canRumble, playWeb, stopWeb, webPads } from './web-backend';

export type { PadHint, Resolved } from './identity';
export { parseWebGamepadId, resolvePad } from './identity';
export type { MotorFrame, PadCompileReport } from './pattern/pad-compile';
export type { Pattern, PatternEvent, Policy, Tier } from './pattern/types';
export { PATTERN_FORMAT } from './pattern/types';
export type { Capabilities, Frame, PadInfo, PlayResult, Transport } from './types';

const PREFIX = 'plugin:gamepad-haptics|';

export const PAD_CONNECTED_EVENT = 'gamepad-haptics://connected';
export const PAD_CHANGED_EVENT = 'gamepad-haptics://changed';
export const PAD_DISCONNECTED_EVENT = 'gamepad-haptics://disconnected';

export function capabilities(): Promise<Capabilities> {
	return invoke<Capabilities>(`${PREFIX}capabilities`);
}

export function listPads(): Promise<PadInfo[]> {
	return invoke<PadInfo[]>(`${PREFIX}list_pads`);
}

/**
 * Plays motor frames on one native pad. Rust validates them first, so a bad request is rejected
 * even when `scale` is 0 or the pad cannot play. `scale` multiplies with the configured master scale.
 */
export function playFrames(padId: string, frames: Frame[], scale?: number): Promise<PlayResult> {
	return invoke<PlayResult>(`${PREFIX}play_frames`, { args: { padId, frames, scale } });
}

/** Buzzes one pad in a pattern that tells it from the others, so a player can confirm which it is. */
export function identify(padId: string): Promise<PlayResult> {
	return invoke<PlayResult>(`${PREFIX}identify`, { padId });
}

/** Stops one pad, or every pad when `padId` is omitted. */
export async function stop(padId?: string): Promise<void> {
	await invoke<void>(`${PREFIX}stop`, { padId });
	if (padId === undefined) await stopWeb();
}

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

export type TriggerOptions = {
	/** 0..1, multiplies with the master scale. */
	scale?: number;
	/** Play on this native pad (`gamepad:N`). */
	padId?: string;
	/** Find the pad from something the game knows, such as a Web `Gamepad`. */
	hint?: PadHint;
};

/** A backend that plays registered patterns on gamepads, with the same shape the phone plugin uses. */
export type GamepadBackend = {
	readonly id: 'gamepad';
	capabilities(): Promise<Capabilities>;
	register(id: string, pattern: Pattern): Promise<PadCompileReport>;
	trigger(id: string, options?: TriggerOptions): Promise<PlayResult>;
	setMasterScale(value: number): void;
	setMaxTier(tier: Tier | null): void;
	stop(): Promise<void>;
};

const silent = (target: string, reason: string): PlayResult => ({
	ok: true,
	tier: 0,
	target,
	downgraded: true,
	reason,
});

/**
 * Creates a backend that plays on a native pad when the plugin can address one and falls back to the
 * webview's Gamepad API when it cannot. One pad is never driven by both: a pad found natively is
 * played natively. Do not call `playEffect` on the same pad yourself while this backend plays.
 */
export function createBackend(): GamepadBackend {
	const patterns = new Map<string, Pattern>();
	const scheduler = new PatternScheduler();
	let masterScale = 1;
	let maxTier: Tier | null = null;

	async function limitsAndPads(): Promise<Capabilities> {
		return capabilities();
	}

	function compileFor(pattern: Pattern, topTier: Tier, maxDurationMs: number): PadCompileReport {
		return compilePad(pattern, { topTier, maxDurationMs }, maxTier === null ? {} : { maxTier });
	}

	return {
		id: 'gamepad',
		capabilities: limitsAndPads,

		async register(id, pattern) {
			const caps = await limitsAndPads();
			const issues = validatePattern(pattern, { maxDurationMs: caps.limits.maxDurationMs });
			if (issues.length > 0) throw new Error(formatIssues(issues));
			patterns.set(id, pattern);
			const best = caps.pads.reduce<Tier>((t, p) => Math.max(t, p.topTier) as Tier, 0);
			return compileFor(pattern, best === 0 ? 2 : best, caps.limits.maxDurationMs);
		},

		async trigger(id, options = {}) {
			const pattern = patterns.get(id);
			if (!pattern) throw new Error(`Unknown pattern \`${id}\`. Register it first.`);
			const caps = await limitsAndPads();
			// Rust rejects a scale over 1, so the product is held to 0..1 however the caller scaled it.
			const scale = Math.min(1, Math.max(0, (options.scale ?? 1) * masterScale));
			const policy = pattern.policy ?? 'interrupt';

			// Choose the pad: an id, a hint, or the first native pad that can play.
			let native: PadInfo | undefined;
			let hintedWeb: number | undefined;
			if (options.padId !== undefined) {
				native = caps.pads.find((p) => p.id === options.padId);
				if (!native) return silent(options.padId, `No pad \`${options.padId}\``);
			} else if (options.hint !== undefined) {
				const found = resolvePad(caps.pads, options.hint);
				if (found.ambiguous) {
					return silent(
						'gamepad',
						'More than one pad fits; call identify() and pass the padId the player confirms'
					);
				}
				native = found.matches[0];
				if (!native && 'gamepad' in options.hint) hintedWeb = options.hint.gamepad.index;
			} else {
				native = caps.pads.find((p) => p.topTier > 0);
			}

			if (native) {
				const report = compileFor(pattern, native.topTier, caps.limits.maxDurationMs);
				if (report.frames.length === 0) {
					return silent(native.id, report.notes[0] ?? 'Nothing to play');
				}
				const padId = native.id;
				const outcome = await scheduler.submit<PlayResult>({
					key: id,
					policy,
					estimatedMs: report.estimatedMs,
					scale,
					run: (s) => playFrames(padId, report.frames, s === 1 ? undefined : s),
					didPlay: (res) => res.tier > 0,
				});
				return outcome.result ?? silent(padId, `Not played: ${outcome.policy}`);
			}

			// No native pad: use the webview's Gamepad API.
			const web = webPads().filter(canRumble);
			const pad = hintedWeb === undefined ? web[0] : web.find((p) => p.index === hintedWeb);
			if (!pad) return silent('gamepad', 'No gamepad found');
			const report = compileFor(pattern, 2, caps.limits.maxDurationMs);
			if (report.frames.length === 0)
				return silent(`web:${pad.index}`, report.notes[0] ?? 'Nothing to play');
			const outcome = await scheduler.submit<PlayResult>({
				key: id,
				policy,
				estimatedMs: report.estimatedMs,
				scale,
				run: async (s) => {
					await playWeb(pad, report.frames, s);
					return { ok: true, tier: report.tier, target: `web:${pad.index}`, downgraded: false };
				},
				didPlay: (res) => res.tier > 0,
			});
			return outcome.result ?? silent(`web:${pad.index}`, `Not played: ${outcome.policy}`);
		},

		setMasterScale(value) {
			masterScale = Math.min(1, Math.max(0, value));
		},

		setMaxTier(tier) {
			maxTier = tier;
		},

		async stop() {
			scheduler.stop();
			await stop();
		},
	};
}
