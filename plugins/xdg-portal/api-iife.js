if ('__TAURI__' in window) {
var __TAURI_PLUGIN_XDG_PORTAL__ = (function (exports, core) {
    'use strict';

    // Exposes typed guest-side wrappers for plugin command invocation
    //
    // (c) Copyright 2026 Liminal HQ, Scott Morris
    // SPDX-License-Identifier: Apache-2.0 OR MIT
    const PREFIX = 'plugin:xdg-portal|';
    function cmd(name, args) {
        return core.invoke(`${PREFIX}${name}`, args);
    }
    const portal = {
        checkAvailability: () => cmd('check_availability'),
        getThemeInfo: () => cmd('get_theme_info'),
    };

    exports.portal = portal;

    return exports;

})({}, __TAURI__.core);
Object.defineProperty(window.__TAURI__, 'xdgPortal', { value: __TAURI_PLUGIN_XDG_PORTAL__ }) }
