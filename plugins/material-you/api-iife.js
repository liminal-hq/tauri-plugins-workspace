if ('__TAURI__' in window) {
var __TAURI_PLUGIN_MATERIAL_YOU__ = (function (exports, core) {
    'use strict';

    /**
     * Retrieves Material You dynamic palette data from Android system resources.
     *
     * This plugin is Android-only. On unsupported devices, `supported` is `false`
     * and `palettes` may be empty.
     *
     * @returns Material You support state, Android API level, and available palettes.
     */
    async function getMaterialYouColours() {
        return await core.invoke('plugin:material-you|get_material_you_colours');
    }

    exports.getMaterialYouColours = getMaterialYouColours;

    return exports;

})({}, __TAURI__.core);
Object.defineProperty(window.__TAURI__, 'materialYou', { value: __TAURI_PLUGIN_MATERIAL_YOU__ }) }
