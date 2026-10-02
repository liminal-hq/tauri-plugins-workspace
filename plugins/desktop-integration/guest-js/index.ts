// Exposes guest-side bindings for the desktop-integration plugin
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import { invoke } from '@tauri-apps/api/core';
import { listen } from '@tauri-apps/api/event';
import type { Feature } from './bindings/Feature';
import type { FeatureStatus } from './bindings/FeatureStatus';
import type { FileManagerCall } from './bindings/FileManagerCall';
import type { FileManagerMethod } from './bindings/FileManagerMethod';
import type { FileManagerOwnership } from './bindings/FileManagerOwnership';
import type { FileManagerTarget } from './bindings/FileManagerTarget';
import type { GlobalShortcutPressed } from './bindings/GlobalShortcutPressed';
import type { GlobalShortcutRequest } from './bindings/GlobalShortcutRequest';
import type { LauncherProgress } from './bindings/LauncherProgress';
import type { LauncherRequest } from './bindings/LauncherRequest';
import type { NotificationAction } from './bindings/NotificationAction';
import type { NotifyRequest } from './bindings/NotifyRequest';
import type { PluginStatus } from './bindings/PluginStatus';
import type { ServiceError } from './bindings/ServiceError';
import type { ServiceErrorKind } from './bindings/ServiceErrorKind';
import type { ShortcutActivatedPayload } from './bindings/ShortcutActivatedPayload';
import type { ShortcutBindingResult } from './bindings/ShortcutBindingResult';
import type { ShortcutChangedPayload } from './bindings/ShortcutChangedPayload';
import type { SleepInhibitHandle } from './bindings/SleepInhibitHandle';
import type { SleepInhibitRequest } from './bindings/SleepInhibitRequest';
import type { SleepKind } from './bindings/SleepKind';
import type { UnavailableReason } from './bindings/UnavailableReason';
import type { Urgency } from './bindings/Urgency';

export type {
	Feature,
	FeatureStatus,
	FileManagerCall,
	FileManagerMethod,
	FileManagerOwnership,
	FileManagerTarget,
	GlobalShortcutPressed,
	GlobalShortcutRequest,
	LauncherProgress,
	LauncherRequest,
	NotificationAction,
	NotifyRequest,
	PluginStatus,
	ServiceError,
	ServiceErrorKind,
	ShortcutActivatedPayload,
	ShortcutBindingResult,
	ShortcutChangedPayload,
	SleepInhibitHandle,
	SleepInhibitRequest,
	SleepKind,
	UnavailableReason,
	Urgency,
};

/** Event emitted when the user clicks a notification shown through `notify`. */
export const NOTIFICATION_ACTION_EVENT = 'desktop-integration://notification-action';
/** Event emitted for each `org.freedesktop.FileManager1` call another application makes. */
export const FILE_MANAGER_CALL_EVENT = 'desktop-integration://file-manager';
/** Event emitted when the app gains or loses the `org.freedesktop.FileManager1` name. */
export const FILE_MANAGER_OWNERSHIP_EVENT = 'desktop-integration://file-manager-ownership';
/** Event emitted when a Windows global shortcut is pressed. */
export const SHORTCUT_PRESSED_EVENT = 'desktop-integration://shortcut-pressed';

/** Whether a rejection from one of the service commands is a {@link ServiceError}. */
export function isServiceError(error: unknown): error is ServiceError {
	return (
		typeof error === 'object' &&
		error !== null &&
		typeof (error as ServiceError).kind === 'string' &&
		typeof (error as ServiceError).message === 'string'
	);
}

/** The status of one feature, or `undefined` when the plugin did not report it. */
export function featureStatus(status: PluginStatus, feature: Feature): FeatureStatus | undefined {
	return status.features.find((entry) => entry.feature === feature);
}

/** Whether a feature is available; the way to decide whether to show the option that needs it. */
export function isFeatureAvailable(status: PluginStatus, feature: Feature): boolean {
	return featureStatus(status, feature)?.available === true;
}

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

function on<T>(event: string, callback: (_payload: T) => void): Promise<() => void> {
	return listen<T>(event, (e) => callback(e.payload));
}

