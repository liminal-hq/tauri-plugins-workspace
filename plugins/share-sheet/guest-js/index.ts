// Exposes typed guest-side wrappers for the share-sheet plugin
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import { invoke } from '@tauri-apps/api/core';
import type { ErrorCode } from './bindings/ErrorCode';
import type { ShareCapabilities } from './bindings/ShareCapabilities';
import type { ShareFileOptions } from './bindings/ShareFileOptions';
import type { ShareResult } from './bindings/ShareResult';
import type { ShareStatus } from './bindings/ShareStatus';
import type { ShareTextOptions } from './bindings/ShareTextOptions';

const PREFIX = 'plugin:share-sheet|';

const ERROR_CODES: readonly ErrorCode[] = [
	'unsupported',
	'invalid-argument',
	'invalid-path',
	'not-found',
	'failed',
];

/**
 * A rejected share. `code` says why: `unsupported` where there is no share UI (Linux, for one),
 * `invalid-argument` for an empty text or a malformed MIME type, `invalid-path` for a path that is
 * not absolute, not a regular file or (on Android) outside the app's cache and files directories,
 * `not-found` when no file exists there, and `failed` for anything else, including a command the
 * app's capabilities do not allow.
 */
export class ShareSheetError extends Error {
	readonly code: ErrorCode;

	constructor(code: ErrorCode, message: string) {
		super(message);
		this.code = code;
		this.name = 'ShareSheetError';
	}
}

function isErrorCode(value: unknown): value is ErrorCode {
	return typeof value === 'string' && (ERROR_CODES as readonly string[]).includes(value);
}

/** Turns whatever the IPC layer rejected with into a `ShareSheetError`. */
function toShareSheetError(error: unknown): ShareSheetError {
	if (error instanceof ShareSheetError) return error;
	if (typeof error === 'object' && error !== null) {
		const { code, message } = error as { code?: unknown; message?: unknown };
		const text = typeof message === 'string' ? message : String(error);
		return new ShareSheetError(isErrorCode(code) ? code : 'failed', text);
	}
	return new ShareSheetError('failed', typeof error === 'string' ? error : String(error));
}

async function cmd<T>(name: string, args?: Record<string, unknown>): Promise<T> {
	try {
		return await invoke<T>(`${PREFIX}${name}`, args);
	} catch (error) {
		throw toShareSheetError(error);
	}
}

/**
 * Hands a file to the operating system's share sheet and resolves when the platform says how it
 * ended: `shared` when the person picked a target, `cancelled` when they dismissed the sheet, and
 * `unknown` when the platform does not say. On Android the file must be inside the app's cache or
 * files directory. Rejects with a `ShareSheetError`.
 */
export function shareFile(options: ShareFileOptions): Promise<ShareResult> {
	return cmd<ShareResult>('share_file', { options });
}

/** Hands text to the operating system's share sheet. Resolves and rejects as `shareFile` does. */
export function shareText(options: ShareTextOptions): Promise<ShareResult> {
	return cmd<ShareResult>('share_text', { options });
}

/** Reports whether a share sheet exists here and what it takes, so a UI can hide its Share button. */
export function canShare(): Promise<ShareCapabilities> {
	return cmd<ShareCapabilities>('can_share');
}

export type {
	ErrorCode,
	ShareCapabilities,
	ShareFileOptions,
	ShareResult,
	ShareStatus,
	ShareTextOptions,
};
