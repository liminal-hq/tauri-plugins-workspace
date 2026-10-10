// Tauri command handlers exposed to the webview
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

use tauri::{command, AppHandle, Runtime};

use crate::{
    error::Result,
    models::{Capabilities, PadInfo, PlayFramesArgs, PlayResult},
    GamepadHapticsExt,
};

#[command]
pub(crate) async fn capabilities<R: Runtime>(app: AppHandle<R>) -> Result<Capabilities> {
    app.gamepad_haptics().capabilities()
}

/// Buzzes one pad so the player can tell which it is.
#[command]
pub(crate) async fn identify<R: Runtime>(app: AppHandle<R>, pad_id: String) -> Result<PlayResult> {
    app.gamepad_haptics().identify(&pad_id)
}

#[command]
pub(crate) async fn list_pads<R: Runtime>(app: AppHandle<R>) -> Result<Vec<PadInfo>> {
    app.gamepad_haptics().list_pads()
}

#[command]
pub(crate) async fn play_frames<R: Runtime>(
    app: AppHandle<R>,
    args: PlayFramesArgs,
) -> Result<PlayResult> {
    app.gamepad_haptics().play_frames(args)
}

/// Stops one pad, or every pad when `pad_id` is absent.
#[command]
pub(crate) async fn stop<R: Runtime>(app: AppHandle<R>, pad_id: Option<String>) -> Result<()> {
    app.gamepad_haptics().stop(pad_id.as_deref())
}
