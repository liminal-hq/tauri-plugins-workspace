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
		// The haptics guest bindings use browser timers and `structuredClone`, and declare callback
		// types with named parameters, which the base `no-unused-vars` rule misreads as unused.
		// `tsc --strict` already covers unused and undefined identifiers there.
		files: ['plugins/haptics/guest-js/**/*.ts', 'tests/haptics/**/*.ts'],
		languageOptions: {
			globals: {
				clearTimeout: 'readonly',
				document: 'readonly',
				setTimeout: 'readonly',
				structuredClone: 'readonly',
			},
		},
		rules: {
			'no-unused-vars': 'off',
		},
	},
	baseIgnores,
];
