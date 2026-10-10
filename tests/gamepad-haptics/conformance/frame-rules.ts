// A test-only copy of the Rust frame rules, pinned to the shared corpus so the two cannot drift
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import type { MotorFrame } from '../../../plugins/gamepad-haptics/guest-js/pattern/pad-compile';

export const MAX_FRAMES = 512;

export type Limits = { maxDurationMs: number; maxContinuousMs: number };

const level = (v: unknown): v is number => typeof v === 'number' && v >= 0 && v <= 1;

/** The first problem the Rust validator would report for these frames, or undefined when valid. */
export function checkFrames(frames: MotorFrame[], limits: Limits): string | undefined {
	if (frames.length === 0) return 'frames cannot be empty';
	if (frames.length > MAX_FRAMES) return `frames exceeds the maximum of ${MAX_FRAMES}`;
	let total = 0;
	for (let i = 0; i < frames.length; i++) {
		const f = frames[i];
		if (!Number.isInteger(f.durationMs) || f.durationMs <= 0) {
			return `frames[${i}]: durationMs must be positive`;
		}
		for (const [name, v] of [
			['heavy', f.heavy],
			['light', f.light],
			['leftTrigger', f.leftTrigger],
			['rightTrigger', f.rightTrigger],
		] as const) {
			if (v !== undefined && !level(v)) return `frames[${i}]: ${name} must be within 0..1`;
		}
		total += f.durationMs;
	}
	if (total > limits.maxDurationMs) {
		return `frames last ${total} ms, over the limit of ${limits.maxDurationMs} ms`;
	}
	return undefined;
}

export function checkScale(scale: number | undefined): string | undefined {
	return scale !== undefined && !level(scale) ? 'scale must be within 0..1' : undefined;
}
