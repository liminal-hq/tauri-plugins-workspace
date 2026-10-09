// Unit tests for the public haptics API, with the Tauri bridge mocked
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { envelopeDevice, pixel8Pro } from './pattern/__fixtures__/capabilities';
import { seedCues } from './pattern/__fixtures__/cues';
import { PATTERN_FORMAT } from '../../plugins/haptics/guest-js/pattern/types';
import type { Capabilities, PlayResult } from '../../plugins/haptics/guest-js/types';

const invoke = vi.fn();

// The plugin's `@tauri-apps/api` copy is not resolvable from here, so mock the bridge it calls.
(globalThis as unknown as { window: unknown }).window = {
	__TAURI_INTERNALS__: {
		invoke: (cmd: string, args?: unknown) => (args === undefined ? invoke(cmd) : invoke(cmd, args)),
	},
};

type Api = typeof import('../../plugins/haptics/guest-js/index');
let api: Api;
let caps: Capabilities;

function nativeResult(over: Partial<PlayResult> = {}): PlayResult {
	return { ok: true, tier: 3, target: 'phone', estimatedMs: 20, downgraded: false, ...over };
}

function commands(): string[] {
	return invoke.mock.calls.map((c) => c[0] as string);
}

beforeEach(async () => {
	vi.useFakeTimers();
	vi.resetModules();
	invoke.mockReset();
	caps = pixel8Pro;
	invoke.mockImplementation(async (cmd: string) => {
		if (cmd === 'plugin:haptics|capabilities') return caps;
		return nativeResult();
	});
	api = await import('../../plugins/haptics/guest-js/index');
});

afterEach(() => {
	vi.useRealTimers();
});

const click = {
	format: PATTERN_FORMAT,
	events: [{ type: 'transient', at: 0, intensity: 0.8, sharpness: 0.8 }],
} as const;

describe('capabilities', () => {
	it('reads once and caches', async () => {
		await api.capabilities();
		await api.capabilities();
		expect(commands().filter((c) => c.endsWith('capabilities'))).toHaveLength(1);
	});

	it('reads again on refresh', async () => {
		await api.capabilities();
		caps = { ...pixel8Pro, touchFeedbackEnabled: false };
		const fresh = await api.capabilities({ refresh: true });
		expect(fresh.touchFeedbackEnabled).toBe(false);
		expect((await api.capabilities()).touchFeedbackEnabled).toBe(false);
	});
});

describe('register and compile', () => {
	it('reports how a pattern compiles on this device', async () => {
		const report = await api.register('jump', { ...click });
		expect(report.id).toBe('jump');
		expect(report.tier).toBe(3);
	});

	it('rejects an invalid pattern with every problem listed', async () => {
		const bad = { ...click, events: [{ type: 'transient', at: 0, intensity: 2, sharpness: 5 }] };
		await expect(api.register('bad', bad as never)).rejects.toMatchObject({
			code: 'INVALID_EFFECT',
			message: expect.stringContaining('events[0].intensity: 2 is above 1'),
		});
	});

	it('compiles synchronously once capabilities are loaded, and previews any tier', async () => {
		await api.capabilities();
		expect(api.compile({ ...click }).tier).toBe(3);
		expect(api.compile({ ...click }, { tier: 2 }).tier).toBe(2);
	});

	it('refuses to compile before capabilities are loaded', () => {
		expect(() => api.compile({ ...click })).toThrow(/not loaded/);
	});

	it('registers a whole table', async () => {
		const reports = await api.registerAll({
			jump: seedCues.jump,
			hurt: { pattern: seedCues.hurt, options: { tier: 2 } },
		});
		expect(reports.jump.tier).toBe(3);
		expect(reports.hurt.tier).toBe(2);
	});
});

