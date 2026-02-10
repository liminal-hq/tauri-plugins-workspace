import { invoke } from '@tauri-apps/api/core';

/**
 * Retrieves Material You dynamic palette data from Android system resources.
 * This plugin is Android-only.
 */
export async function getMaterialYouColours() {
	return await invoke('plugin:material-you|get_material_you_colours');
}
