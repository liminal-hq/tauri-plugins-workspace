// Registers the steamworks plugin, its builder and its commands
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

use std::{sync::Arc, time::Duration};

use tauri::{
    plugin::{Builder as PluginBuilder, TauriPlugin},
    AppHandle, Emitter, Manager, RunEvent, Runtime,
};

use config::Config;
use steam::StartOptions;

pub mod backend;
mod commands;
pub mod config;
mod error;
pub mod input;
pub mod lobby;
#[cfg(any(test, feature = "mock"))]
pub mod mock;
pub mod models;
mod pump;
mod real;
mod steam;
pub mod workshop;

pub use backend::{Emit, SteamworksBackend};
pub use error::{Error, Result};
pub use input::*;
pub use lobby::*;
#[cfg(any(test, feature = "mock"))]
pub use mock::{MockBackend, MockConfig};
pub use models::*;
pub use steam::{preinit, PreInit, Steamworks};
/// The underlying client crate, re-exported so an app and this plugin cannot end up on different
/// versions.
#[cfg(steam_sdk)]
pub use steamworks;
pub use workshop::*;

/// Extensions to [`tauri::App`], [`tauri::AppHandle`] and [`tauri::Window`] to access the steamworks APIs.
pub trait SteamworksExt<R: Runtime> {
    fn steamworks(&self) -> &Steamworks;
}

impl<R: Runtime, T: Manager<R>> SteamworksExt<R> for T {
    fn steamworks(&self) -> &Steamworks {
        self.state::<Steamworks>().inner()
    }
}

/// Configures and builds the plugin.
///
/// ```ignore
/// tauri::Builder::default().plugin(tauri_plugin_steamworks::Builder::new(480).build())
/// ```
pub struct Builder {
    app_id: u32,
    restart_if_necessary: bool,
    interval: Option<Duration>,
    preinit: Option<PreInit>,
    #[cfg(any(test, feature = "mock"))]
    mock: Option<MockConfig>,
}

impl Builder {
    pub fn new(app_id: u32) -> Self {
        Self {
            app_id,
            restart_if_necessary: false,
            interval: None,
            preinit: None,
            #[cfg(any(test, feature = "mock"))]
            mock: None,
        }
    }

    /// Uses Steam initialised earlier by [`preinit`].
    pub fn from_preinit(preinit: PreInit) -> Self {
        let mut builder = Self::new(preinit.app_id);
        builder.preinit = Some(preinit);
        builder
    }

    /// Relaunches the app through Steam and exits when it was started outside Steam. Off by default
    /// because exiting the app on its behalf is surprising. Ignored with [`Builder::from_preinit`],
    /// which takes the choice made in [`preinit`].
    pub fn restart_if_necessary(mut self, restart: bool) -> Self {
        self.restart_if_necessary = restart;
        self
    }

    /// How often Steam callbacks are pumped. Clamped to 10–1000 ms; the default is 100 ms.
    pub fn callback_interval(mut self, interval: Duration) -> Self {
        self.interval = Some(interval);
        self
    }

    /// Uses the in-memory fake instead of Steam.
    #[cfg(any(test, feature = "mock"))]
    pub fn mock(mut self, config: MockConfig) -> Self {
        self.mock = Some(config);
        self
    }

    pub fn build<R: Runtime>(self) -> TauriPlugin<R, Option<Config>> {
        PluginBuilder::<R, Option<Config>>::new("steamworks")
            .invoke_handler(tauri::generate_handler![
                commands::is_available,
                commands::get_status,
                commands::get_player,
                commands::get_achievement,
                commands::unlock_achievement,
                commands::clear_achievement,
                commands::indicate_achievement_progress,
                commands::get_stat,
                commands::set_stat,
                commands::store_stats,
                commands::set_rich_presence,
                commands::clear_rich_presence,
                commands::activate_overlay,
                commands::activate_overlay_to_web_page,
                commands::cloud_write,
                commands::cloud_read,
                commands::cloud_delete,
                commands::cloud_list,
                commands::cloud_quota,
                commands::cloud_status,
                commands::get_dlc,
                commands::is_dlc_installed,
                commands::get_device_info,
                commands::hook_screenshots,
                commands::trigger_screenshot,
                commands::add_screenshot_to_library,
                commands::social::create_lobby,
                commands::social::join_lobby,
                commands::social::list_lobbies,
                commands::social::leave_lobby,
                commands::social::get_lobby,
                commands::social::set_lobby_data,
                commands::social::set_lobby_joinable,
                commands::social::get_workshop_items,
                commands::social::subscribe_workshop_item,
                commands::social::unsubscribe_workshop_item,
                commands::social::download_workshop_item,
                commands::social::input_init,
                commands::social::get_input_controllers,
                commands::social::get_input_action_set,
                commands::social::activate_input_action_set,
                commands::social::get_input_action,
                commands::social::get_digital_action_state,
                commands::social::get_analog_action_state,
                commands::social::get_input_glyph
            ])
            .setup(move |app, api| {
                let config = api.config().clone().unwrap_or_default();
                let handle = app.clone();
                let emit: Emit = Arc::new(move |event| emit_event(&handle, event));
                let options = StartOptions {
                    app_id: self.app_id,
                    restart_if_necessary: self.restart_if_necessary,
                    interval: self.interval,
                    preinit: self.preinit,
                    #[cfg(any(test, feature = "mock"))]
                    mock: self.mock,
                };
                let steamworks = Steamworks::start(options, &config, emit);
                let restart = matches!(
                    steamworks.status(),
                    SteamStatus::Unavailable {
                        reason: UnavailableReason::RestartRequired,
                        ..
                    }
                );
                app.manage(steamworks);
                if restart {
                    app.app_handle().exit(0);
                }
                Ok(())
            })
            .on_event(|app, event| {
                if let RunEvent::Exit = event {
                    app.steamworks().shutdown();
                }
            })
            .build()
    }
}

