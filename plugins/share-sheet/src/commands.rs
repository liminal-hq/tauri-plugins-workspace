// Implements the IPC commands exposed by the share-sheet plugin
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

use tauri::{command, AppHandle, Runtime, WebviewWindow};

use crate::{
    error::Result,
    models::{ShareCapabilities, ShareFileOptions, ShareResult, ShareTextOptions},
    ShareSheetExt,
};

/// Shows the share sheet for a file over the calling window.
#[command]
pub(crate) async fn share_file<R: Runtime>(
    app: AppHandle<R>,
    window: WebviewWindow<R>,
    options: ShareFileOptions,
) -> Result<ShareResult> {
    app.share_sheet().share_file(&window, options).await
}

/// Shows the share sheet for text over the calling window.
#[command]
pub(crate) async fn share_text<R: Runtime>(
    app: AppHandle<R>,
    window: WebviewWindow<R>,
    options: ShareTextOptions,
) -> Result<ShareResult> {
    app.share_sheet().share_text(&window, options).await
}

/// Reports what can be shared, so a UI can hide its Share button.
#[command]
pub(crate) async fn can_share<R: Runtime>(
    app: AppHandle<R>,
    window: WebviewWindow<R>,
) -> ShareCapabilities {
    app.share_sheet().can_share(&window).await
}
