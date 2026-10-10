// Registers the gamepad-haptics plugin commands
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

use std::sync::Arc;

use tauri::{
    plugin::{Builder, TauriPlugin},
    AppHandle, Emitter, Manager, RunEvent, Runtime, WindowEvent,
};

#[cfg(not(target_os = "linux"))]
use backend::NullBackend;
use backend::RumbleBackend;
use config::Config;

pub mod backend;
mod commands;
pub mod config;
#[cfg(test)]
mod conformance;
mod error;
mod haptics;
#[cfg(target_os = "linux")]
pub mod linux;
#[cfg(test)]
mod mock;
pub mod models;
pub mod normalise;
mod player;
#[cfg(test)]
mod props;
mod registry;
pub mod validate;

pub use error::{Error, Result};
pub use haptics::{Emit, GamepadHaptics};
pub use models::*;

/// Extensions to [`tauri::App`], [`tauri::AppHandle`] and [`tauri::Window`] to access the gamepad-haptics APIs.
pub trait GamepadHapticsExt<R: Runtime> {
    fn gamepad_haptics(&self) -> &GamepadHaptics;
}

impl<R: Runtime, T: Manager<R>> GamepadHapticsExt<R> for T {
    fn gamepad_haptics(&self) -> &GamepadHaptics {
        self.state::<GamepadHaptics>().inner()
    }
}

fn emit_pad_event<R: Runtime>(app: &AppHandle<R>, event: PadEvent) {
    let (name, payload) = match event {
        PadEvent::Connected(pad) => (CONNECTED_EVENT, serde_json::to_value(pad)),
        PadEvent::Changed(pad) => (CHANGED_EVENT, serde_json::to_value(pad)),
        PadEvent::Disconnected { id, slot } => (
            DISCONNECTED_EVENT,
            Ok(serde_json::json!({ "id": id, "slot": slot })),
        ),
    };
    match payload {
        Ok(payload) => {
            if let Err(e) = app.emit(name, payload) {
                log::warn!("gamepad-haptics: could not emit {name}: {e}");
            }
        }
        Err(e) => log::warn!("gamepad-haptics: could not encode {name}: {e}"),
    }
}

/// Initialises the plugin.
pub fn init<R: Runtime>() -> TauriPlugin<R, Option<Config>> {
    Builder::<R, Option<Config>>::new("gamepad-haptics")
        .invoke_handler(tauri::generate_handler![
            commands::capabilities,
            commands::identify,
            commands::list_pads,
            commands::play_frames,
            commands::stop
        ])
        .setup(|app, api| {
            let config = api.config().clone().unwrap_or_default();
            let handle = app.clone();
            let emit: haptics::Emit = Arc::new(move |event| emit_pad_event(&handle, event));
            app.manage(GamepadHaptics::new(platform_backend(), config, emit));
            Ok(())
        })
        .on_event(|app, event| match event {
            RunEvent::Exit => app.gamepad_haptics().stop_all(),
            RunEvent::WindowEvent {
                event: WindowEvent::Focused(false),
                ..
            } if app.gamepad_haptics().config().stop_on_blur() => app.gamepad_haptics().stop_all(),
            _ => {}
        })
        .build()
}

/// The backend for this platform. Platforms without a native path find no pads, and the guest
/// falls back to the webview's Gamepad API.
fn platform_backend() -> Arc<dyn RumbleBackend> {
    #[cfg(target_os = "linux")]
    return Arc::new(linux::EvdevBackend::new());
    #[cfg(not(target_os = "linux"))]
    Arc::new(NullBackend)
}
