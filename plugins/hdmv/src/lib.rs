use tauri::{
    plugin::{Builder, TauriPlugin},
    Manager, Runtime,
};

mod commands;
mod error;
mod models;
mod session;

pub use error::{Error, Result};
pub use models::*;

/// Initialises the hdmv plugin.
pub fn init<R: Runtime>() -> TauriPlugin<R> {
    Builder::new("hdmv")
        .invoke_handler(tauri::generate_handler![
            commands::hdmv_open_disc,
            commands::hdmv_close_disc,
            commands::hdmv_get_disc_info,
            commands::hdmv_list_titles,
            commands::hdmv_list_playlists,
            commands::hdmv_get_playlist,
            commands::hdmv_start_navigation,
            commands::hdmv_send_key,
            commands::hdmv_mouse_move,
            commands::hdmv_mouse_click,
            commands::hdmv_render_preview,
            commands::hdmv_get_menu_state,
            commands::hdmv_build_disc,
        ])
        .setup(|app, _api| {
            app.manage(session::SessionStore::new());
            Ok(())
        })
        .build()
}
