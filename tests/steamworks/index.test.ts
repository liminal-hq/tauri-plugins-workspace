// Unit tests for the steamworks guest API, with the Tauri bridge mocked
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import { beforeEach, describe, expect, it, vi } from 'vitest';

const invoke = vi.fn();

(globalThis as unknown as { window: unknown }).window = {
	__TAURI_INTERNALS__: {
		invoke: (cmd: string, args?: unknown, options?: unknown) => {
			if (options !== undefined) return invoke(cmd, args, options);
			return args === undefined ? invoke(cmd) : invoke(cmd, args);
		},
	},
};

type Api = typeof import('../../plugins/steamworks/guest-js/index');
let api: Api;

beforeEach(async () => {
	invoke.mockReset();
	api = await import('../../plugins/steamworks/guest-js/index');
});

describe('commands', () => {
	it('sends the exact command names and argument shapes', async () => {
		invoke.mockResolvedValue(undefined);

		await api.unlockAchievement('ACH_ONE');
		await api.clearAchievement('ACH_ONE');
		await api.indicateProgress('ACH_COUNT', 3, 10);
		await api.getAchievement('ACH_ONE');
		await api.storeStats();

		expect(invoke.mock.calls).toEqual([
			['plugin:steamworks|unlock_achievement', { id: 'ACH_ONE' }],
			['plugin:steamworks|clear_achievement', { id: 'ACH_ONE' }],
			['plugin:steamworks|indicate_achievement_progress', { id: 'ACH_COUNT', current: 3, max: 10 }],
			['plugin:steamworks|get_achievement', { id: 'ACH_ONE' }],
			['plugin:steamworks|store_stats', {}],
		]);
	});

	it('reads availability, status and the player without arguments', async () => {
		invoke.mockResolvedValue(true);
		await api.isAvailable();
		await api.getStatus();
		await api.getPlayer();
		expect(invoke.mock.calls.map((c) => c[0])).toEqual([
			'plugin:steamworks|is_available',
			'plugin:steamworks|get_status',
			'plugin:steamworks|get_player',
		]);
	});
});

describe('presence and overlay', () => {
	it('sends presence entries, with null removing a key', async () => {
		invoke.mockResolvedValue(undefined);
		await api.setRichPresence({ status: 'Mayor of Fizzton', connect: null });
		await api.clearRichPresence();
		expect(invoke.mock.calls).toEqual([
			[
				'plugin:steamworks|set_rich_presence',
				{ entries: { status: 'Mayor of Fizzton', connect: null } },
			],
			['plugin:steamworks|clear_rich_presence', {}],
		]);
	});

	it('opens overlay pages and web pages', async () => {
		invoke.mockResolvedValue(undefined);
		await api.activateOverlay('achievements');
		await api.activateOverlayToWebPage('https://store.steampowered.com/app/480');
		expect(invoke.mock.calls).toEqual([
			['plugin:steamworks|activate_overlay', { page: 'achievements' }],
			[
				'plugin:steamworks|activate_overlay_to_web_page',
				{ url: 'https://store.steampowered.com/app/480' },
			],
		]);
	});
});

describe('cloud files', () => {
	it('sends bytes as the body and the name in a header', async () => {
		invoke.mockResolvedValue(undefined);
		await api.cloudWrite('saves/city.sav', new Uint8Array([1, 2, 3]));
		await api.cloudWrite('notes.txt', 'héllo');

		expect(invoke.mock.calls[0]).toEqual([
			'plugin:steamworks|cloud_write',
			new Uint8Array([1, 2, 3]),
			{ headers: { 'x-steamworks-cloud-name': 'saves/city.sav' } },
		]);
		const [, text, options] = invoke.mock.calls[1];
		expect(new TextDecoder().decode(text as Uint8Array)).toBe('héllo');
		expect(options).toEqual({ headers: { 'x-steamworks-cloud-name': 'notes.txt' } });
	});

	it('reads bytes and text back', async () => {
		invoke.mockResolvedValue(new TextEncoder().encode('héllo').buffer);
		expect(await api.cloudReadText('notes.txt')).toBe('héllo');
		expect(invoke).toHaveBeenCalledWith('plugin:steamworks|cloud_read', { name: 'notes.txt' });

		invoke.mockResolvedValue([9, 8]);
		expect(await api.cloudRead('a')).toEqual(new Uint8Array([9, 8]));
	});

	it('lists, deletes and reads quota and status', async () => {
		invoke.mockResolvedValue(undefined);
		await api.cloudList();
		await api.cloudDelete('a');
		await api.cloudQuota();
		await api.cloudStatus();
		expect(invoke.mock.calls).toEqual([
			['plugin:steamworks|cloud_list', {}],
			['plugin:steamworks|cloud_delete', { name: 'a' }],
			['plugin:steamworks|cloud_quota', {}],
			['plugin:steamworks|cloud_status', {}],
		]);
	});
});

describe('stats', () => {
	it('sends whole numbers as ints and fractions as floats by default', async () => {
		invoke.mockResolvedValue(undefined);
		await api.setStat('NumGames', 4);
		await api.setStat('Distance', 2.5);
		expect(invoke.mock.calls).toEqual([
			['plugin:steamworks|set_stat', { name: 'NumGames', value: { kind: 'int', value: 4 } }],
			['plugin:steamworks|set_stat', { name: 'Distance', value: { kind: 'float', value: 2.5 } }],
		]);
	});

	it('lets a float stat hold a whole number when the kind is given', async () => {
		invoke.mockResolvedValue(undefined);
		await api.setStat('Distance', 3, 'float');
		expect(invoke).toHaveBeenCalledWith('plugin:steamworks|set_stat', {
			name: 'Distance',
			value: { kind: 'float', value: 3 },
		});
	});

	it('unwraps the stat value and defaults to int', async () => {
		invoke.mockResolvedValue({ kind: 'int', value: 9 });
		expect(await api.getStat('NumGames')).toBe(9);
		expect(invoke).toHaveBeenCalledWith('plugin:steamworks|get_stat', {
			name: 'NumGames',
			kind: 'int',
		});
	});
});

describe('isUnavailableError', () => {
	it('recognises the stable prefix on strings and errors', () => {
		expect(api.isUnavailableError('unavailable: steamNotRunning')).toBe(true);
		expect(api.isUnavailableError(new Error('unavailable: notCompiled'))).toBe(true);
	});

	it('does not mistake other failures for unavailability', () => {
		expect(api.isUnavailableError('unknown achievement `X`')).toBe(false);
		expect(api.isUnavailableError(undefined)).toBe(false);
	});
});

describe('events', () => {
	it('uses the documented event names', () => {
		expect(api.STATS_RECEIVED_EVENT).toBe('steamworks://stats-received');
		expect(api.STATS_STORED_EVENT).toBe('steamworks://stats-stored');
		expect(api.ACHIEVEMENT_STORED_EVENT).toBe('steamworks://achievement-stored');
		expect(api.OVERLAY_TOGGLED_EVENT).toBe('steamworks://overlay-toggled');
	});
});
