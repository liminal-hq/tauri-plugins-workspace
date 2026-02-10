import js from '@eslint/js';

export default [
	js.configs.recommended,
	{
		files: ['**/*.js', '**/*.ts'],
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
		ignores: ['**/node_modules/**', '**/dist-js/**', '**/target/**', '**/*.d.ts', '**/api-iife.js'],
	},
];
