// Forwards share requests to the Swift plugin, which presents UIActivityViewController
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

use serde::de::DeserializeOwned;
use tauri::{
    plugin::{PluginApi, PluginHandle},
    AppHandle, Runtime, WebviewWindow,
};

use crate::{
    error::{Error, Result},
    models::{ShareCapabilities, SharePlatform, ShareResult},
    native,
    validate::{self, FileRequest, TextRequest},
};

tauri::ios_plugin_binding!(init_plugin_share_sheet);

pub struct Backend<R: Runtime> {
    handle: PluginHandle<R>,
}

pub fn init<R: Runtime, C: DeserializeOwned>(
    _app: &AppHandle<R>,
    api: PluginApi<R, C>,
) -> Result<Backend<R>> {
    let handle = api.register_ios_plugin(init_plugin_share_sheet)?;
    Ok(Backend { handle })
}

impl<R: Runtime> Backend<R> {
    pub async fn capabilities(&self, _window: &WebviewWindow<R>) -> ShareCapabilities {
        ShareCapabilities::for_platform(SharePlatform::Ios)
    }

    /// The app sandbox already limits what the process can read, so any existing file is offered.
    pub async fn share_file(
        &self,
        _window: &WebviewWindow<R>,
        request: FileRequest,
    ) -> Result<ShareResult> {
        let path = validate::resolve_existing_file(&request.path)?;
        let path = path
            .to_str()
            .ok_or_else(|| Error::invalid_path("the path is not valid UTF-8"))?;
        let result = self
            .handle
            .run_mobile_plugin_async(
                "shareFile",
                native::FileArgs {
                    path,
                    mime: &request.mime,
                    title: request.title.as_deref(),
                },
            )
            .await?;
        Ok(result)
    }

    pub async fn share_text(
        &self,
        _window: &WebviewWindow<R>,
        request: TextRequest,
    ) -> Result<ShareResult> {
        let result = self
            .handle
            .run_mobile_plugin_async(
                "shareText",
                native::TextArgs {
                    text: &request.text,
                    title: request.title.as_deref(),
                },
            )
            .await?;
        Ok(result)
    }
}
