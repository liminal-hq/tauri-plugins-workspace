import { beforeEach, describe, expect, it, vi } from 'vitest';

describe('material-you guest-js API', () => {
	const invokeMock = vi.fn();

	beforeEach(() => {
		invokeMock.mockReset();
		globalThis.window = {
			__TAURI_INTERNALS__: {
				invoke: invokeMock,
			},
		};
	});

	it('invokes the material-you command', async () => {
		const { getMaterialYouColours } = await import('../plugins/material-you/guest-js/index');

		invokeMock.mockResolvedValue({
			supported: true,
			apiLevel: 34,
			palettes: {},
		});

		const result = await getMaterialYouColours();

		expect(invokeMock).toHaveBeenCalledTimes(1);
		expect(invokeMock).toHaveBeenCalledWith(
			'plugin:material-you|get_material_you_colours',
			{},
			undefined
		);
		expect(result).toEqual({
			supported: true,
			apiLevel: 34,
			palettes: {},
		});
	});
});
