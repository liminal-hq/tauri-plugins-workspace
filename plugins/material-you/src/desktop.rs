use tauri::{plugin::PluginApi, AppHandle, Runtime};

use crate::models::MaterialYouResponse;

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
        // Material You is Android-only, so every other desktop platform reports unsupported.
        Ok(MaterialYouResponse::unsupported())
    }
}
