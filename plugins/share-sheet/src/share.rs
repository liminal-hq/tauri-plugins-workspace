// Checks each share request and hands it to the platform backend
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

use tauri::{Runtime, WebviewWindow};

use crate::{
    error::{Error, Result},
    models::{ShareCapabilities, ShareFileOptions, ShareResult, ShareTextOptions},
    platform, validate,
};

/// Why sharing is unavailable on this target, for the `unsupported` error.
pub(crate) const UNSUPPORTED_REASON: &str = if cfg!(target_os = "linux") {
    "Linux desktops have no standard share UI (xdg-desktop-portal has no share portal), so there is nothing to show"
} else {
    "this system has no share UI"
};

/// The share sheet, managed as plugin state. Reach it with [`crate::ShareSheetExt::share_sheet`].
pub struct ShareSheet<R: Runtime> {
    backend: platform::Backend<R>,
}

impl<R: Runtime> ShareSheet<R> {
    pub(crate) fn new(backend: platform::Backend<R>) -> Self {
        Self { backend }
    }

    /// What can be shared from `window` on this system.
    pub async fn can_share(&self, window: &WebviewWindow<R>) -> ShareCapabilities {
        self.backend.capabilities(window).await
    }

    /// Shows the share sheet for a file, over `window`, and resolves when the platform says how it
    /// ended. Rejects with `unsupported` before looking at the arguments where there is no share UI.
    pub async fn share_file(
        &self,
        window: &WebviewWindow<R>,
        options: ShareFileOptions,
    ) -> Result<ShareResult> {
        if !self.can_share(window).await.files {
            return Err(Error::unsupported(UNSUPPORTED_REASON));
        }
        let request = validate::file_request(&options)?;
        self.backend.share_file(window, request).await
    }

    /// Shows the share sheet for text, over `window`.
    pub async fn share_text(
        &self,
        window: &WebviewWindow<R>,
        options: ShareTextOptions,
    ) -> Result<ShareResult> {
        if !self.can_share(window).await.text {
            return Err(Error::unsupported(UNSUPPORTED_REASON));
        }
        let request = validate::text_request(&options)?;
        self.backend.share_text(window, request).await
    }
}