describe('trigger', () => {
	it('plays a registered pattern through play() and reports the policy', async () => {
		await api.register('jump', { ...click });
		const res = await api.trigger('jump');
		expect(commands()).toContain('plugin:haptics|play');
		expect(res.ok).toBe(true);
		expect(res.policy).toBe('played');
	});

	it('rejects an unknown id', async () => {
		await expect(api.trigger('nope')).rejects.toMatchObject({ code: 'UNKNOWN_PATTERN' });
	});

	it('forgets an unregistered pattern', async () => {
		await api.register('jump', { ...click });
		api.unregister('jump');
		await expect(api.trigger('jump')).rejects.toMatchObject({ code: 'UNKNOWN_PATTERN' });
	});

	it('multiplies the trigger scale with the master scale', async () => {
		await api.register('jump', { ...click });
		api.setMasterScale(0.5);
		await api.trigger('jump', { scale: 0.5 });
		const call = invoke.mock.calls.find((c) => c[0] === 'plugin:haptics|play');
		expect(call?.[1].req.effect.steps[0].scale).toBeCloseTo(0.2);
	});

	it('drops a second trigger while a drop-if-busy pattern plays', async () => {
		await api.register('fizz', { ...click, policy: 'drop-if-busy' });
		const first = await api.trigger('fizz');
		const second = await api.trigger('fizz');
		expect(first.policy).toBe('played');
		expect(second.policy).toBe('dropped');
		expect(commands().filter((c) => c.endsWith('|play'))).toHaveLength(1);
	});

	it('runs a mixed pattern as a scheduled step list', async () => {
		caps = {
			...pixel8Pro,
			primitives: {
				...pixel8Pro.primitives,
				spin: { supported: false, durationMs: null },
				quick_rise: { supported: false, durationMs: null },
			},
		};
		await api.register('mix', {
			format: PATTERN_FORMAT,
			events: [
				{ type: 'transient', at: 0, intensity: 0.8, sharpness: 0.8 },
				{ type: 'continuous', at: 100, duration: 120, intensity: 0.6, sharpness: 0.5 },
			],
		});
		const res = await api.trigger('mix');
		expect(commands()).toContain('plugin:haptics|play_steps');
		expect(res.reason).toContain('drops to tier 2');
	});

	it('resolves at tier 0 without calling native on a device with no vibrator', async () => {
		caps = { ...pixel8Pro, topTier: 0, hasVibrator: false };
		await api.register('jump', { ...click });
		const res = await api.trigger('jump');
		expect(res.tier).toBe(0);
		expect(commands().filter((c) => c.endsWith('|play'))).toHaveLength(0);
	});

	it('notes when setMaxTier lowered the tier', async () => {
		await api.register('jump', { ...click });
		api.setMaxTier(2);
		const res = await api.trigger('jump');
		expect(res.downgraded).toBe(true);
		expect(res.reason).toContain('Capped at tier 2 by setMaxTier');
	});
});

describe('review fixes', () => {
	it('registers a copy, so editing the original does not change the pattern', async () => {
		const original = JSON.parse(JSON.stringify(click));
		await api.register('copy', original);
		original.events[0].intensity = 0.1;
		await api.trigger('copy');
		const call = invoke.mock.calls.find((c) => c[0] === 'plugin:haptics|play');
		expect(call?.[1].req.effect.steps[0].scale).toBeCloseTo(0.8);
	});

	it('unregister cancels what that pattern still had queued', async () => {
		await api.register('q', { ...click, policy: 'queue' });
		await api.trigger('q');
		const waiting = api.trigger('q');
		await vi.advanceTimersByTimeAsync(0);
		api.unregister('q');
		expect((await waiting).policy).toBe('dropped');
	});

	it('does not play a trigger that was waiting on capabilities when stop() ran', async () => {
		await api.register('late', { ...click });
		const pending = api.trigger('late');
		await api.stop();
		const res = await pending;
		expect(res.policy).toBe('dropped');
		expect(commands().filter((c) => c.endsWith('|play'))).toHaveLength(0);
	});

	it('plays nothing at a master scale of 0, for raw plays and patterns alike', async () => {
		api.setMasterScale(0);
		await api.register('zero', { ...click });
		const pattern = await api.trigger('zero');
		const raw = await api.play({ effect: { type: 'oneshot', durationMs: 50, amplitude: 200 } });
		expect(pattern.tier).toBe(0);
		expect(raw.tier).toBe(0);
		expect(commands().filter((c) => c.endsWith('|play'))).toHaveLength(0);
	});

	it('plays no steps at a master scale of 0', async () => {
		api.setMasterScale(0);
		const res = await api.playSteps([
			{ atMs: 0, request: { effect: { type: 'oneshot', durationMs: 20, amplitude: 200 } } },
		]);
		expect(res.tier).toBe(0);
		expect(commands().filter((c) => c.endsWith('|play_steps'))).toHaveLength(0);
	});

	it('treats a non-finite trigger scale as full strength', async () => {
		await api.register('nan', { ...click });
		await api.trigger('nan', { scale: Number.NaN });
		const call = invoke.mock.calls.find((c) => c[0] === 'plugin:haptics|play');
		expect(call?.[1].req.effect.steps[0].scale).toBeCloseTo(0.8);
	});
});

