// Reports no share UI on targets without one, such as Linux desktops
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

use std::marker::PhantomData;

use serde::de::DeserializeOwned;
use tauri::{plugin::PluginApi, AppHandle, Runtime, WebviewWindow};

use crate::{
    error::{Error, Result},
    models::{ShareCapabilities, SharePlatform, ShareResult},
    share::UNSUPPORTED_REASON,
    validate::{FileRequest, TextRequest},
};

pub struct Backend<R: Runtime>(PhantomData<fn() -> R>);

pub fn init<R: Runtime, C: DeserializeOwned>(
    _app: &AppHandle<R>,
    _api: PluginApi<R, C>,
) -> Result<Backend<R>> {
    Ok(Backend(PhantomData))
}

impl<R: Runtime> Backend<R> {
    pub async fn capabilities(&self, _window: &WebviewWindow<R>) -> ShareCapabilities {
        ShareCapabilities::for_platform(SharePlatform::current())
    }

    pub async fn share_file(
        &self,
        _window: &WebviewWindow<R>,
        _request: FileRequest,
    ) -> Result<ShareResult> {
        Err(Error::unsupported(UNSUPPORTED_REASON))
    }

    pub async fn share_text(
        &self,
        _window: &WebviewWindow<R>,
        _request: TextRequest,
    ) -> Result<ShareResult> {
        Err(Error::unsupported(UNSUPPORTED_REASON))
    }
}
