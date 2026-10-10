// Unit tests for the Gamepad API fallback, with a fake navigator and fake timers
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import {
	canRumble,
	playWeb,
	stopWeb,
	webPads,
} from '../../plugins/gamepad-haptics/guest-js/web-backend';

const playEffect = vi.fn(async () => 'complete');
const reset = vi.fn(async () => 'complete');
const pad = { id: 'x', index: 0, connected: true, vibrationActuator: { playEffect, reset } };

beforeEach(() => {
	vi.useFakeTimers();
	playEffect.mockClear();
	reset.mockClear();
	vi.stubGlobal('navigator', { getGamepads: () => [pad, null] });
});

afterEach(() => {
	vi.useRealTimers();
	vi.unstubAllGlobals();
});

describe('web backend', () => {
	it('lists connected pads and spots which can rumble', () => {
		expect(webPads()).toHaveLength(1);
		expect(canRumble(pad)).toBe(true);
		expect(canRumble({ id: 'y', index: 1 })).toBe(false);
	});

	it('lists nothing without a Gamepad API', () => {
		vi.stubGlobal('navigator', {});
		expect(webPads()).toEqual([]);
	});

	it('plays one dual-rumble effect per frame, scaled, and resets at the end', async () => {
		const total = await playWeb(
			pad,
			[
				{ durationMs: 40, heavy: 1, light: 0 },
				{ durationMs: 60, heavy: 0, light: 0.5 },
			],
			0.5
		);
		expect(total).toBe(100);
		expect(playEffect).toHaveBeenCalledTimes(1);
		expect(playEffect).toHaveBeenLastCalledWith('dual-rumble', {
			startDelay: 0,
			duration: 40,
			strongMagnitude: 0.5,
			weakMagnitude: 0,
		});

		await vi.advanceTimersByTimeAsync(40);
		expect(playEffect).toHaveBeenCalledTimes(2);
		expect(playEffect).toHaveBeenLastCalledWith('dual-rumble', {
			startDelay: 0,
			duration: 60,
			strongMagnitude: 0,
			weakMagnitude: 0.25,
		});

		await vi.advanceTimersByTimeAsync(200);
		expect(reset).toHaveBeenCalled();
	});

	it('stops a running pattern, cancelling the frames still to come', async () => {
		await playWeb(pad, [
			{ durationMs: 40, heavy: 1, light: 0 },
			{ durationMs: 60, heavy: 1, light: 1 },
		]);
		await stopWeb(0);
		await vi.advanceTimersByTimeAsync(200);
		expect(playEffect).toHaveBeenCalledTimes(1);
		expect(reset).toHaveBeenCalled();
	});

	it('refuses a pad with no actuator', async () => {
		await expect(playWeb({ id: 'y', index: 1 }, [])).rejects.toThrow('no vibration actuator');
	});
});
