if ('__TAURI__' in window) {
var __TAURI_PLUGIN_SHARE_SHEET__ = (function (exports, core) {
    'use strict';

    // Exposes typed guest-side wrappers for the share-sheet plugin
    //
    // (c) Copyright 2026 Liminal HQ, Scott Morris
    // SPDX-License-Identifier: Apache-2.0 OR MIT
    const PREFIX = 'plugin:share-sheet|';
    const ERROR_CODES = [
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
    class ShareSheetError extends Error {
        code;
        constructor(code, message) {
            super(message);
            this.code = code;
            this.name = 'ShareSheetError';
        }
    }
    function isErrorCode(value) {
        return typeof value === 'string' && ERROR_CODES.includes(value);
    }
    /** Turns whatever the IPC layer rejected with into a `ShareSheetError`. */
    function toShareSheetError(error) {
        if (error instanceof ShareSheetError)
            return error;
        if (typeof error === 'object' && error !== null) {
            const { code, message } = error;
            const text = typeof message === 'string' ? message : String(error);
            return new ShareSheetError(isErrorCode(code) ? code : 'failed', text);
        }
        return new ShareSheetError('failed', typeof error === 'string' ? error : String(error));
    }
    async function cmd(name, args) {
        try {
            return await core.invoke(`${PREFIX}${name}`, args);
        }
        catch (error) {
            throw toShareSheetError(error);
        }
    }
    /**
     * Hands a file to the operating system's share sheet and resolves when the platform says how it
     * ended: `shared` when the person picked a target, `cancelled` when they dismissed the sheet, and
     * `unknown` when the platform does not say. On Android the file must be inside the app's cache or
     * files directory. Rejects with a `ShareSheetError`.
     */
    function shareFile(options) {
        return cmd('share_file', { options });
    }
    /** Hands text to the operating system's share sheet. Resolves and rejects as `shareFile` does. */
    function shareText(options) {
        return cmd('share_text', { options });
    }
    /** Reports whether a share sheet exists here and what it takes, so a UI can hide its Share button. */
    function canShare() {
        return cmd('can_share');
    }

    exports.ShareSheetError = ShareSheetError;
    exports.canShare = canShare;
    exports.shareFile = shareFile;
    exports.shareText = shareText;

    return exports;

})({}, __TAURI__.core);
Object.defineProperty(window.__TAURI__, 'shareSheet', { value: __TAURI_PLUGIN_SHARE_SHEET__ }) }
