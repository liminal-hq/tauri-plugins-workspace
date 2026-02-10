use tauri::{
  plugin::{Builder, TauriPlugin},
  Manager, Runtime,
};

pub use models::*;

#[cfg(not(target_os = "android"))]
mod desktop;
#[cfg(target_os = "android")]
mod mobile;

mod commands;
mod error;
mod models;

pub use error::{Error, Result};

#[cfg(not(target_os = "android"))]
use desktop::MaterialYou;
#[cfg(target_os = "android")]
use mobile::MaterialYou;

/// Extensions to [`tauri::App`], [`tauri::AppHandle`] and [`tauri::Window`] to access the material-you APIs.
pub trait MaterialYouExt<R: Runtime> {
  fn material_you(&self) -> &MaterialYou<R>;
}

impl<R: Runtime, T: Manager<R>> MaterialYouExt<R> for T {
  fn material_you(&self) -> &MaterialYou<R> {
    self.state::<MaterialYou<R>>().inner()
  }
}

/// Initializes the plugin.
pub fn init<R: Runtime>() -> TauriPlugin<R> {
  Builder::new("material-you")
    .invoke_handler(tauri::generate_handler![commands::get_material_you_colours])
    .setup(|app, api| {
      #[cfg(target_os = "android")]
      let material_you = mobile::init(api, app)?;
      #[cfg(not(target_os = "android"))]
      let material_you = desktop::init(api, app)?;
      app.manage(material_you);
      Ok(())
    })
    .build()
}
