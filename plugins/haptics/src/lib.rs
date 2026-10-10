// Registers the haptics plugin commands and its platform implementation
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

use tauri::{
    plugin::{Builder, TauriPlugin},
    AppHandle, Manager, Runtime,
};

mod commands;
mod config;
#[cfg(not(target_os = "android"))]
mod desktop;
mod error;
#[cfg(target_os = "android")]
mod mobile;
mod models;
#[cfg(not(target_os = "android"))]
mod validate;

pub use error::{Error, Result};

#[cfg(target_os = "android")]
use tauri::plugin::PluginHandle;

#[cfg(target_os = "android")]
const PLUGIN_IDENTIFIER: &str = "ca.liminalhq.haptics";

pub struct HapticsState<R: Runtime> {
    #[allow(dead_code)]
    app: AppHandle<R>,
    #[allow(dead_code)]
    config: config::Config,

    #[cfg(target_os = "android")]
    mobile: mobile::Haptics<R>,

    #[cfg(not(target_os = "android"))]
    desktop: desktop::Haptics,
}

impl<R: Runtime> HapticsState<R> {
    pub fn capabilities(&self) -> Result<models::Capabilities> {
        #[cfg(target_os = "android")]
        {
            return self.mobile.capabilities();
        }

        #[cfg(not(target_os = "android"))]
        {
            self.desktop.capabilities()
        }
    }

    pub fn play(&self, req: models::EffectRequest) -> Result<models::PlayResult> {
        // TODO: apply Rust-side validation/clamping too (belt & suspenders)
        #[cfg(target_os = "android")]
        {
            return self.mobile.play(req);
        }

        #[cfg(not(target_os = "android"))]
        {
            self.desktop.play(req)
        }
    }

    pub fn play_steps(&self, steps: Vec<models::CompiledStep>) -> Result<models::PlayResult> {
        #[cfg(target_os = "android")]
        {
            return self.mobile.play_steps(steps);
        }

        #[cfg(not(target_os = "android"))]
        {
            self.desktop.play_steps(steps)
        }
    }

    pub fn ui(&self, kind: models::UiKind) -> Result<models::PlayResult> {
        #[cfg(target_os = "android")]
        {
            return self.mobile.ui(kind);
        }

        #[cfg(not(target_os = "android"))]
        {
            self.desktop.ui(kind)
        }
    }

    pub fn stop(&self) -> Result<()> {
        #[cfg(target_os = "android")]
        {
            return self.mobile.stop();
        }

        #[cfg(not(target_os = "android"))]
        {
            self.desktop.stop()
        }
    }
}

pub trait HapticsExt<R: Runtime> {
    fn haptics(&self) -> &HapticsState<R>;
}

impl<R: Runtime, T: Manager<R>> HapticsExt<R> for T {
    fn haptics(&self) -> &HapticsState<R> {
        self.state::<HapticsState<R>>().inner()
    }
}

pub fn init<R: Runtime>() -> TauriPlugin<R, Option<config::Config>> {
    Builder::<R, Option<config::Config>>::new("haptics")
        .invoke_handler(tauri::generate_handler![
            commands::capabilities,
            commands::play,
            commands::play_steps,
            commands::ui,
            commands::stop,
        ])
        .setup(|app, api| {
            let default_config = config::Config::default();
            let config = api.config().as_ref().unwrap_or(&default_config).clone();

            #[cfg(target_os = "android")]
            let handle: PluginHandle<R> =
                api.register_android_plugin(PLUGIN_IDENTIFIER, "HapticsPlugin")?;

            app.manage(HapticsState {
                app: app.clone(),
                config: config.clone(),
                #[cfg(target_os = "android")]
                mobile: mobile::Haptics(handle),
                #[cfg(not(target_os = "android"))]
                desktop: desktop::Haptics::new(config),
            });

            Ok(())
        })
        .build()
}
