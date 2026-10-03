// Exposes typed guest-side wrappers for plugin command invocation
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import { invoke } from '@tauri-apps/api/core';
import { listen } from '@tauri-apps/api/event';
import type { ActionButton } from './bindings/ActionButton';
import type { AccentColour } from './bindings/AccentColour';
import type { AvailabilityInfo } from './bindings/AvailabilityInfo';
import type { ColourScheme } from './bindings/ColourScheme';
import type { DesktopEnvironment } from './bindings/DesktopEnvironment';
import type { FeatureStatus } from './bindings/FeatureStatus';
import type { InhibitHandle } from './bindings/InhibitHandle';
import type { InhibitKind } from './bindings/InhibitKind';
import type { InhibitRequest } from './bindings/InhibitRequest';
import type { NotificationAction } from './bindings/NotificationAction';
import type { NotificationRequest } from './bindings/NotificationRequest';
import type { OpenUriRequest } from './bindings/OpenUriRequest';
import type { PortalFeature } from './bindings/PortalFeature';
import type { PortalStatus } from './bindings/PortalStatus';
import type { ServiceError } from './bindings/ServiceError';
import type { ServiceErrorKind } from './bindings/ServiceErrorKind';
import type { ThemeInfo } from './bindings/ThemeInfo';
import type { UnavailableReason } from './bindings/UnavailableReason';
import type { Urgency } from './bindings/Urgency';

const PREFIX = 'plugin:xdg-portal|';

/**
 * Event emitted to every window when the user clicks a notification or presses one of its action
 * buttons; the payload is `{ id, action }`.
 */
export const NOTIFICATION_ACTION_EVENT = 'xdg-portal://notification-action';

function cmd<T>(name: string, args?: Record<string, unknown>): Promise<T> {
	return invoke<T>(`${PREFIX}${name}`, args);
}

export const portal = {
	checkAvailability: () => cmd<AvailabilityInfo>('check_availability'),
	getThemeInfo: () => cmd<ThemeInfo>('get_theme_info'),

	/** Which of the notification, notification-actions, inhibit and open-URI portals work here, with a reason for each that does not. */
	getStatus: () => cmd<PortalStatus>('get_status'),

	/**
	 * Shows a notification, or replaces the one with the same `id`. Rejects with a
	 * {@link ServiceError}.
	 */
	sendNotification: (request: NotificationRequest) => cmd<void>('send_notification', { request }),

	/** Takes a notification off screen. */
	withdrawNotification: (id: string) => cmd<void>('withdraw_notification', { id }),

	/**
	 * Keeps the session from idling and suspending (or only the kinds given) until
	 * {@link portal.releaseInhibit} is called with the returned handle or the app exits.
	 */
	inhibit: (request: InhibitRequest) => cmd<InhibitHandle>('inhibit', { request }),

	/**
	 * Ends an inhibitor taken with {@link portal.inhibit}. If the portal fails to close it, the
	 * call rejects and the handle stays valid, so it can be released again.
	 */
	releaseInhibit: (handle: number) => cmd<void>('release_inhibit', { handle }),

	/**
	 * Opens a URI, a local file (`file:` URI) or a folder with the user's chosen application.
	 * Resolves once the portal accepts the request, not when the user has picked an application.
	 */
	openUri: (request: OpenUriRequest) => cmd<void>('open_uri', { request }),

	/** Subscribes to notification clicks and button presses; resolves to a function that unsubscribes. */
	onNotificationAction: (callback: (_action: NotificationAction) => void): Promise<() => void> =>
		listen<NotificationAction>(NOTIFICATION_ACTION_EVENT, (event) => callback(event.payload)),
};

/** Whether a rejection from a notification, inhibit or open-URI command is a {@link ServiceError}. */
export function isServiceError(error: unknown): error is ServiceError {
	return (
		typeof error === 'object' &&
		error !== null &&
		typeof (error as ServiceError).kind === 'string' &&
		typeof (error as ServiceError).message === 'string'
	);
}

/** The status of one feature, or `undefined` when the plugin did not report it. */
export function featureStatus(
	status: PortalStatus,
	feature: PortalFeature
): FeatureStatus | undefined {
	return status.features.find((entry) => entry.feature === feature);
}

/** Whether a feature is available; the way to decide whether to show the option that needs it. */
export function isFeatureAvailable(status: PortalStatus, feature: PortalFeature): boolean {
	return featureStatus(status, feature)?.available === true;
}

export type {
	ActionButton,
	ThemeInfo,
	ColourScheme,
	DesktopEnvironment,
	AccentColour,
	AvailabilityInfo,
	FeatureStatus,
	InhibitHandle,
	InhibitKind,
	InhibitRequest,
	NotificationAction,
	NotificationRequest,
	OpenUriRequest,
	PortalFeature,
	PortalStatus,
	ServiceError,
	ServiceErrorKind,
	UnavailableReason,
	Urgency,
};
