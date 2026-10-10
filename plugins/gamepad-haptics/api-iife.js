if ('__TAURI__' in window) {
var __TAURI_PLUGIN_GAMEPAD_HAPTICS__ = (function (exports, core, event) {
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
    /** Buzzes one pad in a pattern that tells it from the others, so a player can confirm which it is. */
    function identify(padId) {
        return core.invoke(`${PREFIX}identify`, { padId });
    }
    const PAD_CONNECTED_EVENT = 'gamepad-haptics://connected';
    const PAD_CHANGED_EVENT = 'gamepad-haptics://changed';
    const PAD_DISCONNECTED_EVENT = 'gamepad-haptics://disconnected';
    function onPadConnected(handler) {
        return event.listen(PAD_CONNECTED_EVENT, (e) => handler(e.payload));
    }
    function onPadChanged(handler) {
        return event.listen(PAD_CHANGED_EVENT, (e) => handler(e.payload));
    }
    function onPadDisconnected(handler) {
        return event.listen(PAD_DISCONNECTED_EVENT, (e) => handler(e.payload));
    }

    exports.PAD_CHANGED_EVENT = PAD_CHANGED_EVENT;
    exports.PAD_CONNECTED_EVENT = PAD_CONNECTED_EVENT;
    exports.PAD_DISCONNECTED_EVENT = PAD_DISCONNECTED_EVENT;
    exports.capabilities = capabilities;
    exports.identify = identify;
    exports.listPads = listPads;
    exports.onPadChanged = onPadChanged;
    exports.onPadConnected = onPadConnected;
    exports.onPadDisconnected = onPadDisconnected;
    exports.playFrames = playFrames;
    exports.stop = stop;

    return exports;

})({}, __TAURI__.core, __TAURI__.event);
Object.defineProperty(window.__TAURI__, 'gamepadHaptics', { value: __TAURI_PLUGIN_GAMEPAD_HAPTICS__ }) }
