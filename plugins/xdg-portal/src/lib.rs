// Registers the XDG portal plugin commands for Tauri
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

mod commands;
pub mod error;
pub mod global_shortcuts;
pub mod handles;
pub mod inhibit;
mod linux;
pub mod models;
pub mod notification;
pub mod open_uri;
mod service;
pub mod status;
pub mod timeout;

pub use service::{Portal, PortalExt};

use tauri::{
    plugin::{Builder, TauriPlugin},
    Manager, RunEvent, Runtime,
};

pub fn init<R: Runtime>() -> TauriPlugin<R> {
    Builder::new("xdg-portal")
        .invoke_handler(tauri::generate_handler![
            commands::check_availability,
            commands::get_theme_info,
            commands::get_status,
            commands::send_notification,
            commands::withdraw_notification,
            commands::inhibit,
            commands::release_inhibit,
            commands::open_uri,
        ])
        .setup(|app, _api| {
            app.manage(Portal::new(app.clone()));
            Ok(())
        })
        .on_event(|app, event| {
            if let RunEvent::Exit = event {
                // Inhibitors end with the process anyway; closing them first lets the shell stop
                // showing them at once.
                let portal = app.state::<Portal<R>>();
                tauri::async_runtime::block_on(async {
                    let _ = tokio::time::timeout(
                        std::time::Duration::from_secs(1),
                        portal.release_all(),
                    )
                    .await;
                });
            }
        })
        .build()
}
