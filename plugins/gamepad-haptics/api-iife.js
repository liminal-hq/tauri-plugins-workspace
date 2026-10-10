if ('__TAURI__' in window) {
var __TAURI_PLUGIN_GAMEPAD_HAPTICS__ = (function (exports, core) {
    'use strict';

    // Typed guest-side wrappers for the gamepad-haptics plugin
    //
    // (c) Copyright 2026 Liminal HQ, Scott Morris
    // SPDX-License-Identifier: Apache-2.0 OR MIT
    const PREFIX = 'plugin:gamepad-haptics|';
    function capabilities() {
        return core.invoke(`${PREFIX}capabilities`);
    }
    function listPads() {
        return core.invoke(`${PREFIX}list_pads`);
    }
    function playFrames(padId, frames) {
        return core.invoke(`${PREFIX}play_frames`, { args: { padId, frames } });
    }
    /** Stops one pad, or every pad when `padId` is omitted. */
    function stop(padId) {
        return core.invoke(`${PREFIX}stop`, { padId });
    }

    exports.capabilities = capabilities;
    exports.listPads = listPads;
    exports.playFrames = playFrames;
    exports.stop = stop;

    return exports;

})({}, __TAURI__.core);
Object.defineProperty(window.__TAURI__, 'gamepadHaptics', { value: __TAURI_PLUGIN_GAMEPAD_HAPTICS__ }) }
