// Tests the guest-side bindings for the system-appearance plugin
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import { beforeEach, describe, expect, it, vi } from 'vitest';
import type { AppearancePreferences } from './bindings/AppearancePreferences';
import type { Palette } from './bindings/Palette';
import type { PaletteEntry } from './bindings/PaletteEntry';

const invokeMock = vi.fn();
const listenMock = vi.fn();

vi.mock('@tauri-apps/api/core', () => ({
	invoke: invokeMock,
}));

vi.mock('@tauri-apps/api/event', () => ({
	listen: listenMock,
}));

const preferences: AppearancePreferences = {
	revision: 3,
	colourScheme: 'dark',
	accent: '#3584e4',
	contrast: 'normal',
	reducedMotion: false,
	reducedTransparency: false,
	textScale: 1.25,
	iconTheme: 'Adwaita',
	sources: {
		colourScheme: 'portal',
		accent: 'portal',
		contrast: 'portal',
		reducedMotion: 'portal',
		reducedTransparency: null,
		textScale: 'portal',
		iconTheme: 'portal',
	},
};

function entry(colour: string): PaletteEntry {
	return { colour, source: 'gtkTheme', reason: null, detail: null };
}

const palette: Palette = {
	revision: 2,
	status: { available: true, source: 'gtkTheme', reason: null, detail: null },
	windowBackground: entry('#242424'),
	windowForeground: entry('#ffffff'),
	viewBackground: entry('#1e1e1e'),
	viewForeground: entry('#ffffff'),
	surfaceBackground: {
		colour: null,
		source: null,
		reason: 'sourceMissing',
		detail: 'the theme has no popover_bg_color or card_bg_color',
	},
	selectionBackground: entry('#3584e4'),
	selectionForeground: entry('#ffffff'),
	border: entry('#1b1b1b'),
	focus: entry('#3584e4'),
	warning: entry('#cd9309'),
	error: entry('#c01c28'),
	success: entry('#26a269'),
};

describe('system-appearance guest bindings', () => {
	beforeEach(() => {
		invokeMock.mockReset();
		listenMock.mockReset();
	});

	it('reads the appearance through the plugin command', async () => {
		const { getAppearance } = await import('./index');
		invokeMock.mockResolvedValue(preferences);

		await expect(getAppearance()).resolves.toEqual(preferences);
		expect(invokeMock).toHaveBeenCalledWith('plugin:system-appearance|get_appearance', undefined);
	});

	it('reads the status, which carries the availability of each appearance feature', async () => {
		const { getStatus } = await import('./index');
		const status = {
			available: true,
			reason: null,
			features: ['portal'],
			appearanceAvailable: true,
			appearance: [
				{
					feature: 'reducedTransparency',
					available: false,
					source: null,
					reason: 'noSource',
					detail: 'the portal has no reduced-transparency setting',
				},
			],
		};
		invokeMock.mockResolvedValue(status);

		await expect(getStatus()).resolves.toEqual(status);
		expect(invokeMock).toHaveBeenCalledWith('plugin:system-appearance|get_status', undefined);
	});

	it('listens for the appearance event and hands the payload to the callback', async () => {
		const { onAppearanceChanged, APPEARANCE_CHANGED_EVENT } = await import('./index');
		const unlisten = vi.fn();
		listenMock.mockResolvedValue(unlisten);
		const callback = vi.fn();

		const stop = await onAppearanceChanged(callback);

		expect(APPEARANCE_CHANGED_EVENT).toBe('system-appearance://appearance-changed');
		expect(listenMock).toHaveBeenCalledWith(APPEARANCE_CHANGED_EVENT, expect.any(Function));
		const handler = listenMock.mock.calls[0][1] as (_event: { payload: unknown }) => void;
		handler({ payload: preferences });
		expect(callback).toHaveBeenCalledWith(preferences);
		expect(stop).toBe(unlisten);
	});

	it('reads the palette through the plugin command', async () => {
		const { getPalette } = await import('./index');
		invokeMock.mockResolvedValue(palette);

		await expect(getPalette()).resolves.toEqual(palette);
		expect(invokeMock).toHaveBeenCalledWith('plugin:system-appearance|get_palette', undefined);
	});

	it('listens for the palette event and hands the payload to the callback', async () => {
		const { onPaletteChanged, PALETTE_CHANGED_EVENT } = await import('./index');
		const unlisten = vi.fn();
		listenMock.mockResolvedValue(unlisten);
		const callback = vi.fn();

		const stop = await onPaletteChanged(callback);

		expect(PALETTE_CHANGED_EVENT).toBe('system-appearance://palette-changed');
		expect(listenMock).toHaveBeenCalledWith(PALETTE_CHANGED_EVENT, expect.any(Function));
		const handler = listenMock.mock.calls[0][1] as (_event: { payload: unknown }) => void;
		handler({ payload: palette });
		expect(callback).toHaveBeenCalledWith(palette);
		expect(stop).toBe(unlisten);
	});
});
