use tauri::{plugin::PluginApi, AppHandle, Runtime};

use crate::models::{MaterialYouResponse, Palettes};

pub fn init<R: Runtime>(
    api: PluginApi<R, ()>,
    _app: &AppHandle<R>,
) -> crate::Result<MaterialYou<R>> {
    Ok(MaterialYou(api))
}

/// Access to the material-you APIs.
pub struct MaterialYou<R: Runtime>(PluginApi<R, ()>);

impl<R: Runtime> MaterialYou<R> {
    pub fn get_material_you_colours(&self) -> crate::Result<MaterialYouResponse> {
        // Desktop does not support Material You
        println!("[MaterialYou] Desktop platform detected, returning empty Material You response.");
        Ok(MaterialYouResponse {
            supported: false,
            api_level: 0,
            palettes: Palettes {
                system_accent1: None,
                system_accent2: None,
                system_accent3: None,
                system_neutral1: None,
                system_neutral2: None,
            },
        })
    }
}
