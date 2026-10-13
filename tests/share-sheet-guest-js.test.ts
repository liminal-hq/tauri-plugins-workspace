// Unit tests for the share-sheet guest bindings, with the Tauri bridge mocked
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import { beforeEach, describe, expect, it, vi } from 'vitest';

const invoke = vi.fn();

// The plugin's `@tauri-apps/api` copy is not resolvable from here, so mock the bridge it calls.
(globalThis as unknown as { window: unknown }).window = {
	__TAURI_INTERNALS__: { invoke },
};

type Api = typeof import('../plugins/share-sheet/guest-js/index');
let api: Api;

beforeEach(async () => {
	invoke.mockReset();
	api = await import('../plugins/share-sheet/guest-js/index');
});

describe('share-sheet guest-js API', () => {
	it('sends a file share as options and returns the status', async () => {
		invoke.mockResolvedValue({ status: 'shared' });
		const result = await api.shareFile({
			path: '/data/user/0/app/cache/save.zip',
			mime: 'application/zip',
			title: 'Save',
		});
		expect(result).toEqual({ status: 'shared' });
		expect(invoke).toHaveBeenCalledWith(
			'plugin:share-sheet|share_file',
			{
				options: {
					path: '/data/user/0/app/cache/save.zip',
					mime: 'application/zip',
					title: 'Save',
				},
			},
			undefined
		);
	});

	it('sends a text share as options', async () => {
		invoke.mockResolvedValue({ status: 'unknown' });
		await expect(api.shareText({ text: 'I beat level 3' })).resolves.toEqual({
			status: 'unknown',
		});
		expect(invoke).toHaveBeenCalledWith(
			'plugin:share-sheet|share_text',
			{ options: { text: 'I beat level 3' } },
			undefined
		);
	});

	it('asks what can be shared', async () => {
		invoke.mockResolvedValue({ supported: false, files: false, text: false });
		await expect(api.canShare()).resolves.toEqual({
			supported: false,
			files: false,
			text: false,
		});
		expect(invoke).toHaveBeenCalledWith('plugin:share-sheet|can_share', {}, undefined);
	});

	it.each(['unsupported', 'invalid-argument', 'invalid-path', 'not-found', 'failed'] as const)(
		'rejects with a ShareSheetError carrying the %s code',
		async (code) => {
			invoke.mockRejectedValue({ code, message: `because ${code}` });
			const error = await api.shareText({ text: 'hi' }).catch((e: unknown) => e);
			expect(error).toBeInstanceOf(api.ShareSheetError);
			expect(error).toMatchObject({ name: 'ShareSheetError', code, message: `because ${code}` });
		}
	);

	it('reports a plain string rejection, such as a permission denial, as failed', async () => {
		invoke.mockRejectedValue('share-sheet.share_file not allowed');
		const error = await api
			.shareFile({ path: '/a.zip', mime: 'application/zip' })
			.catch((e: unknown) => e);
		expect(error).toBeInstanceOf(api.ShareSheetError);
		expect(error).toMatchObject({
			code: 'failed',
			message: 'share-sheet.share_file not allowed',
		});
	});

	it('reports an unrecognised code as failed', async () => {
		invoke.mockRejectedValue({ code: 'busy', message: 'busy' });
		const error = await api.canShare().catch((e: unknown) => e);
		expect(error).toMatchObject({ code: 'failed', message: 'busy' });
	});
});