describe('raw play', () => {
	const oneShot = { effect: { type: 'oneshot', durationMs: 50, amplitude: 200 } } as const;

	it('sends the request through unchanged by default', async () => {
		await api.play({ ...oneShot });
		expect(invoke).toHaveBeenCalledWith('plugin:haptics|play', { req: oneShot });
	});

	it('applies the master scale to amplitudes', async () => {
		api.setMasterScale(0.5);
		await api.play({ ...oneShot });
		expect(invoke.mock.calls.at(-1)?.[1].req.effect.amplitude).toBe(100);
	});

	it('resolves at tier 0 when the effect is above the max tier', async () => {
		caps = envelopeDevice;
		api.setMaxTier(2);
		const res = await api.play({
			effect: {
				type: 'composition',
				steps: [{ kind: 'primitive', primitive: 'click' }],
			},
		});
		expect(res).toMatchObject({ ok: true, tier: 0, reason: 'Capped at tier 2 by setMaxTier' });
		expect(commands().filter((c) => c.endsWith('|play'))).toHaveLength(0);
	});

	it('still plays an effect at or below the max tier', async () => {
		api.setMaxTier(3);
		await api.play({ ...oneShot });
		expect(commands()).toContain('plugin:haptics|play');
	});
});

describe('native errors', () => {
	it('wraps a rejected play call as INVALID_EFFECT with the native message', async () => {
		invoke.mockImplementation(async (cmd: string) => {
			if (cmd === 'plugin:haptics|capabilities') return caps;
			throw 'Unknown predefined effect `pop`';
		});
		await expect(
			api.play({ effect: { type: 'predefined', effectId: 'click' } })
		).rejects.toMatchObject({ code: 'INVALID_EFFECT', message: 'Unknown predefined effect `pop`' });
		await expect(api.ui('tick')).rejects.toMatchObject({ code: 'INVALID_EFFECT' });
	});
});

describe('playSteps', () => {
	it('applies the master scale to every step and sends them to native', async () => {
		api.setMasterScale(0.5);
		await api.playSteps([
			{ atMs: 0, request: { effect: { type: 'oneshot', durationMs: 20, amplitude: 200 } } },
			{ atMs: 80, request: { effect: { type: 'oneshot', durationMs: 20, amplitude: 100 } } },
		]);
		const [cmd, args] = invoke.mock.calls.at(-1) ?? [];
		expect(cmd).toBe('plugin:haptics|play_steps');
		expect(
			args.steps.map(
				(s: { request: { effect: { amplitude: number } } }) => s.request.effect.amplitude
			)
		).toEqual([100, 50]);
	});
});

describe('stop and ui', () => {
	it('cancels queued triggers and the motor', async () => {
		await api.register('q', { ...click, policy: 'queue' });
		await api.trigger('q');
		const waiting = api.trigger('q');
		await vi.advanceTimersByTimeAsync(0);
		await api.stop();
		expect((await waiting).policy).toBe('dropped');
		expect(commands()).toContain('plugin:haptics|stop');
	});

	it('passes the UI lane kind to native', async () => {
		await api.ui('toggle-on');
		expect(invoke).toHaveBeenCalledWith('plugin:haptics|ui', { kind: 'toggle-on' });
	});
});
