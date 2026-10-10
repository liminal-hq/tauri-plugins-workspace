// Unit tests for the public gamepad API, with the Tauri bridge mocked
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { PATTERN_FORMAT } from '../../plugins/gamepad-haptics/guest-js/pattern/types';
import type { Pattern } from '../../plugins/gamepad-haptics/guest-js/pattern/types';
import type {
	Capabilities,
	PadInfo,
	PlayResult,
} from '../../plugins/gamepad-haptics/guest-js/types';

const invoke = vi.fn();

(globalThis as unknown as { window: unknown }).window = {
	__TAURI_INTERNALS__: {
		invoke: (cmd: string, args?: unknown) => (args === undefined ? invoke(cmd) : invoke(cmd, args)),
	},
};

type Api = typeof import('../../plugins/gamepad-haptics/guest-js/index');
let api: Api;
let caps: Capabilities;

const ds4 = (over: Partial<PadInfo> = {}): PadInfo => ({
	id: 'gamepad:0',
	slot: 0,
	name: 'Wireless Controller',
	vendorId: 0x054c,
	productId: 0x05c4,
	transport: 'usb',
	guid: 'g',
	motors: 2,
	triggers: false,
	lightBinary: false,
	weakHeavy: false,
	topTier: 2,
	backend: 'evdev',
	...over,
});

const tap: Pattern = {
	format: PATTERN_FORMAT,
	id: 'hit',
	events: [{ type: 'transient', at: 0, intensity: 1, sharpness: 1 }],
};

const played = (over: Partial<PlayResult> = {}): PlayResult => ({
	ok: true,
	tier: 2,
	target: 'gamepad:0',
	downgraded: false,
	...over,
});

const commands = () => invoke.mock.calls.map((c) => c[0] as string);

beforeEach(async () => {
	vi.useFakeTimers();
	vi.resetModules();
	invoke.mockReset();
	caps = {
		platform: 'linux',
		backend: 'evdev',
		limits: { maxDurationMs: 3000, maxContinuousMs: 2000 },
		pads: [ds4()],
	};
	invoke.mockImplementation(async (cmd: string) => {
		if (cmd === 'plugin:gamepad-haptics|capabilities') return caps;
		return played();
	});
	api = await import('../../plugins/gamepad-haptics/guest-js/index');
});

afterEach(() => {
	vi.useRealTimers();
	vi.unstubAllGlobals();
});

describe('createBackend', () => {
	it('registers a pattern and plays its compiled frames on the first native pad', async () => {
		const backend = api.createBackend();
		const report = await backend.register('hit', tap);
		expect(report.tier).toBe(1);

		const res = await backend.trigger('hit');
		expect(res.target).toBe('gamepad:0');
		const call = invoke.mock.calls.find((c) => c[0] === 'plugin:gamepad-haptics|play_frames');
		expect(call?.[1]).toEqual({
			args: {
				padId: 'gamepad:0',
				frames: [{ durationMs: 40, heavy: 0, light: 1 }],
				scale: undefined,
			},
		});
	});

	it('forwards the trigger scale multiplied by the master scale', async () => {
		const backend = api.createBackend();
		await backend.register('hit', tap);
		backend.setMasterScale(0.5);
		await backend.trigger('hit', { scale: 0.5 });
		const call = invoke.mock.calls.find((c) => c[0] === 'plugin:gamepad-haptics|play_frames');
		expect((call?.[1] as { args: { scale: number } }).args.scale).toBe(0.25);
	});

	it('holds the scale it forwards to 0..1', async () => {
		const backend = api.createBackend();
		await backend.register('hit', tap);
		await backend.trigger('hit', { scale: 3 });
		const call = invoke.mock.calls.find((c) => c[0] === 'plugin:gamepad-haptics|play_frames');
		expect((call?.[1] as { args: { scale?: number } }).args.scale).toBeUndefined();
	});

	it('rejects a pattern that breaks the format at register time', async () => {
		const backend = api.createBackend();
		await expect(backend.register('bad', { ...tap, events: [] })).rejects.toThrow();
	});

	it('compiles for a lower tier when setMaxTier is used', async () => {
		caps.pads = [ds4({ topTier: 3, triggers: true })];
		const backend = api.createBackend();
		await backend.register('hit', tap);
		backend.setMaxTier(1);
		await backend.trigger('hit');
		const call = invoke.mock.calls.find((c) => c[0] === 'plugin:gamepad-haptics|play_frames');
		const frames = (call?.[1] as { args: { frames: { light: number; rightTrigger?: number }[] } })
			.args.frames;
		expect(frames[0].light).toBe(0);
		expect(frames[0].rightTrigger).toBeUndefined();
	});

	it('returns a silent result for a pad it cannot play, without calling native', async () => {
		caps.pads = [ds4({ topTier: 0, reason: 'No write access' })];
		const backend = api.createBackend();
		await backend.register('hit', tap);
		const res = await backend.trigger('hit', { padId: 'gamepad:0' });
		expect(res.tier).toBe(0);
		expect(commands()).not.toContain('plugin:gamepad-haptics|play_frames');
	});

	it('asks the caller to identify when a hint fits two pads', async () => {
		caps.pads = [ds4(), ds4({ id: 'gamepad:1', slot: 1 })];
		const backend = api.createBackend();
		await backend.register('hit', tap);
		const res = await backend.trigger('hit', { hint: { vendorId: 0x054c, productId: 0x05c4 } });
		expect(res.tier).toBe(0);
		expect(res.reason).toContain('identify');
		expect(commands()).not.toContain('plugin:gamepad-haptics|play_frames');
	});

	it('falls back to the Gamepad API when no native pad is present', async () => {
		caps.pads = [];
		const playEffect = vi.fn(async () => 'complete');
		vi.stubGlobal('navigator', {
			getGamepads: () => [
				{
					id: 'x',
					index: 3,
					connected: true,
					vibrationActuator: { playEffect, reset: vi.fn(async () => 'complete') },
				},
			],
		});
		const backend = api.createBackend();
		await backend.register('hit', tap);
		const res = await backend.trigger('hit');
		expect(res.target).toBe('web:3');
		expect(playEffect).toHaveBeenCalled();
		expect(commands()).not.toContain('plugin:gamepad-haptics|play_frames');
	});

	it('reports no gamepad when nothing can play', async () => {
		caps.pads = [];
		vi.stubGlobal('navigator', { getGamepads: () => [] });
		const backend = api.createBackend();
		await backend.register('hit', tap);
		const res = await backend.trigger('hit');
		expect(res).toMatchObject({ tier: 0, reason: 'No gamepad found' });
	});

	it('stops native and web playback together', async () => {
		const backend = api.createBackend();
		await backend.stop();
		expect(commands()).toContain('plugin:gamepad-haptics|stop');
	});
});
