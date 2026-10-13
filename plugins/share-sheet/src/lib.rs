// Registers the share-sheet plugin, which hands a file or text to the operating system's share UI
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

use tauri::{
    plugin::{Builder, TauriPlugin},
    Manager, Runtime,
};

mod commands;
mod error;
pub mod models;
mod share;
pub mod validate;

#[cfg_attr(not(mobile), allow(dead_code))]
mod native;

#[cfg(target_os = "android")]
mod android;
#[cfg(target_os = "android")]
use android as platform;

#[cfg(target_os = "ios")]
mod ios;
#[cfg(target_os = "ios")]
use ios as platform;

#[cfg(target_os = "macos")]
mod macos;
#[cfg(target_os = "macos")]
use macos as platform;

#[cfg(not(any(target_os = "android", target_os = "ios", target_os = "macos")))]
mod unsupported;
#[cfg(not(any(target_os = "android", target_os = "ios", target_os = "macos")))]
use unsupported as platform;

pub use error::{Error, ErrorCode, Result};
pub use models::*;
pub use share::ShareSheet;

/// Extensions to [`tauri::App`], [`tauri::AppHandle`] and [`tauri::Window`] to reach the share sheet.
pub trait ShareSheetExt<R: Runtime> {
    fn share_sheet(&self) -> &ShareSheet<R>;
}

impl<R: Runtime, T: Manager<R>> ShareSheetExt<R> for T {
    fn share_sheet(&self) -> &ShareSheet<R> {
        self.state::<ShareSheet<R>>().inner()
    }
}

/// Initialises the plugin.
pub fn init<R: Runtime>() -> TauriPlugin<R> {
    Builder::new("share-sheet")
        .invoke_handler(tauri::generate_handler![
            commands::share_file,
            commands::share_text,
            commands::can_share,
        ])
        .setup(|app, api| {
            let backend = platform::init(app, api)?;
            app.manage(ShareSheet::new(backend));
            Ok(())
        })
        .build()
}
