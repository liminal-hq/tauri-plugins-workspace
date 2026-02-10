import { invoke } from '@tauri-apps/api/core';

/**
 * Retrieves Material You dynamic palette data from the platform plugin.
 * Desktop platforms return `supported: false`.
 */
export async function getMaterialYouColours() {
	return await invoke('plugin:material-you|get_material_you_colours');
}
