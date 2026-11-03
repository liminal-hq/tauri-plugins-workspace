if ('__TAURI__' in window) {
var __TAURI_PLUGIN_STEAMWORKS__ = (function (exports, core, event) {
    'use strict';

    // Typed guest-side wrappers for the steamworks plugin
    //
    // (c) Copyright 2026 Liminal HQ, Scott Morris
    // SPDX-License-Identifier: Apache-2.0 OR MIT
    const PREFIX = 'plugin:steamworks|';
    const STATS_RECEIVED_EVENT = 'steamworks://stats-received';
    const STATS_STORED_EVENT = 'steamworks://stats-stored';
    const ACHIEVEMENT_STORED_EVENT = 'steamworks://achievement-stored';
    /** True when Steam initialised, or the in-memory mock is active. Never rejects. */
    function isAvailable() {
        return core.invoke(`${PREFIX}is_available`);
    }
    /** Says whether Steam is usable and, if not, why. Never rejects. */
    function getStatus() {
        return core.invoke(`${PREFIX}get_status`);
    }
    function getPlayer() {
        return core.invoke(`${PREFIX}get_player`);
    }
    function getAchievement(id) {
        return core.invoke(`${PREFIX}get_achievement`, { id });
    }
    /** Unlocks an achievement and stores it so Steam shows its notification. */
    function unlockAchievement(id) {
        return core.invoke(`${PREFIX}unlock_achievement`, { id });
    }
    /** Re-locks an achievement. Meant for development builds; it needs its own permission. */
    function clearAchievement(id) {
        return core.invoke(`${PREFIX}clear_achievement`, { id });
    }
    /** Shows Steam's progress toast for a counted achievement. `current` must not exceed `max`. */
    function indicateProgress(id, current, max) {
        return core.invoke(`${PREFIX}indicate_achievement_progress`, { id, current, max });
    }
    async function getStat(name, kind = 'int') {
        const stat = await core.invoke(`${PREFIX}get_stat`, { name, kind });
        return stat.value;
    }
    /**
     * Sets a stat without storing it, so several writes can be stored together with `storeStats`.
     * Without `kind`, a whole number is sent as an int and anything else as a float. Pass `kind` for a
     * float stat that currently holds a whole number.
     */
    function setStat(name, value, kind) {
        const resolved = kind ?? (Number.isInteger(value) ? 'int' : 'float');
        return core.invoke(`${PREFIX}set_stat`, { name, value: { kind: resolved, value } });
    }
    function storeStats() {
        return core.invoke(`${PREFIX}store_stats`);
    }
    /** Needs `core:event:default` as well as the plugin's permissions. */
    function onStatsReceived(handler) {
        return event.listen(STATS_RECEIVED_EVENT, (e) => handler(e.payload));
    }
    function onStatsStored(handler) {
        return event.listen(STATS_STORED_EVENT, (e) => handler(e.payload));
    }
    function onAchievementStored(handler) {
        return event.listen(ACHIEVEMENT_STORED_EVENT, (e) => handler(e.payload));
    }
    /** True when a rejection means Steam is not usable, as opposed to a bad id or a failed call. */
    function isUnavailableError(error) {
        const message = error instanceof Error ? error.message : String(error);
        return message.startsWith('unavailable:');
    }

    exports.ACHIEVEMENT_STORED_EVENT = ACHIEVEMENT_STORED_EVENT;
    exports.STATS_RECEIVED_EVENT = STATS_RECEIVED_EVENT;
    exports.STATS_STORED_EVENT = STATS_STORED_EVENT;
    exports.clearAchievement = clearAchievement;
    exports.getAchievement = getAchievement;
    exports.getPlayer = getPlayer;
    exports.getStat = getStat;
    exports.getStatus = getStatus;
    exports.indicateProgress = indicateProgress;
    exports.isAvailable = isAvailable;
    exports.isUnavailableError = isUnavailableError;
    exports.onAchievementStored = onAchievementStored;
    exports.onStatsReceived = onStatsReceived;
    exports.onStatsStored = onStatsStored;
    exports.setStat = setStat;
    exports.storeStats = storeStats;
    exports.unlockAchievement = unlockAchievement;

    return exports;

})({}, __TAURI__.core, __TAURI__.event);
Object.defineProperty(window.__TAURI__, 'steamworks', { value: __TAURI_PLUGIN_STEAMWORKS__ }) }
