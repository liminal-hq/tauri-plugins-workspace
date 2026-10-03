if ('__TAURI__' in window) {
var __TAURI_PLUGIN_DESKTOP_INTEGRATION__ = (function (exports, core, event) {
    'use strict';

    // Exposes guest-side bindings for the desktop-integration plugin
    //
    // (c) Copyright 2026 Liminal HQ, Scott Morris
    // SPDX-License-Identifier: Apache-2.0 OR MIT
    /**
     * Event emitted when the user clicks a notification shown through `notify` or presses one of its
     * action buttons; the payload is `{ id, action }`.
     */
    const NOTIFICATION_ACTION_EVENT = 'desktop-integration://notification-action';
    /** Event emitted for each `org.freedesktop.FileManager1` call another application makes. */
    const FILE_MANAGER_CALL_EVENT = 'desktop-integration://file-manager';
    /** Event emitted when the app gains or loses the `org.freedesktop.FileManager1` name. */
    const FILE_MANAGER_OWNERSHIP_EVENT = 'desktop-integration://file-manager-ownership';
    /** Event emitted when a Windows global shortcut is pressed. */
    const SHORTCUT_PRESSED_EVENT = 'desktop-integration://shortcut-pressed';
    /** Whether a rejection from one of the service commands is a {@link ServiceError}. */
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
    const PREFIX = 'plugin:desktop-integration|';
    function cmd(name, args) {
        return core.invoke(`${PREFIX}${name}`, args);
    }
    let activationCallback = null;
    let activationListening = null;
    function ensureActivationListener() {
        if (activationListening)
            return;
        activationListening = event.listen('shortcut-activated', () => {
            activationCallback?.();
        });
    }
    function on(event$1, callback) {
        return event.listen(event$1, (e) => callback(e.payload));
    }
    const desktopIntegration = {
        /** Which services work on this system, with a reason for each that does not. */
        getStatus: () => cmd('get_status'),
        /**
         * Shows a desktop notification outside the portal, or replaces the one with the same `id`.
         * Windows needs an AppUserModelID (see {@link desktopIntegration.setAppUserModelId}); without
         * one this rejects with kind `needs-app-id`.
         */
        notify: (request) => cmd('notify', { request }),
        /** Takes a notification shown through `notify` off screen. */
        withdrawNotification: (id) => cmd('withdraw_notification', { id }),
        /** Sets the AppUserModelID of the process, which Windows toasts need. Windows only. */
        setAppUserModelId: (id) => cmd('set_app_user_model_id', { id }),
        /**
         * Keeps the machine from sleeping (or idling, on Linux) until
         * {@link desktopIntegration.releaseSleepInhibit} is called with the returned handle or the app
         * exits.
         */
        inhibitSleep: (request) => cmd('inhibit_sleep', { request }),
        /** Ends an inhibitor taken with {@link desktopIntegration.inhibitSleep}. */
        releaseSleepInhibit: (handle) => cmd('release_sleep_inhibit', { handle }),
        /**
         * Shows one combined progress value (0 to 1), an indeterminate state or nothing on the app's
         * dock or taskbar entry, with an optional badge count (Linux).
         */
        setLauncherProgress: (request) => cmd('set_launcher_progress', { request }),
        /**
         * Takes the `org.freedesktop.FileManager1` name so other applications' "Show in folder" reaches
         * this app as {@link FILE_MANAGER_CALL_EVENT} events. Linux only.
         */
        ownFileManager: () => cmd('own_file_manager'),
        /** Gives the `org.freedesktop.FileManager1` name back. */
        disownFileManager: () => cmd('disown_file_manager'),
        /**
         * Registers a global shortcut with `RegisterHotKey`; {@link desktopIntegration.onShortcutPressed}
         * reports presses. Registering an id again replaces its accelerator, the same one included; if
         * the new accelerator is refused, the earlier binding stays. A call that rejects with
         * `timeout` did not take effect. Windows only; Linux uses
         * {@link desktopIntegration.registerShortcut}.
         */
        registerGlobalShortcut: (request) => cmd('register_global_shortcut', { request }),
        /** Removes a shortcut registered with {@link desktopIntegration.registerGlobalShortcut}. */
        unregisterGlobalShortcut: (id) => cmd('unregister_global_shortcut', { id }),
        /** Subscribes to notification clicks and button presses; resolves to a function that unsubscribes. */
        onNotificationAction: (callback) => on(NOTIFICATION_ACTION_EVENT, callback),
        /** Subscribes to `FileManager1` calls from other applications. */
        onFileManagerCall: (callback) => on(FILE_MANAGER_CALL_EVENT, callback),
        /** Subscribes to the app losing the `FileManager1` name to another file manager. */
        onFileManagerOwnership: (callback) => on(FILE_MANAGER_OWNERSHIP_EVENT, callback),
        /** Subscribes to presses of Windows global shortcuts. */
        onShortcutPressed: (callback) => on(SHORTCUT_PRESSED_EVENT, callback),
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
        registerShortcut: (sessionId, sessionDescription, shortcut, onActivated) => {
            activationCallback = onActivated;
            ensureActivationListener();
            return cmd('register_shortcut', { sessionId, sessionDescription, shortcut });
        },
        /**
         * Returns true once the portal BindShortcuts call has completed successfully.
         * On X11 this is always true immediately after startup.
         * Use this as a race guard after registering the shortcut-binding-result listener.
         */
        checkShortcutBindingComplete: () => cmd('check_shortcut_binding_complete'),
        /**
         * Returns the error message if BindShortcuts failed, or null if still pending
         * or successful. Use this as a race guard after registering the
         * shortcut-binding-result listener — complements checkShortcutBindingComplete.
         */
        checkShortcutBindingError: () => cmd('check_shortcut_binding_error'),
        /**
         * Returns the trigger description (e.g. "Super+E") from the most recent
         * shortcut-changed event, or null if the shortcut hasn't been externally
         * rebound yet this session. Use this to hydrate UI that mounts after a missed
         * event — listen for the `shortcut-changed` event directly via
         * `@tauri-apps/api/event`'s `listen()` for live updates.
         */
        checkShortcutTriggerDescription: () => cmd('check_shortcut_trigger_description'),
    };

    exports.FILE_MANAGER_CALL_EVENT = FILE_MANAGER_CALL_EVENT;
    exports.FILE_MANAGER_OWNERSHIP_EVENT = FILE_MANAGER_OWNERSHIP_EVENT;
    exports.NOTIFICATION_ACTION_EVENT = NOTIFICATION_ACTION_EVENT;
    exports.SHORTCUT_PRESSED_EVENT = SHORTCUT_PRESSED_EVENT;
    exports.desktopIntegration = desktopIntegration;
    exports.featureStatus = featureStatus;
    exports.isFeatureAvailable = isFeatureAvailable;
    exports.isServiceError = isServiceError;

    return exports;

})({}, __TAURI__.core, __TAURI__.event);
Object.defineProperty(window.__TAURI__, 'desktopIntegration', { value: __TAURI_PLUGIN_DESKTOP_INTEGRATION__ }) }
