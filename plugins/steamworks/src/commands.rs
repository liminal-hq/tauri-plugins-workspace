// Tauri command handlers exposed to the webview
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

use tauri::{command, AppHandle, Runtime};

use crate::{
    error::Result,
    models::{Achievement, Player, StatKind, StatValue, SteamStatus},
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
