// Mobile implementation that forwards requests to the native plugin
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

use tauri::{plugin::PluginHandle, Runtime};

use crate::{models::*, Result};

pub struct Haptics<R: Runtime>(pub PluginHandle<R>);

#[derive(serde::Serialize)]
struct PlayStepsArgs {
    steps: Vec<CompiledStep>,
}

#[derive(serde::Serialize)]
struct UiArgs {
    kind: UiKind,
}

impl<R: Runtime> Haptics<R> {
    pub fn capabilities(&self) -> Result<Capabilities> {
        self.0
            .run_mobile_plugin("capabilities", ())
            .map_err(|e| crate::Error::MobilePluginInvoke(e.to_string()))
    }

    pub fn play(&self, req: EffectRequest) -> Result<PlayResult> {
        self.0
            .run_mobile_plugin("play", req)
            .map_err(|e| crate::Error::MobilePluginInvoke(e.to_string()))
    }

    pub fn play_steps(&self, steps: Vec<CompiledStep>) -> Result<PlayResult> {
        self.0
            .run_mobile_plugin("play_steps", PlayStepsArgs { steps })
            .map_err(|e| crate::Error::MobilePluginInvoke(e.to_string()))
    }

    pub fn ui(&self, kind: UiKind) -> Result<PlayResult> {
        self.0
            .run_mobile_plugin("ui", UiArgs { kind })
            .map_err(|e| crate::Error::MobilePluginInvoke(e.to_string()))
    }

    pub fn stop(&self) -> Result<()> {
        self.0
            .run_mobile_plugin("stop", ())
            .map(|_: serde_json::Value| ())
            .map_err(|e| crate::Error::MobilePluginInvoke(e.to_string()))
    }
}
