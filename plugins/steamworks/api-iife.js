if ('__TAURI__' in window) {
var __TAURI_PLUGIN_STEAMWORKS__ = (function (exports, core, event) {
    'use strict';

    // Typed guest-side wrappers for the steamworks plugin
    //
    // (c) Copyright 2026 Liminal HQ, Scott Morris
    // SPDX-License-Identifier: Apache-2.0 OR MIT
    const PREFIX = 'plugin:steamworks|';
    const CLOUD_NAME_HEADER = 'x-steamworks-cloud-name';
    const STATS_RECEIVED_EVENT = 'steamworks://stats-received';
    const STATS_STORED_EVENT = 'steamworks://stats-stored';
    const ACHIEVEMENT_STORED_EVENT = 'steamworks://achievement-stored';
    const OVERLAY_TOGGLED_EVENT = 'steamworks://overlay-toggled';
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
    /**
     * Sets the rich presence keys friends see. A `null` value removes that key. Steam allows 20 keys of
     * up to 64 bytes, with values of up to 256 bytes. Needs the `allow-presence` permission.
     */
    function setRichPresence(entries) {
        return core.invoke(`${PREFIX}set_rich_presence`, { entries });
    }
    function clearRichPresence() {
        return core.invoke(`${PREFIX}clear_rich_presence`);
    }
    /**
     * Opens a page of the Steam overlay. Needs the `allow-overlay` permission, and Steam's overlay
     * must be able to hook the window; see the README for Windows.
     */
    function activateOverlay(page) {
        return core.invoke(`${PREFIX}activate_overlay`, { page });
    }
    /** Opens an `http` or `https` address in the overlay's browser. Needs `allow-overlay`. */
    function activateOverlayToWebPage(url) {
        return core.invoke(`${PREFIX}activate_overlay_to_web_page`, { url });
    }
    /**
     * Writes a whole file to Steam Cloud, replacing any file of that name. Names use ASCII letters,
     * digits and ` ._-`, with `/` between folders, and files are at most 100 MiB. Strings are written as
     * UTF-8. Needs `allow-cloud`. Prefer Auto-Cloud for save games; this is for apps that cannot use it.
     */
    function cloudWrite(name, data) {
        const bytes = typeof data === 'string' ? new TextEncoder().encode(data) : data;
        return core.invoke(`${PREFIX}cloud_write`, bytes, { headers: { [CLOUD_NAME_HEADER]: name } });
    }
    /** Reads a whole Cloud file. Rejects when the file does not exist. */
    async function cloudRead(name) {
        const data = await core.invoke(`${PREFIX}cloud_read`, { name });
        return new Uint8Array(data);
    }
    /** Reads a whole Cloud file as UTF-8 text. */
    async function cloudReadText(name) {
        return new TextDecoder().decode(await cloudRead(name));
    }
    /** Deletes a Cloud file. Rejects when the file does not exist. */
    function cloudDelete(name) {
        return core.invoke(`${PREFIX}cloud_delete`, { name });
    }
    function cloudList() {
        return core.invoke(`${PREFIX}cloud_list`);
    }
    function cloudQuota() {
        return core.invoke(`${PREFIX}cloud_quota`);
    }
    /** Says whether the player's account and this app allow Steam Cloud. */
    function cloudStatus() {
        return core.invoke(`${PREFIX}cloud_status`);
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
    /** Fires when the Steam overlay opens or closes. */
    function onOverlayToggled(handler) {
        return event.listen(OVERLAY_TOGGLED_EVENT, (e) => handler(e.payload));
    }
    /** True when a rejection means Steam is not usable, as opposed to a bad id or a failed call. */
    function isUnavailableError(error) {
        const message = error instanceof Error ? error.message : String(error);
        return message.startsWith('unavailable:');
    }

    exports.ACHIEVEMENT_STORED_EVENT = ACHIEVEMENT_STORED_EVENT;
    exports.OVERLAY_TOGGLED_EVENT = OVERLAY_TOGGLED_EVENT;
    exports.STATS_RECEIVED_EVENT = STATS_RECEIVED_EVENT;
    exports.STATS_STORED_EVENT = STATS_STORED_EVENT;
    exports.activateOverlay = activateOverlay;
    exports.activateOverlayToWebPage = activateOverlayToWebPage;
    exports.clearAchievement = clearAchievement;
    exports.clearRichPresence = clearRichPresence;
    exports.cloudDelete = cloudDelete;
    exports.cloudList = cloudList;
    exports.cloudQuota = cloudQuota;
    exports.cloudRead = cloudRead;
    exports.cloudReadText = cloudReadText;
    exports.cloudStatus = cloudStatus;
    exports.cloudWrite = cloudWrite;
    exports.getAchievement = getAchievement;
    exports.getPlayer = getPlayer;
    exports.getStat = getStat;
    exports.getStatus = getStatus;
    exports.indicateProgress = indicateProgress;
    exports.isAvailable = isAvailable;
    exports.isUnavailableError = isUnavailableError;
    exports.onAchievementStored = onAchievementStored;
    exports.onOverlayToggled = onOverlayToggled;
    exports.onStatsReceived = onStatsReceived;
    exports.onStatsStored = onStatsStored;
    exports.setRichPresence = setRichPresence;
    exports.setStat = setStat;
    exports.storeStats = storeStats;
    exports.unlockAchievement = unlockAchievement;

    return exports;

})({}, __TAURI__.core, __TAURI__.event);
Object.defineProperty(window.__TAURI__, 'steamworks', { value: __TAURI_PLUGIN_STEAMWORKS__ }) }
