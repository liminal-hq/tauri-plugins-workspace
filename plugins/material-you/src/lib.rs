use tauri::{
    plugin::{Builder, TauriPlugin},
    Runtime,
};

pub use models::*;

#[cfg(target_os = "android")]
mod commands;
#[cfg(target_os = "android")]
mod mobile;

mod error;
mod models;

pub use error::{Error, Result};

#[cfg(target_os = "android")]
use mobile::MaterialYou;

/// Extensions to [`tauri::App`], [`tauri::AppHandle`] and [`tauri::Window`] to access the material-you APIs.
#[cfg(target_os = "android")]
pub trait MaterialYouExt<R: Runtime> {
    fn material_you(&self) -> &MaterialYou<R>;
}

#[cfg(target_os = "android")]
impl<R: Runtime, T: tauri::Manager<R>> MaterialYouExt<R> for T {
    fn material_you(&self) -> &MaterialYou<R> {
        self.state::<MaterialYou<R>>().inner()
    }
}

/// Initializes the plugin.
pub fn init<R: Runtime>() -> TauriPlugin<R> {
    let builder = Builder::new("material-you");

    #[cfg(target_os = "android")]
    let builder = builder
        .invoke_handler(tauri::generate_handler![commands::get_material_you_colours])
        .setup(|app, api| {
            let material_you = mobile::init(api, app)?;
            app.manage(material_you);
            Ok(())
        });

    #[cfg(not(target_os = "android"))]
    let builder = builder.setup(|_, _| Ok(()));

    builder.build()
}
