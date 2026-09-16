use serde::de::DeserializeOwned;
#[cfg(target_os = "android")]
use tauri::plugin::PluginHandle;
use tauri::{plugin::PluginApi, AppHandle, Runtime};

use crate::models::MaterialYouResponse;

#[cfg(target_os = "android")]
const PLUGIN_IDENTIFIER: &str = "com.plugin.materialyou";

pub fn init<R: Runtime, C: DeserializeOwned>(
    _api: PluginApi<R, C>,
    _app: &AppHandle<R>,
) -> crate::Result<MaterialYou<R>> {
    #[cfg(target_os = "android")]
    {
        let handle = _api.register_android_plugin(PLUGIN_IDENTIFIER, "MaterialYouPlugin")?;
        Ok(MaterialYou { handle })
    }

    // Material You is an Android-only concept and no Swift plugin ships for it, so on iOS
    // nothing is registered: get_material_you_colours() below returns the same unsupported
    // stub as the desktop implementation.
    #[cfg(not(target_os = "android"))]
    {
        Ok(MaterialYou {
            _marker: std::marker::PhantomData,
        })
    }
}

/// Access to the material-you APIs.
pub struct MaterialYou<R: Runtime> {
    #[cfg(target_os = "android")]
    handle: PluginHandle<R>,
    #[cfg(not(target_os = "android"))]
    _marker: std::marker::PhantomData<R>,
}

impl<R: Runtime> MaterialYou<R> {
    pub fn get_material_you_colours(&self) -> crate::Result<MaterialYouResponse> {
        #[cfg(target_os = "android")]
        {
            self.handle
                .run_mobile_plugin("getMaterialYouColours", ())
                .map_err(Into::into)
        }

        #[cfg(not(target_os = "android"))]
        {
            Ok(MaterialYouResponse::unsupported())
        }
    }
}
