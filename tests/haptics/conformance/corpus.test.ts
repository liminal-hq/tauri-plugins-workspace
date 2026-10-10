// Checks the shared request corpus against the guest: the rules mirror and what is forwarded
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import { readFileSync } from 'node:fs';
import { URL } from 'node:url';
import { beforeEach, describe, expect, it, vi } from 'vitest';
import { pixel8Pro } from '../pattern/__fixtures__/capabilities';
import {
	EFFECT_IDS,
	MAX_STEPS,
	PRIMITIVE_IDS,
} from '../../../plugins/haptics/guest-js/pattern/tables';
import type { CompiledStep, EffectRequest } from '../../../plugins/haptics/guest-js/types';
import { checkRequest, checkSteps } from './request-rules';

type Case = {
	name: string;
	command: 'play' | 'play_steps';
	request?: EffectRequest;
	steps?: CompiledStep[];
	controls?: { scale?: number; maxTier?: 0 | 1 | 2 | 3 | 4 };
	limits?: { maxDurationMs?: number };
	expect: { ok: unknown } | { error: string };
};

const corpus = JSON.parse(
	readFileSync(
		new URL('../../../plugins/haptics/tests/conformance/requests.json', import.meta.url),
		'utf8'
	)
) as {
	constants: { maxSteps: number; effectIds: string[]; primitiveIds: string[] };
	defaults: { limits: { maxDurationMs: number } };
	cases: Case[];
};

const invoke = vi.fn();
(globalThis as unknown as { window: unknown }).window = {
	__TAURI_INTERNALS__: {
		invoke: (cmd: string, args?: unknown) => (args === undefined ? invoke(cmd) : invoke(cmd, args)),
	},
};

type Api = typeof import('../../../plugins/haptics/guest-js/index');
let api: Api;

beforeEach(async () => {
	vi.resetModules();
	invoke.mockReset();
	invoke.mockImplementation(async (cmd: string) =>
		cmd === 'plugin:haptics|capabilities'
			? pixel8Pro
			: { ok: true, tier: 0, target: 'phone', estimatedMs: 0, downgraded: false }
	);
	api = await import('../../../plugins/haptics/guest-js/index');
});

describe('corpus constants', () => {
	it('match the guest tables', () => {
		expect(corpus.constants.maxSteps).toBe(MAX_STEPS);
		expect(corpus.constants.effectIds).toEqual(EFFECT_IDS);
		expect(corpus.constants.primitiveIds).toEqual(PRIMITIVE_IDS);
	});
});

describe('request rules mirror', () => {
	it.each(corpus.cases)('agrees with the plugin on $name', (c) => {
		const limits = { ...corpus.defaults.limits, ...c.limits };
		const reason =
			c.command === 'play'
				? checkRequest(c.request as EffectRequest, limits.maxDurationMs)
				: checkSteps(c.steps as CompiledStep[], limits);

		if ('error' in c.expect) {
			expect(reason, c.name).not.toBeNull();
			expect(reason).toContain(c.expect.error);
		} else {
			expect(reason, c.name).toBeNull();
		}
	});
});

describe('the guest forwards requests untouched', () => {
	it.each(corpus.cases)('sends $name as it was given, plus the controls', async (c) => {
		if (c.controls?.scale !== undefined) api.setMasterScale(c.controls.scale);
		if (c.controls?.maxTier !== undefined) api.setMaxTier(c.controls.maxTier);

		const sent = c.command === 'play' ? { req: c.request } : { steps: c.steps };
		const controls = {
			...(c.controls?.scale !== undefined && c.controls.scale !== 1
				? { scale: c.controls.scale }
				: {}),
			...(c.controls?.maxTier !== undefined ? { maxTier: c.controls.maxTier } : {}),
		};

		if (c.command === 'play') await api.play(c.request as EffectRequest);
		else await api.playSteps(c.steps as CompiledStep[]);

		expect(invoke).toHaveBeenCalledTimes(1);
		expect(invoke).toHaveBeenCalledWith(`plugin:haptics|${c.command}`, { ...sent, ...controls });
	});
});
