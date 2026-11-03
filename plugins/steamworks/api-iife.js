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
    const SCREENSHOT_REQUESTED_EVENT = 'steamworks://screenshot-requested';
    const LOBBY_CHAT_UPDATE_EVENT = 'steamworks://lobby-chat-update';
    const LOBBY_DATA_UPDATE_EVENT = 'steamworks://lobby-data-update';
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
    /** Lists the app's downloadable content and whether each is installed. Needs `allow-dlc`. */
    function getDlc() {
        return core.invoke(`${PREFIX}get_dlc`);
    }
    function isDlcInstalled(appId) {
        return core.invoke(`${PREFIX}is_dlc_installed`, { appId });
    }
    /** Says whether the app runs on a Steam Deck or in Big Picture mode. Needs `allow-device`. */
    function getDeviceInfo() {
        return core.invoke(`${PREFIX}get_device_info`);
    }
    async function isSteamDeck() {
        return (await getDeviceInfo()).steamDeck;
    }
    /**
     * Makes Steam send a `screenshot-requested` event, and leaves the capture to the app, instead of
     * taking screenshots itself. Do not enable it when an overlay-surface plugin already hooks
     * screenshots. Needs `allow-screenshots`.
     */
    function hookScreenshots(hook) {
        return core.invoke(`${PREFIX}hook_screenshots`, { hook });
    }
    /** Asks Steam for a screenshot; when hooked, the app gets `screenshot-requested` instead. */
    function triggerScreenshot() {
        return core.invoke(`${PREFIX}trigger_screenshot`);
    }
    /**
     * Adds an image the app saved to the player's Steam screenshot library. The path must be absolute
     * and name an existing `png`, `jpg`, `jpeg` or `tga` file.
     */
    function addScreenshotToLibrary(path, width, height) {
        return core.invoke(`${PREFIX}add_screenshot_to_library`, { path, width, height });
    }
    /**
     * Creates a lobby and resolves with its ID. Steam allows 1 to 250 members. Lobby calls need
     * `allow-lobbies`.
     */
    function createLobby(kind, maxMembers) {
        return core.invoke(`${PREFIX}create_lobby`, { kind, maxMembers });
    }
    /** Joins a lobby and resolves with its ID. Rejects when the lobby is full, closed or unknown. */
    function joinLobby(id) {
        return core.invoke(`${PREFIX}join_lobby`, { id });
    }
    /** Lists open public lobbies using Steam's default search. */
    function listLobbies() {
        return core.invoke(`${PREFIX}list_lobbies`);
    }
    function leaveLobby(id) {
        return core.invoke(`${PREFIX}leave_lobby`, { id });
    }
    function getLobby(id) {
        return core.invoke(`${PREFIX}get_lobby`, { id });
    }
    /** Sets a lobby data key, or removes it with `null`. Only the lobby owner can change data. */
    function setLobbyData(id, key, value) {
        return core.invoke(`${PREFIX}set_lobby_data`, { id, key, value });
    }
    /** Opens or closes a lobby to new members. Only the lobby owner can change it. */
    function setLobbyJoinable(id, joinable) {
        return core.invoke(`${PREFIX}set_lobby_joinable`, { id, joinable });
    }
    /** Lists the Workshop items the player is subscribed to. Needs `allow-workshop`. */
    function getWorkshopItems() {
        return core.invoke(`${PREFIX}get_workshop_items`);
    }
    function subscribeWorkshopItem(id) {
        return core.invoke(`${PREFIX}subscribe_workshop_item`, { id });
    }
    function unsubscribeWorkshopItem(id) {
        return core.invoke(`${PREFIX}unsubscribe_workshop_item`, { id });
    }
    /** Starts or prioritises a download. Resolves with whether Steam accepted the request. */
    function downloadWorkshopItem(id, highPriority = false) {
        return core.invoke(`${PREFIX}download_workshop_item`, { id, highPriority });
    }
    /**
     * Starts Steam Input; call it once before the other input functions. Needs `allow-input` and an
     * action manifest configured for the app in Steamworks. Steam updates input whenever callbacks run,
     * so a short `callbackIntervalMs` makes input more responsive.
     */
    function inputInit() {
        return core.invoke(`${PREFIX}input_init`);
    }
    function getInputControllers() {
        return core.invoke(`${PREFIX}get_input_controllers`);
    }
    /** Looks up an action set from the app's action manifest and resolves with its handle. */
    function getInputActionSet(name) {
        return core.invoke(`${PREFIX}get_input_action_set`, { name });
    }
    function activateInputActionSet(controller, actionSet) {
        return core.invoke(`${PREFIX}activate_input_action_set`, { controller, actionSet });
    }
    /** Looks up a digital or analog action from the action manifest and resolves with its handle. */
    function getInputAction(kind, name) {
        return core.invoke(`${PREFIX}get_input_action`, { kind, name });
    }
    function getDigitalActionState(controller, action) {
        return core.invoke(`${PREFIX}get_digital_action_state`, { controller, action });
    }
    function getAnalogActionState(controller, action) {
        return core.invoke(`${PREFIX}get_analog_action_state`, { controller, action });
    }
    /**
     * Resolves with the PNG of the first button bound to an action on a controller, for showing
     * prompts such as "Press (A)". Wrap it with `new Blob([bytes], { type: 'image/png' })` for an
     * image URL. Rejects when the action has no button bound.
     */
    async function getInputGlyph(controller, actionSet, kind, action, size = 'medium') {
        const data = await core.invoke(`${PREFIX}get_input_glyph`, {
            controller,
            actionSet,
            kind,
            action,
            size,
        });
        return new Uint8Array(data);
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
    /** Fires when Steam asks the app for a screenshot; needs `hookScreenshots(true)`. */
    function onScreenshotRequested(handler) {
        return event.listen(SCREENSHOT_REQUESTED_EVENT, () => handler());
    }
    /** Fires when a lobby member joins, leaves or is removed. */
    function onLobbyChatUpdate(handler) {
        return event.listen(LOBBY_CHAT_UPDATE_EVENT, (e) => handler(e.payload));
    }
    /** Fires when lobby data, or a member's data in a lobby, changes. */
    function onLobbyDataUpdate(handler) {
        return event.listen(LOBBY_DATA_UPDATE_EVENT, (e) => handler(e.payload));
    }
    /** True when a rejection means Steam is not usable, as opposed to a bad id or a failed call. */
    function isUnavailableError(error) {
        const message = error instanceof Error ? error.message : String(error);
        return message.startsWith('unavailable:');
    }

    exports.ACHIEVEMENT_STORED_EVENT = ACHIEVEMENT_STORED_EVENT;
    exports.LOBBY_CHAT_UPDATE_EVENT = LOBBY_CHAT_UPDATE_EVENT;
    exports.LOBBY_DATA_UPDATE_EVENT = LOBBY_DATA_UPDATE_EVENT;
    exports.OVERLAY_TOGGLED_EVENT = OVERLAY_TOGGLED_EVENT;
    exports.SCREENSHOT_REQUESTED_EVENT = SCREENSHOT_REQUESTED_EVENT;
    exports.STATS_RECEIVED_EVENT = STATS_RECEIVED_EVENT;
    exports.STATS_STORED_EVENT = STATS_STORED_EVENT;
    exports.activateInputActionSet = activateInputActionSet;
    exports.activateOverlay = activateOverlay;
    exports.activateOverlayToWebPage = activateOverlayToWebPage;
    exports.addScreenshotToLibrary = addScreenshotToLibrary;
    exports.clearAchievement = clearAchievement;
    exports.clearRichPresence = clearRichPresence;
    exports.cloudDelete = cloudDelete;
    exports.cloudList = cloudList;
    exports.cloudQuota = cloudQuota;
    exports.cloudRead = cloudRead;
    exports.cloudReadText = cloudReadText;
    exports.cloudStatus = cloudStatus;
    exports.cloudWrite = cloudWrite;
    exports.createLobby = createLobby;
    exports.downloadWorkshopItem = downloadWorkshopItem;
    exports.getAchievement = getAchievement;
    exports.getAnalogActionState = getAnalogActionState;
    exports.getDeviceInfo = getDeviceInfo;
    exports.getDigitalActionState = getDigitalActionState;
    exports.getDlc = getDlc;
    exports.getInputAction = getInputAction;
    exports.getInputActionSet = getInputActionSet;
    exports.getInputControllers = getInputControllers;
    exports.getInputGlyph = getInputGlyph;
    exports.getLobby = getLobby;
    exports.getPlayer = getPlayer;
    exports.getStat = getStat;
    exports.getStatus = getStatus;
    exports.getWorkshopItems = getWorkshopItems;
    exports.hookScreenshots = hookScreenshots;
    exports.indicateProgress = indicateProgress;
    exports.inputInit = inputInit;
    exports.isAvailable = isAvailable;
    exports.isDlcInstalled = isDlcInstalled;
    exports.isSteamDeck = isSteamDeck;
    exports.isUnavailableError = isUnavailableError;
    exports.joinLobby = joinLobby;
    exports.leaveLobby = leaveLobby;
    exports.listLobbies = listLobbies;
    exports.onAchievementStored = onAchievementStored;
    exports.onLobbyChatUpdate = onLobbyChatUpdate;
    exports.onLobbyDataUpdate = onLobbyDataUpdate;
    exports.onOverlayToggled = onOverlayToggled;
    exports.onScreenshotRequested = onScreenshotRequested;
    exports.onStatsReceived = onStatsReceived;
    exports.onStatsStored = onStatsStored;
    exports.setLobbyData = setLobbyData;
    exports.setLobbyJoinable = setLobbyJoinable;
    exports.setRichPresence = setRichPresence;
    exports.setStat = setStat;
    exports.storeStats = storeStats;
    exports.subscribeWorkshopItem = subscribeWorkshopItem;
    exports.triggerScreenshot = triggerScreenshot;
    exports.unlockAchievement = unlockAchievement;
    exports.unsubscribeWorkshopItem = unsubscribeWorkshopItem;

    return exports;

})({}, __TAURI__.core, __TAURI__.event);
Object.defineProperty(window.__TAURI__, 'steamworks', { value: __TAURI_PLUGIN_STEAMWORKS__ }) }
