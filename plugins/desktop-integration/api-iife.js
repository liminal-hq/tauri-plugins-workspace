if ('__TAURI__' in window) {
var __TAURI_PLUGIN_DESKTOP_INTEGRATION__ = (function (exports, core, event) {
    'use strict';

    // Exposes guest-side bindings for the desktop-integration plugin
    //
    // (c) Copyright 2026 Liminal HQ, Scott Morris
    // SPDX-License-Identifier: Apache-2.0 OR MIT
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
    const desktopIntegration = {
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
    };

    exports.desktopIntegration = desktopIntegration;

    return exports;

})({}, __TAURI__.core, __TAURI__.event);
Object.defineProperty(window.__TAURI__, 'desktopIntegration', { value: __TAURI_PLUGIN_DESKTOP_INTEGRATION__ }) }
