if ('__TAURI__' in window) {
var __TAURI_PLUGIN_XDG_PORTAL__ = (function (exports, core, event) {
    'use strict';

    // Exposes typed guest-side wrappers for plugin command invocation
    //
    // (c) Copyright 2026 Liminal HQ, Scott Morris
    // SPDX-License-Identifier: Apache-2.0 OR MIT
    const PREFIX = 'plugin:xdg-portal|';
    /** Event emitted to every window when the user clicks a notification's default action. */
    const NOTIFICATION_ACTION_EVENT = 'xdg-portal://notification-action';
    function cmd(name, args) {
        return core.invoke(`${PREFIX}${name}`, args);
    }
    const portal = {
        checkAvailability: () => cmd('check_availability'),
        getThemeInfo: () => cmd('get_theme_info'),
        /** Which of the notification, inhibit and open-URI portals work here, with a reason for each that does not. */
        getStatus: () => cmd('get_status'),
        /**
         * Shows a notification, or replaces the one with the same `id`. Rejects with a
         * {@link ServiceError}.
         */
        sendNotification: (request) => cmd('send_notification', { request }),
        /** Takes a notification off screen. */
        withdrawNotification: (id) => cmd('withdraw_notification', { id }),
        /**
         * Keeps the session from idling and suspending (or only the kinds given) until
         * {@link portal.releaseInhibit} is called with the returned handle or the app exits.
         */
        inhibit: (request) => cmd('inhibit', { request }),
        /** Ends an inhibitor taken with {@link portal.inhibit}. */
        releaseInhibit: (handle) => cmd('release_inhibit', { handle }),
        /**
         * Opens a URI, a local file (`file:` URI) or a folder with the user's chosen application.
         * Resolves once the portal accepts the request, not when the user has picked an application.
         */
        openUri: (request) => cmd('open_uri', { request }),
        /** Subscribes to notification clicks; resolves to a function that unsubscribes. */
        onNotificationAction: (callback) => event.listen(NOTIFICATION_ACTION_EVENT, (event) => callback(event.payload)),
    };
    /** Whether a rejection from a notification, inhibit or open-URI command is a {@link ServiceError}. */
    function isServiceError(error) {
        return (typeof error === 'object' &&
            error !== null &&
            typeof error.kind === 'string' &&
            typeof error.message === 'string');
    }
    /** The status of one feature, or `undefined` when the plugin did not report it. */
    function featureStatus(status, feature) {
        return status.features.find((entry) => entry.feature === feature);
    }
    /** Whether a feature is available; the way to decide whether to show the option that needs it. */
    function isFeatureAvailable(status, feature) {
        return featureStatus(status, feature)?.available === true;
    }

    exports.NOTIFICATION_ACTION_EVENT = NOTIFICATION_ACTION_EVENT;
    exports.featureStatus = featureStatus;
    exports.isFeatureAvailable = isFeatureAvailable;
    exports.isServiceError = isServiceError;
    exports.portal = portal;

    return exports;

})({}, __TAURI__.core, __TAURI__.event);
Object.defineProperty(window.__TAURI__, 'xdgPortal', { value: __TAURI_PLUGIN_XDG_PORTAL__ }) }
