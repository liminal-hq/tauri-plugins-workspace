// Tests the guest-side bindings for the xdg-portal plugin
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import { beforeEach, describe, expect, it, vi } from 'vitest';

const { invokeMock, listenMock } = vi.hoisted(() => ({
	invokeMock: vi.fn(),
	listenMock: vi.fn(),
}));

vi.mock('@tauri-apps/api/core', () => ({
	invoke: invokeMock,
}));

vi.mock('@tauri-apps/api/event', () => ({
	listen: listenMock,
}));

import {
	NOTIFICATION_ACTION_EVENT,
	featureStatus,
	isFeatureAvailable,
	isServiceError,
	portal,
} from './index';
import type { PortalStatus } from './index';

describe('portal', () => {
	beforeEach(() => {
		invokeMock.mockReset();
		listenMock.mockReset();
		invokeMock.mockResolvedValue(undefined);
	});

	it('keeps the existing commands', async () => {
		await portal.checkAvailability();
		await portal.getThemeInfo();
		expect(invokeMock).toHaveBeenNthCalledWith(
			1,
			'plugin:xdg-portal|check_availability',
			undefined
		);
		expect(invokeMock).toHaveBeenNthCalledWith(2, 'plugin:xdg-portal|get_theme_info', undefined);
	});

	it('asks for the status', async () => {
		await portal.getStatus();
		expect(invokeMock).toHaveBeenCalledWith('plugin:xdg-portal|get_status', undefined);
	});

	it('sends and withdraws notifications', async () => {
		const request = {
			id: 'job-1',
			title: 'Done',
			body: null,
			defaultAction: 'show',
			urgency: null,
		};
		await portal.sendNotification(request);
		await portal.withdrawNotification('job-1');
		expect(invokeMock).toHaveBeenNthCalledWith(1, 'plugin:xdg-portal|send_notification', {
			request,
		});
		expect(invokeMock).toHaveBeenNthCalledWith(2, 'plugin:xdg-portal|withdraw_notification', {
			id: 'job-1',
		});
	});

	it('takes and releases an inhibitor by handle', async () => {
		invokeMock.mockResolvedValueOnce({ handle: 7 });
		const handle = await portal.inhibit({ reason: 'Copying', kinds: ['idle', 'suspend'] });
		expect(handle).toEqual({ handle: 7 });
		expect(invokeMock).toHaveBeenCalledWith('plugin:xdg-portal|inhibit', {
			request: { reason: 'Copying', kinds: ['idle', 'suspend'] },
		});

		await portal.releaseInhibit(handle.handle);
		expect(invokeMock).toHaveBeenLastCalledWith('plugin:xdg-portal|release_inhibit', {
			handle: 7,
		});
	});

	it('opens a URI with the ask and writable options', async () => {
		const request = { uri: 'file:///tmp/a.txt', ask: true, writable: false };
		await portal.openUri(request);
		expect(invokeMock).toHaveBeenCalledWith('plugin:xdg-portal|open_uri', { request });
	});

	it('delivers notification actions and unsubscribes', async () => {
		const unlisten = vi.fn();
		listenMock.mockResolvedValue(unlisten);
		const callback = vi.fn();

		const stop = await portal.onNotificationAction(callback);

		expect(listenMock).toHaveBeenCalledWith(NOTIFICATION_ACTION_EVENT, expect.any(Function));
		const handler = listenMock.mock.calls[0][1] as (_event: { payload: unknown }) => void;
		handler({ payload: { id: 'job-1', action: 'show' } });
		expect(callback).toHaveBeenCalledWith({ id: 'job-1', action: 'show' });
		expect(stop).toBe(unlisten);
	});
});

describe('status helpers', () => {
	const status: PortalStatus = {
		available: true,
		reason: null,
		sandboxed: false,
		features: [
			{ feature: 'notification', available: true, reason: null, detail: null, version: 1 },
			{
				feature: 'inhibit',
				available: false,
				reason: 'interface-missing',
				detail: null,
				version: null,
			},
		],
	};

	it('finds a feature and reports its availability', () => {
		expect(featureStatus(status, 'inhibit')?.reason).toBe('interface-missing');
		expect(isFeatureAvailable(status, 'notification')).toBe(true);
		expect(isFeatureAvailable(status, 'inhibit')).toBe(false);
		expect(isFeatureAvailable(status, 'openUri')).toBe(false);
	});

	it('recognises a typed service error', () => {
		expect(isServiceError({ kind: 'timeout', message: 'slow' })).toBe(true);
		expect(isServiceError('plain string')).toBe(false);
		expect(isServiceError(null)).toBe(false);
	});
});
