import { invoke } from '@tauri-apps/api/core';

export type MaterialYouPaletteName =
	| 'system_accent1'
	| 'system_accent2'
	| 'system_accent3'
	| 'system_neutral1'
	| 'system_neutral2';

export type MaterialYouPaletteTones = Record<string, string>;

export type MaterialYouPalettes = Partial<Record<MaterialYouPaletteName, MaterialYouPaletteTones>>;

export interface MaterialYouResponse {
	supported: boolean;
	apiLevel: number;
	palettes: MaterialYouPalettes;
}

/**
 * Retrieves Material You dynamic palette data from Android system resources.
 * This plugin is Android-only.
 */
export async function getMaterialYouColours(): Promise<MaterialYouResponse> {
	return await invoke<MaterialYouResponse>('plugin:material-you|get_material_you_colours');
}
