// Tauri command handlers exposed to the webview
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

use std::collections::BTreeMap;

pub(crate) mod social;

use tauri::{command, AppHandle, Runtime};

use crate::{
    error::{Error, Result},
    models::{
        Achievement, CloudFile, CloudQuota, CloudStatus, DeviceInfo, DlcInfo, OverlayPage, Player,
        StatKind, StatValue, SteamStatus,
    },
    SteamworksExt,
};

#[command]
pub(crate) async fn is_available<R: Runtime>(app: AppHandle<R>) -> bool {
    app.steamworks().is_available()
}

#[command]
pub(crate) async fn get_status<R: Runtime>(app: AppHandle<R>) -> SteamStatus {
    app.steamworks().status()
}

#[command]
pub(crate) async fn get_player<R: Runtime>(app: AppHandle<R>) -> Result<Player> {
    app.steamworks().player()
}

#[command]
pub(crate) async fn get_achievement<R: Runtime>(
    app: AppHandle<R>,
    id: String,
) -> Result<Achievement> {
    app.steamworks().achievement(&id)
}

/// Unlocks an achievement and stores it so Steam shows its notification.
#[command]
pub(crate) async fn unlock_achievement<R: Runtime>(app: AppHandle<R>, id: String) -> Result<()> {
    app.steamworks().unlock_achievement(&id)
}

/// Re-locks an achievement. Meant for development builds.
#[command]
pub(crate) async fn clear_achievement<R: Runtime>(app: AppHandle<R>, id: String) -> Result<()> {
    app.steamworks().clear_achievement(&id)
}

#[command]
pub(crate) async fn indicate_achievement_progress<R: Runtime>(
    app: AppHandle<R>,
    id: String,
    current: u32,
    max: u32,
) -> Result<()> {
    app.steamworks().indicate_progress(&id, current, max)
}

#[command]
pub(crate) async fn get_stat<R: Runtime>(
    app: AppHandle<R>,
    name: String,
    kind: StatKind,
) -> Result<StatValue> {
    app.steamworks().get_stat(&name, kind)
}

#[command]
pub(crate) async fn set_stat<R: Runtime>(
    app: AppHandle<R>,
    name: String,
    value: StatValue,
) -> Result<()> {
    app.steamworks().set_stat(&name, value)
}

#[command]
pub(crate) async fn store_stats<R: Runtime>(app: AppHandle<R>) -> Result<()> {
    app.steamworks().store_stats()
}

/// Sets rich presence keys; a `null` value removes the key.
#[command]
pub(crate) async fn set_rich_presence<R: Runtime>(
    app: AppHandle<R>,
    entries: BTreeMap<String, Option<String>>,
) -> Result<()> {
    app.steamworks().set_rich_presence(&entries)
}

#[command]
pub(crate) async fn clear_rich_presence<R: Runtime>(app: AppHandle<R>) -> Result<()> {
    app.steamworks().clear_rich_presence()
}

#[command]
pub(crate) async fn activate_overlay<R: Runtime>(
    app: AppHandle<R>,
    page: OverlayPage,
) -> Result<()> {
    app.steamworks().activate_overlay(page)
}

#[command]
pub(crate) async fn activate_overlay_to_web_page<R: Runtime>(
    app: AppHandle<R>,
    url: String,
) -> Result<()> {
    app.steamworks().activate_overlay_to_web_page(&url)
}

/// The header that carries the cloud file name, so the request body can be the raw bytes.
pub(crate) const CLOUD_NAME_HEADER: &str = "x-steamworks-cloud-name";

/// Writes a Cloud file. The body is the raw file bytes and the name travels in a header.
#[command]
pub(crate) async fn cloud_write<R: Runtime>(
    app: AppHandle<R>,
    request: tauri::ipc::Request<'_>,
) -> Result<()> {
    let name = request
        .headers()
        .get(CLOUD_NAME_HEADER)
        .and_then(|v| v.to_str().ok())
        .ok_or_else(|| Error::InvalidRequest(format!("missing the {CLOUD_NAME_HEADER} header")))?
        .to_string();
    let tauri::ipc::InvokeBody::Raw(data) = request.body() else {
        return Err(Error::InvalidRequest(
            "cloud_write expects the raw file bytes as its body".into(),
        ));
    };
    app.steamworks().cloud_write(&name, data)
}

#[command]
pub(crate) async fn cloud_read<R: Runtime>(
    app: AppHandle<R>,
    name: String,
) -> Result<tauri::ipc::Response> {
    app.steamworks()
        .cloud_read(&name)
        .map(tauri::ipc::Response::new)
}

#[command]
pub(crate) async fn cloud_delete<R: Runtime>(app: AppHandle<R>, name: String) -> Result<()> {
    app.steamworks().cloud_delete(&name)
}

#[command]
pub(crate) async fn cloud_list<R: Runtime>(app: AppHandle<R>) -> Result<Vec<CloudFile>> {
    app.steamworks().cloud_list()
}

#[command]
pub(crate) async fn cloud_quota<R: Runtime>(app: AppHandle<R>) -> Result<CloudQuota> {
    app.steamworks().cloud_quota()
}

#[command]
pub(crate) async fn cloud_status<R: Runtime>(app: AppHandle<R>) -> Result<CloudStatus> {
    app.steamworks().cloud_status()
}

#[command]
pub(crate) async fn get_dlc<R: Runtime>(app: AppHandle<R>) -> Result<Vec<DlcInfo>> {
    app.steamworks().dlc_list()
}

#[command]
pub(crate) async fn is_dlc_installed<R: Runtime>(app: AppHandle<R>, app_id: u32) -> Result<bool> {
    app.steamworks().is_dlc_installed(app_id)
}

#[command]
pub(crate) async fn get_device_info<R: Runtime>(app: AppHandle<R>) -> Result<DeviceInfo> {
    app.steamworks().device_info()
}

#[command]
pub(crate) async fn hook_screenshots<R: Runtime>(app: AppHandle<R>, hook: bool) -> Result<()> {
    app.steamworks().hook_screenshots(hook)
}

#[command]
pub(crate) async fn trigger_screenshot<R: Runtime>(app: AppHandle<R>) -> Result<()> {
    app.steamworks().trigger_screenshot()
}

#[command]
pub(crate) async fn add_screenshot_to_library<R: Runtime>(
    app: AppHandle<R>,
    path: std::path::PathBuf,
    width: u32,
    height: u32,
) -> Result<()> {
    app.steamworks()
        .add_screenshot_to_library(&path, width, height)
}
