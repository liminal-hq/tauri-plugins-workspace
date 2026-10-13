// Forwards share requests to the Kotlin plugin, after checking the file is in the app's storage
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

use std::path::PathBuf;

use serde::{de::DeserializeOwned, Deserialize};
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

const PLUGIN_IDENTIFIER: &str = "ca.liminalhq.sharesheet";

pub struct Backend<R: Runtime> {
    handle: PluginHandle<R>,
}

pub fn init<R: Runtime, C: DeserializeOwned>(
    _app: &AppHandle<R>,
    api: PluginApi<R, C>,
) -> Result<Backend<R>> {
    let handle = api.register_android_plugin(PLUGIN_IDENTIFIER, "ShareSheetPlugin")?;
    Ok(Backend { handle })
}

/// The directories the plugin's FileProvider exposes, as `Context.getCacheDir` and `getFilesDir`.
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct AppDirs {
    cache_dir: PathBuf,
    files_dir: PathBuf,
}

impl<R: Runtime> Backend<R> {
    pub async fn capabilities(&self, _window: &WebviewWindow<R>) -> ShareCapabilities {
        ShareCapabilities::for_platform(SharePlatform::Android)
    }

    pub async fn share_file(
        &self,
        _window: &WebviewWindow<R>,
        request: FileRequest,
    ) -> Result<ShareResult> {
        let dirs: AppDirs = self.handle.run_mobile_plugin_async("appDirs", ()).await?;
        let path = validate::resolve_file_within(&request.path, &[dirs.cache_dir, dirs.files_dir])?;
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
