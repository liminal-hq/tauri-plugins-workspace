import { readFileSync } from 'node:fs';
import { join } from 'node:path';

/**
 * Creates Rollup configuration for building Tauri plugin guest bindings.
 *
 * @param {string} pluginName - The plugin name (for example, 'alarm-manager').
 * @returns {import('rollup').RollupOptions[]} Rollup configs.
 */
export function createConfig(pluginName) {
	// Read package.json to use package-level output paths.
	const pkg = JSON.parse(readFileSync(join(process.cwd(), 'package.json'), 'utf8'));

	return [
		// ESM and CJS builds for npm distribution.
		{
			input: 'guest-js/index.ts',
			output: [
				{
					file: pkg.exports.import,
					format: 'es',
				},
				{
					file: pkg.exports.require,
					format: 'cjs',
				},
			],
			external: [/^@tauri-apps\/api/],
			plugins: [
				typescript({
					declaration: true,
					declarationDir: './dist-js',
				}),
			],
		},
		// IIFE build for global mode (used by build.rs).
		{
			input: 'guest-js/index.ts',
			output: {
				file: 'api-iife.js',
				format: 'iife',
				name: `__TAURI_PLUGIN_${pluginName.toUpperCase().replace(/-/g, '_')}__`,
			},
			external: [/^@tauri-apps\/api/],
			plugins: [typescript()],
		},
	];
}

// Simple TypeScript plugin placeholder.
// Replace with @rollup/plugin-typescript for production use.
function typescript(options = {}) {
	return {
		name: 'typescript',
		transform(code, id) {
			if (!id.endsWith('.ts')) {
				return null;
			}

			return {
				code: code.replace(/: \w+/g, '').replace(/import type .*/g, ''),
				map: null,
			};
		},
		options,
	};
}

export default createConfig;
