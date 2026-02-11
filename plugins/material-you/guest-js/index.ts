import { invoke } from '@tauri-apps/api/core';

/**
 * Material You system palette groups exposed by Android.
 */
export type MaterialYouPaletteName =
	| 'system_accent1'
	| 'system_accent2'
	| 'system_accent3'
	| 'system_neutral1'
	| 'system_neutral2';

/**
 * Tone map for a single Material You palette.
 *
 * Keys are tone values (for example `"0"`, `"500"`, `"1000"`), and values are
 * ARGB hex strings (for example `#FF4285F4`).
 */
export type MaterialYouPaletteTones = Record<string, string>;

/**
 * Collection of available Material You palettes keyed by palette name.
 */
export type MaterialYouPalettes = Partial<Record<MaterialYouPaletteName, MaterialYouPaletteTones>>;

/**
 * Response returned by the Material You plugin.
 */
export interface MaterialYouResponse {
	/**
	 * Whether Material You palette extraction is supported on the current device.
	 */
	supported: boolean;
	/**
	 * Android API level reported by the device.
	 */
	apiLevel: number;
	/**
	 * Extracted system palettes.
	 */
	palettes: MaterialYouPalettes;
}

/**
 * Retrieves Material You dynamic palette data from Android system resources.
 *
 * This plugin is Android-only. On unsupported devices, `supported` is `false`
 * and `palettes` may be empty.
 *
 * @returns Material You support state, Android API level, and available palettes.
 */
export async function getMaterialYouColours(): Promise<MaterialYouResponse> {
	return await invoke<MaterialYouResponse>('plugin:material-you|get_material_you_colours');
}
