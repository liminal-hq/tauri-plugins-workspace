// Tests the guest-side bindings for the desktop-integration plugin
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import { beforeEach, describe, expect, it, vi } from 'vitest';

const invokeMock = vi.fn();
const listenMock = vi.fn();

vi.mock('@tauri-apps/api/core', () => ({
	invoke: invokeMock,
}));

vi.mock('@tauri-apps/api/event', () => ({
	listen: listenMock,
}));

// Module-level state in index.ts (activationCallback/activationListening) persists
// across imports within the same module instance — reset modules and re-import fresh
// for every test so each one starts from a clean slate.
async function freshDesktopIntegration() {
	vi.resetModules();
	const mod = await import('./index');
	return mod.desktopIntegration;
}

describe('desktopIntegration', () => {
	beforeEach(() => {
		invokeMock.mockReset();
		listenMock.mockReset();
		listenMock.mockResolvedValue(() => {});
	});

	describe('registerShortcut', () => {
		it('invokes register_shortcut with the session and shortcut details', async () => {
			const desktopIntegration = await freshDesktopIntegration();
			invokeMock.mockResolvedValue(undefined);

			await desktopIntegration.registerShortcut(
				'emoji-nook-toggle',
				'Toggle Emoji Nook',
				'Alt+Shift+E',
				() => {}
			);

			expect(invokeMock).toHaveBeenCalledWith('plugin:desktop-integration|register_shortcut', {
				sessionId: 'emoji-nook-toggle',
				sessionDescription: 'Toggle Emoji Nook',
				shortcut: 'Alt+Shift+E',
			});
		});

		it('sets up the activation listener only once across repeated calls', async () => {
			const desktopIntegration = await freshDesktopIntegration();
			invokeMock.mockResolvedValue(undefined);

			await desktopIntegration.registerShortcut('a', 'A', 'Alt+A', () => {});
			await desktopIntegration.registerShortcut('b', 'B', 'Alt+B', () => {});
			await desktopIntegration.registerShortcut('c', 'C', 'Alt+C', () => {});

			expect(listenMock).toHaveBeenCalledTimes(1);
			expect(listenMock).toHaveBeenCalledWith('shortcut-activated', expect.any(Function));
		});

		it('invokes the latest onActivated callback when the event fires', async () => {
			const desktopIntegration = await freshDesktopIntegration();
			invokeMock.mockResolvedValue(undefined);

			const firstCallback = vi.fn();
			const secondCallback = vi.fn();

			await desktopIntegration.registerShortcut('a', 'A', 'Alt+A', firstCallback);
			await desktopIntegration.registerShortcut('a', 'A', 'Alt+B', secondCallback);

			// The listener registered on the first call — grab the handler it was given.
			const activationHandler = listenMock.mock.calls[0][1];
			activationHandler({ payload: { sessionId: 'a' } });

			expect(firstCallback).not.toHaveBeenCalled();
			expect(secondCallback).toHaveBeenCalledTimes(1);
		});
	});

	describe('checkShortcutBindingComplete', () => {
		it('invokes check_shortcut_binding_complete and returns the result', async () => {
			const desktopIntegration = await freshDesktopIntegration();
			invokeMock.mockResolvedValue(true);

			const result = await desktopIntegration.checkShortcutBindingComplete();

			expect(invokeMock).toHaveBeenCalledWith(
				'plugin:desktop-integration|check_shortcut_binding_complete',
				undefined
			);
			expect(result).toBe(true);
		});
	});

	describe('checkShortcutBindingError', () => {
		it('invokes check_shortcut_binding_error and returns the result', async () => {
			const desktopIntegration = await freshDesktopIntegration();
			invokeMock.mockResolvedValue('compositor returned no bound shortcuts');

			const result = await desktopIntegration.checkShortcutBindingError();

			expect(invokeMock).toHaveBeenCalledWith(
				'plugin:desktop-integration|check_shortcut_binding_error',
				undefined
			);
			expect(result).toBe('compositor returned no bound shortcuts');
		});

		it('returns null when there is no pending error', async () => {
			const desktopIntegration = await freshDesktopIntegration();
			invokeMock.mockResolvedValue(null);

			const result = await desktopIntegration.checkShortcutBindingError();

			expect(result).toBeNull();
		});
	});

	describe('checkShortcutTriggerDescription', () => {
		it('invokes check_shortcut_trigger_description and returns the result', async () => {
			const desktopIntegration = await freshDesktopIntegration();
			invokeMock.mockResolvedValue('Super+E');

			const result = await desktopIntegration.checkShortcutTriggerDescription();

			expect(invokeMock).toHaveBeenCalledWith(
				'plugin:desktop-integration|check_shortcut_trigger_description',
				undefined
			);
			expect(result).toBe('Super+E');
		});

		it('returns null when the shortcut has never been externally rebound', async () => {
			const desktopIntegration = await freshDesktopIntegration();
			invokeMock.mockResolvedValue(null);

			const result = await desktopIntegration.checkShortcutTriggerDescription();

			expect(result).toBeNull();
		});
	});
});