export const desktopIntegration = {
	/** Which services work on this system, with a reason for each that does not. */
	getStatus: () => cmd<PluginStatus>('get_status'),

	/**
	 * Shows a desktop notification outside the portal, or replaces the one with the same `id`.
	 * Windows needs an AppUserModelID (see {@link desktopIntegration.setAppUserModelId}); without
	 * one this rejects with kind `needs-app-id`.
	 */
	notify: (request: NotifyRequest) => cmd<void>('notify', { request }),

	/** Takes a notification shown through `notify` off screen. */
	withdrawNotification: (id: string) => cmd<void>('withdraw_notification', { id }),

	/** Sets the AppUserModelID of the process, which Windows toasts need. Windows only. */
	setAppUserModelId: (id: string) => cmd<void>('set_app_user_model_id', { id }),

	/**
	 * Keeps the machine from sleeping (or idling, on Linux) until
	 * {@link desktopIntegration.releaseSleepInhibit} is called with the returned handle or the app
	 * exits.
	 */
	inhibitSleep: (request: SleepInhibitRequest) =>
		cmd<SleepInhibitHandle>('inhibit_sleep', { request }),

	/** Ends an inhibitor taken with {@link desktopIntegration.inhibitSleep}. */
	releaseSleepInhibit: (handle: number) => cmd<void>('release_sleep_inhibit', { handle }),

	/**
	 * Shows one combined progress value (0 to 1), an indeterminate state or nothing on the app's
	 * dock or taskbar entry, with an optional badge count (Linux).
	 */
	setLauncherProgress: (request: LauncherRequest) =>
		cmd<void>('set_launcher_progress', { request }),

	/**
	 * Takes the `org.freedesktop.FileManager1` name so other applications' "Show in folder" reaches
	 * this app as {@link FILE_MANAGER_CALL_EVENT} events. Linux only.
	 */
	ownFileManager: () => cmd<FileManagerOwnership>('own_file_manager'),

	/** Gives the `org.freedesktop.FileManager1` name back. */
	disownFileManager: () => cmd<FileManagerOwnership>('disown_file_manager'),

	/**
	 * Registers a global shortcut with `RegisterHotKey`; {@link desktopIntegration.onShortcutPressed}
	 * reports presses. Windows only; Linux uses {@link desktopIntegration.registerShortcut}.
	 */
	registerGlobalShortcut: (request: GlobalShortcutRequest) =>
		cmd<void>('register_global_shortcut', { request }),

	/** Removes a shortcut registered with {@link desktopIntegration.registerGlobalShortcut}. */
	unregisterGlobalShortcut: (id: string) => cmd<void>('unregister_global_shortcut', { id }),

	/** Subscribes to notification clicks; resolves to a function that unsubscribes. */
	onNotificationAction: (callback: (_action: NotificationAction) => void) =>
		on<NotificationAction>(NOTIFICATION_ACTION_EVENT, callback),

	/** Subscribes to `FileManager1` calls from other applications. */
	onFileManagerCall: (callback: (_call: FileManagerCall) => void) =>
		on<FileManagerCall>(FILE_MANAGER_CALL_EVENT, callback),

	/** Subscribes to the app losing the `FileManager1` name to another file manager. */
	onFileManagerOwnership: (callback: (_ownership: FileManagerOwnership) => void) =>
		on<FileManagerOwnership>(FILE_MANAGER_OWNERSHIP_EVENT, callback),

	/** Subscribes to presses of Windows global shortcuts. */
	onShortcutPressed: (callback: (_pressed: GlobalShortcutPressed) => void) =>
		on<GlobalShortcutPressed>(SHORTCUT_PRESSED_EVENT, callback),

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

	/**
	 * Returns the trigger description (e.g. "Super+E") from the most recent
	 * shortcut-changed event, or null if the shortcut hasn't been externally
	 * rebound yet this session. Use this to hydrate UI that mounts after a missed
	 * event — listen for the `shortcut-changed` event directly via
	 * `@tauri-apps/api/event`'s `listen()` for live updates.
	 */
	checkShortcutTriggerDescription: () => cmd<string | null>('check_shortcut_trigger_description'),
};
