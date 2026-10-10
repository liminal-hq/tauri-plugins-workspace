// Registers the gamepad-haptics plugin commands
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

use tauri::{
    plugin::{Builder, TauriPlugin},
    Manager, Runtime,
};

mod commands;
pub mod config;
#[cfg(test)]
mod conformance;
mod error;
mod haptics;
pub mod models;
pub mod normalise;
#[cfg(test)]
mod props;
pub mod validate;

pub use error::{Error, Result};
pub use haptics::GamepadHaptics;
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

/// Initialises the plugin.
pub fn init<R: Runtime>() -> TauriPlugin<R> {
    Builder::new("gamepad-haptics")
        .invoke_handler(tauri::generate_handler![
            commands::capabilities,
            commands::list_pads,
            commands::play_frames,
            commands::stop
        ])
        .setup(|app, _api| {
            app.manage(GamepadHaptics::new());
            Ok(())
        })
        .build()
}
