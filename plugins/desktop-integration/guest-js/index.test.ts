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

describe('desktopIntegration services', () => {
	beforeEach(() => {
		invokeMock.mockReset();
		listenMock.mockReset();
		invokeMock.mockResolvedValue(undefined);
		listenMock.mockResolvedValue(() => {});
	});

	it('asks for the status', async () => {
		const desktopIntegration = await freshDesktopIntegration();
		await desktopIntegration.getStatus();
		expect(invokeMock).toHaveBeenCalledWith('plugin:desktop-integration|get_status', undefined);
	});

	it('shows and withdraws notifications', async () => {
		const desktopIntegration = await freshDesktopIntegration();
		const request = {
			id: 'job-1',
			title: 'Done',
			body: null,
			defaultAction: 'show',
			urgency: null,
			appName: null,
			desktopId: null,
		};
		await desktopIntegration.notify(request);
		await desktopIntegration.withdrawNotification('job-1');
		await desktopIntegration.setAppUserModelId('ca.liminalhq.waypoint');
		expect(invokeMock).toHaveBeenNthCalledWith(1, 'plugin:desktop-integration|notify', { request });
		expect(invokeMock).toHaveBeenNthCalledWith(
			2,
			'plugin:desktop-integration|withdraw_notification',
			{ id: 'job-1' }
		);
		expect(invokeMock).toHaveBeenNthCalledWith(
			3,
			'plugin:desktop-integration|set_app_user_model_id',
			{ id: 'ca.liminalhq.waypoint' }
		);
	});

	it('takes and releases a sleep inhibitor by handle', async () => {
		const desktopIntegration = await freshDesktopIntegration();
		invokeMock.mockResolvedValueOnce({ handle: 3 });
		const handle = await desktopIntegration.inhibitSleep({ reason: 'Copying', kinds: ['sleep'] });
		expect(handle).toEqual({ handle: 3 });
		expect(invokeMock).toHaveBeenCalledWith('plugin:desktop-integration|inhibit_sleep', {
			request: { reason: 'Copying', kinds: ['sleep'] },
		});
		await desktopIntegration.releaseSleepInhibit(handle.handle);
		expect(invokeMock).toHaveBeenLastCalledWith(
			'plugin:desktop-integration|release_sleep_inhibit',
			{ handle: 3 }
		);
	});

	it('sets launcher progress as a value, indeterminate or cleared', async () => {
		const desktopIntegration = await freshDesktopIntegration();
		for (const progress of [
			{ state: 'value', value: 0.5 },
			{ state: 'indeterminate' },
			{ state: 'cleared' },
		] as const) {
			const request = { progress, count: 2, desktopId: null, windowLabel: null };
			await desktopIntegration.setLauncherProgress(request);
			expect(invokeMock).toHaveBeenLastCalledWith(
				'plugin:desktop-integration|set_launcher_progress',
				{ request }
			);
		}
	});

	it('owns and disowns the file manager name', async () => {
		const desktopIntegration = await freshDesktopIntegration();
		invokeMock.mockResolvedValueOnce({ owned: true, reason: null });
		expect(await desktopIntegration.ownFileManager()).toEqual({ owned: true, reason: null });
		await desktopIntegration.disownFileManager();
		expect(invokeMock).toHaveBeenNthCalledWith(
			1,
			'plugin:desktop-integration|own_file_manager',
			undefined
		);
		expect(invokeMock).toHaveBeenNthCalledWith(
			2,
			'plugin:desktop-integration|disown_file_manager',
			undefined
		);
	});

	it('registers and unregisters a Windows global shortcut', async () => {
		const desktopIntegration = await freshDesktopIntegration();
		const request = { id: 'toggle', accelerator: 'Ctrl+Alt+K' };
		await desktopIntegration.registerGlobalShortcut(request);
		await desktopIntegration.unregisterGlobalShortcut('toggle');
		expect(invokeMock).toHaveBeenNthCalledWith(
			1,
			'plugin:desktop-integration|register_global_shortcut',
			{ request }
		);
		expect(invokeMock).toHaveBeenNthCalledWith(
			2,
			'plugin:desktop-integration|unregister_global_shortcut',
			{ id: 'toggle' }
		);
	});

	it('delivers each event payload and unsubscribes', async () => {
		const mod = await import('./index');
		const unlisten = vi.fn();
		listenMock.mockResolvedValue(unlisten);
		const cases = [
			[mod.desktopIntegration.onNotificationAction, mod.NOTIFICATION_ACTION_EVENT],
			[mod.desktopIntegration.onFileManagerCall, mod.FILE_MANAGER_CALL_EVENT],
			[mod.desktopIntegration.onFileManagerOwnership, mod.FILE_MANAGER_OWNERSHIP_EVENT],
			[mod.desktopIntegration.onShortcutPressed, mod.SHORTCUT_PRESSED_EVENT],
		] as const;
		for (const [subscribe, event] of cases) {
			listenMock.mockClear();
			const callback = vi.fn();
			const stop = await subscribe(callback as never);
			expect(listenMock).toHaveBeenCalledWith(event, expect.any(Function));
			listenMock.mock.calls[0][1]({ payload: { sample: event } });
			expect(callback).toHaveBeenCalledWith({ sample: event });
			expect(stop).toBe(unlisten);
		}
	});

	it('names the events under the plugin scheme', async () => {
		const mod = await import('./index');
		expect(mod.FILE_MANAGER_CALL_EVENT).toBe('desktop-integration://file-manager');
		expect(mod.SHORTCUT_PRESSED_EVENT).toBe('desktop-integration://shortcut-pressed');
	});

	it('finds feature status and recognises service errors', async () => {
		const mod = await import('./index');
		const status = {
			available: true,
			reason: null,
			fileManagerOwned: false,
			features: [
				{ feature: 'notify', available: false, reason: 'needs-app-id', detail: null },
				{ feature: 'inhibitSleep', available: true, reason: null, detail: null },
			],
		} as const;
		expect(mod.isFeatureAvailable(status as never, 'inhibitSleep')).toBe(true);
		expect(mod.isFeatureAvailable(status as never, 'notify')).toBe(false);
		expect(mod.featureStatus(status as never, 'notify')?.reason).toBe('needs-app-id');
		expect(mod.isFeatureAvailable(status as never, 'fileManager')).toBe(false);
		expect(mod.isServiceError({ kind: 'needs-app-id', message: 'x' })).toBe(true);
		expect(mod.isServiceError('nope')).toBe(false);
	});
});
