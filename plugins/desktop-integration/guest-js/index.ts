// Exposes guest-side bindings for the desktop-integration plugin
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import { invoke } from '@tauri-apps/api/core';
import { listen } from '@tauri-apps/api/event';
import type { ShortcutActivatedPayload } from './bindings/ShortcutActivatedPayload';
import type { ShortcutBindingResult } from './bindings/ShortcutBindingResult';

export type { ShortcutActivatedPayload, ShortcutBindingResult };

const PREFIX = 'plugin:desktop-integration|';

function cmd<T>(name: string, args?: Record<string, unknown>): Promise<T> {
	return invoke<T>(`${PREFIX}${name}`, args);
}

let activationCallback: (() => void) | null = null;
let activationListening: Promise<unknown> | null = null;

function ensureActivationListener(): void {
	if (activationListening) return;
	activationListening = listen<ShortcutActivatedPayload>('shortcut-activated', () => {
		activationCallback?.();
	});
}

export const desktopIntegration = {
	/**
	 * Registers a global shortcut. On X11 it's bound immediately; on Wayland,
	 * binding is deferred until the compositor confirms it — see
	 * checkShortcutBindingComplete/checkShortcutBindingError.
	 *
	 * `sessionId` and `sessionDescription` identify the Wayland portal session:
	 * `sessionId` should be a stable, app-specific string, and `sessionDescription`
	 * is shown to the user in the compositor's shortcut binding dialog.
	 *
	 * `onActivated` fires each time the shortcut is pressed. Registering a new
	 * shortcut replaces both the binding and the callback.
	 */
	registerShortcut: (
		sessionId: string,
		sessionDescription: string,
		shortcut: string,
		onActivated: () => void
	): Promise<void> => {
		activationCallback = onActivated;
		ensureActivationListener();
		return cmd('register_shortcut', { sessionId, sessionDescription, shortcut });
	},

	/**
	 * Returns true once the portal BindShortcuts call has completed successfully.
	 * On X11 this is always true immediately after startup.
	 * Use this as a race guard after registering the shortcut-binding-result listener.
	 */
	checkShortcutBindingComplete: () => cmd<boolean>('check_shortcut_binding_complete'),

	/**
	 * Returns the error message if BindShortcuts failed, or null if still pending
	 * or successful. Use this as a race guard after registering the
	 * shortcut-binding-result listener — complements checkShortcutBindingComplete.
	 */
	checkShortcutBindingError: () => cmd<string | null>('check_shortcut_binding_error'),
};
