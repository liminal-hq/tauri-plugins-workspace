if ('__TAURI__' in window) {
var __TAURI_PLUGIN_HDMV__ = (function (exports, core) {
    'use strict';

    // -- Session lifecycle --
    /**
     * Open a BDMV folder and return a session ID for subsequent commands.
     */
    async function openDisc(path) {
        return await core.invoke('plugin:hdmv|hdmv_open_disc', { path });
    }
    /**
     * Close a disc session and free its resources.
     */
    async function closeDisc(sessionId) {
        await core.invoke('plugin:hdmv|hdmv_close_disc', { sessionId });
    }
    // -- Disc inspection --
    /**
     * Get a summary of the disc's top-level structure.
     */
    async function getDiscInfo(sessionId) {
        return await core.invoke('plugin:hdmv|hdmv_get_disc_info', { sessionId });
    }
    /**
     * List all title entries from the disc index.
     */
    async function listTitles(sessionId) {
        return await core.invoke('plugin:hdmv|hdmv_list_titles', { sessionId });
    }
    /**
     * List playlist summaries.
     */
    async function listPlaylists(sessionId) {
        return await core.invoke('plugin:hdmv|hdmv_list_playlists', { sessionId });
    }
    /**
     * Get full playlist detail including play items and chapters.
     */
    async function getPlaylist(sessionId, playlistIndex) {
        return await core.invoke('plugin:hdmv|hdmv_get_playlist', {
            sessionId,
            playlistIndex,
        });
    }
    // -- Navigation --
    /**
     * Execute the First Play object and return initial navigation events.
     */
    async function startNavigation(sessionId) {
        return await core.invoke('plugin:hdmv|hdmv_start_navigation', { sessionId });
    }
    /**
     * Load an interactive menu scene from externally-parsed IGS data.
     *
     * Must be called before scene-dependent commands (sendKey, mouseMove,
     * mouseClick, renderPreview) will work.
     */
    async function loadScene(sessionId, sceneData) {
        await core.invoke('plugin:hdmv|hdmv_load_scene', { sessionId, sceneData });
    }
    /**
     * Send a remote key input and return resulting navigation events.
     */
    async function sendKey(sessionId, key) {
        return await core.invoke('plugin:hdmv|hdmv_send_key', { sessionId, key });
    }
    /**
     * Hit-test mouse position, return button ID if hovering over one.
     */
    async function mouseMove(sessionId, x, y) {
        return await core.invoke('plugin:hdmv|hdmv_mouse_move', { sessionId, x, y });
    }
    /**
     * Click at a position and return resulting navigation events.
     */
    async function mouseClick(sessionId, x, y) {
        return await core.invoke('plugin:hdmv|hdmv_mouse_click', { sessionId, x, y });
    }
    // -- Rendering --
    /**
     * Render the current menu page as a base64-encoded PNG.
     */
    async function renderPreview(sessionId, maxWidth) {
        return await core.invoke('plugin:hdmv|hdmv_render_preview', { sessionId, maxWidth });
    }
    /**
     * Get a snapshot of the current menu state.
     */
    async function getMenuState(sessionId) {
        return await core.invoke('plugin:hdmv|hdmv_get_menu_state', { sessionId });
    }
    // -- Authoring --
    /**
     * Build a new BDMV disc structure from configuration.
     */
    async function buildDisc(config) {
        await core.invoke('plugin:hdmv|hdmv_build_disc', { config });
    }

    exports.buildDisc = buildDisc;
    exports.closeDisc = closeDisc;
    exports.getDiscInfo = getDiscInfo;
    exports.getMenuState = getMenuState;
    exports.getPlaylist = getPlaylist;
    exports.listPlaylists = listPlaylists;
    exports.listTitles = listTitles;
    exports.loadScene = loadScene;
    exports.mouseClick = mouseClick;
    exports.mouseMove = mouseMove;
    exports.openDisc = openDisc;
    exports.renderPreview = renderPreview;
    exports.sendKey = sendKey;
    exports.startNavigation = startNavigation;

    return exports;

})({}, __TAURI__.core);
Object.defineProperty(window.__TAURI__, 'hdmv', { value: __TAURI_PLUGIN_HDMV__ }) }
