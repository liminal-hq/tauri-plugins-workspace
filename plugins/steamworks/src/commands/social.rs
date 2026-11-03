// Tauri command handlers for lobbies, Workshop and Steam Input
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

use tauri::{command, ipc::Response, AppHandle, Runtime};

use crate::{
    error::Result,
    input::{ActionKind, AnalogActionState, DigitalActionState, GlyphSize, InputController},
    lobby::{LobbyInfo, LobbyKind},
    workshop::WorkshopItem,
    SteamworksExt,
};

#[command]
pub(crate) async fn create_lobby<R: Runtime>(
    app: AppHandle<R>,
    kind: LobbyKind,
    max_members: u32,
) -> Result<String> {
    app.steamworks().create_lobby(kind, max_members).await
}

#[command]
pub(crate) async fn join_lobby<R: Runtime>(app: AppHandle<R>, id: String) -> Result<String> {
    app.steamworks().join_lobby(&id).await
}

#[command]
pub(crate) async fn list_lobbies<R: Runtime>(app: AppHandle<R>) -> Result<Vec<LobbyInfo>> {
    app.steamworks().list_lobbies().await
}

#[command]
pub(crate) async fn leave_lobby<R: Runtime>(app: AppHandle<R>, id: String) -> Result<()> {
    app.steamworks().leave_lobby(&id)
}

#[command]
pub(crate) async fn get_lobby<R: Runtime>(app: AppHandle<R>, id: String) -> Result<LobbyInfo> {
    app.steamworks().lobby_info(&id)
}

/// Sets a lobby data key; a `null` value removes it. Only the lobby owner may change data.
#[command]
pub(crate) async fn set_lobby_data<R: Runtime>(
    app: AppHandle<R>,
    id: String,
    key: String,
    value: Option<String>,
) -> Result<()> {
    app.steamworks().set_lobby_data(&id, &key, value.as_deref())
}

#[command]
pub(crate) async fn set_lobby_joinable<R: Runtime>(
    app: AppHandle<R>,
    id: String,
    joinable: bool,
) -> Result<()> {
    app.steamworks().set_lobby_joinable(&id, joinable)
}

#[command]
pub(crate) async fn get_workshop_items<R: Runtime>(app: AppHandle<R>) -> Result<Vec<WorkshopItem>> {
    app.steamworks().workshop_items()
}

#[command]
pub(crate) async fn subscribe_workshop_item<R: Runtime>(
    app: AppHandle<R>,
    id: String,
) -> Result<()> {
    app.steamworks().subscribe_item(&id).await
}

#[command]
pub(crate) async fn unsubscribe_workshop_item<R: Runtime>(
    app: AppHandle<R>,
    id: String,
) -> Result<()> {
    app.steamworks().unsubscribe_item(&id).await
}

#[command]
pub(crate) async fn download_workshop_item<R: Runtime>(
    app: AppHandle<R>,
    id: String,
    high_priority: Option<bool>,
) -> Result<bool> {
    app.steamworks()
        .download_item(&id, high_priority.unwrap_or(false))
}

#[command]
pub(crate) async fn input_init<R: Runtime>(app: AppHandle<R>) -> Result<()> {
    app.steamworks().input_init()
}

#[command]
pub(crate) async fn get_input_controllers<R: Runtime>(
    app: AppHandle<R>,
) -> Result<Vec<InputController>> {
    app.steamworks().input_controllers()
}

#[command]
pub(crate) async fn get_input_action_set<R: Runtime>(
    app: AppHandle<R>,
    name: String,
) -> Result<String> {
    app.steamworks().input_action_set(&name)
}

#[command]
pub(crate) async fn activate_input_action_set<R: Runtime>(
    app: AppHandle<R>,
    controller: String,
    action_set: String,
) -> Result<()> {
    app.steamworks()
        .input_activate_action_set(&controller, &action_set)
}

#[command]
pub(crate) async fn get_input_action<R: Runtime>(
    app: AppHandle<R>,
    kind: ActionKind,
    name: String,
) -> Result<String> {
    app.steamworks().input_action(kind, &name)
}

#[command]
pub(crate) async fn get_digital_action_state<R: Runtime>(
    app: AppHandle<R>,
    controller: String,
    action: String,
) -> Result<DigitalActionState> {
    app.steamworks().input_digital_state(&controller, &action)
}

#[command]
pub(crate) async fn get_analog_action_state<R: Runtime>(
    app: AppHandle<R>,
    controller: String,
    action: String,
) -> Result<AnalogActionState> {
    app.steamworks().input_analog_state(&controller, &action)
}

/// Returns the PNG of the first button bound to an action, as the raw response body.
#[command]
pub(crate) async fn get_input_glyph<R: Runtime>(
    app: AppHandle<R>,
    controller: String,
    action_set: String,
    kind: ActionKind,
    action: String,
    size: Option<GlyphSize>,
) -> Result<Response> {
    app.steamworks()
        .input_glyph(
            &controller,
            &action_set,
            kind,
            &action,
            size.unwrap_or(GlyphSize::Medium),
        )
        .map(Response::new)
}