fn emit_event<R: Runtime>(app: &AppHandle<R>, event: SteamEvent) {
    let (name, payload) = match event {
        SteamEvent::StatsReceived { success, steam_id } => (
            STATS_RECEIVED_EVENT,
            serde_json::json!({ "success": success, "steamId": steam_id }),
        ),
        SteamEvent::StatsStored { success } => (
            STATS_STORED_EVENT,
            serde_json::json!({ "success": success }),
        ),
        SteamEvent::AchievementStored { id, current, max } => (
            ACHIEVEMENT_STORED_EVENT,
            serde_json::json!({ "id": id, "current": current, "max": max }),
        ),
        SteamEvent::OverlayToggled { active } => (
            OVERLAY_TOGGLED_EVENT,
            serde_json::json!({ "active": active }),
        ),
        SteamEvent::ScreenshotRequested => (SCREENSHOT_REQUESTED_EVENT, serde_json::json!({})),
        SteamEvent::LobbyChatUpdate {
            lobby_id,
            user_changed,
            making_change,
            change,
        } => (
            LOBBY_CHAT_UPDATE_EVENT,
            serde_json::json!({
                "lobbyId": lobby_id,
                "userChanged": user_changed,
                "makingChange": making_change,
                "change": change,
            }),
        ),
        SteamEvent::LobbyDataUpdate {
            lobby_id,
            member_id,
            success,
        } => (
            LOBBY_DATA_UPDATE_EVENT,
            serde_json::json!({ "lobbyId": lobby_id, "memberId": member_id, "success": success }),
        ),
    };
    if let Err(e) = app.emit(name, payload) {
        log::warn!("steamworks: could not emit {name}: {e}");
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Mutex;
    use tauri::Listener;

    fn payload_for(event: SteamEvent, name: &str) -> serde_json::Value {
        let app = tauri::test::mock_app();
        let seen = Arc::new(Mutex::new(None));
        let sink = seen.clone();
        app.listen(name, move |e| {
            *sink.lock().unwrap() = Some(e.payload().to_string())
        });
        emit_event(app.handle(), event);
        let payload = seen
            .lock()
            .unwrap()
            .take()
            .expect("the event should be emitted");
        serde_json::from_str(&payload).unwrap()
    }

    #[test]
    fn lobby_chat_updates_carry_string_ids_and_a_camel_case_change() {
        let payload = payload_for(
            SteamEvent::LobbyChatUpdate {
                lobby_id: "109775240000000001".into(),
                user_changed: "76561198000000001".into(),
                making_change: "76561198000000002".into(),
                change: LobbyMemberChange::Kicked,
            },
            LOBBY_CHAT_UPDATE_EVENT,
        );
        assert_eq!(
            payload,
            serde_json::json!({
                "lobbyId": "109775240000000001",
                "userChanged": "76561198000000001",
                "makingChange": "76561198000000002",
                "change": "kicked",
            })
        );
    }

    #[test]
    fn lobby_data_updates_name_the_lobby_and_the_member() {
        let payload = payload_for(
            SteamEvent::LobbyDataUpdate {
                lobby_id: "5".into(),
                member_id: "5".into(),
                success: true,
            },
            LOBBY_DATA_UPDATE_EVENT,
        );
        assert_eq!(
            payload,
            serde_json::json!({"lobbyId": "5", "memberId": "5", "success": true})
        );
    }
}
