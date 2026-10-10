import js from '@eslint/js';
import tsParser from '@typescript-eslint/parser';

const baseIgnores = {
	ignores: ['**/node_modules/**', '**/dist-js/**', '**/target/**', '**/*.d.ts', '**/api-iife.js'],
};

export default [
	js.configs.recommended,
	{
		files: ['**/*.js'],
		languageOptions: {
			ecmaVersion: 2022,
			sourceType: 'module',
			globals: {
				console: 'readonly',
				process: 'readonly',
			},
		},
		rules: {
			'no-unused-vars': ['error', { argsIgnorePattern: '^_' }],
			'no-console': 'off',
		},
	},
	{
		files: ['**/*.ts'],
		languageOptions: {
			parser: tsParser,
			ecmaVersion: 2022,
			sourceType: 'module',
			globals: {
				console: 'readonly',
				process: 'readonly',
			},
		},
		rules: {
			'no-unused-vars': ['error', { argsIgnorePattern: '^_' }],
			'no-console': 'off',
		},
	},
	{
		// The scheduler uses browser timers, and callback types name their parameters.
		files: ['plugins/gamepad-haptics/guest-js/**/*.ts', 'tests/gamepad-haptics/**/*.ts'],
		languageOptions: {
			globals: {
				setTimeout: 'readonly',
				clearTimeout: 'readonly',
				navigator: 'readonly',
				window: 'readonly',
				URL: 'readonly',
			},
		},
		rules: {
			'no-unused-vars': 'off',
		},
	},
	baseIgnores,
];
