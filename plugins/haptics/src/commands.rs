// Tauri command handlers that forward haptics requests to the platform implementation
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

use tauri::{command, AppHandle, Runtime};

use crate::{models::*, HapticsExt};

#[command]
pub fn capabilities<R: Runtime>(app: AppHandle<R>) -> std::result::Result<Capabilities, String> {
    app.haptics().capabilities().map_err(|e| e.to_string())
}

#[command]
pub fn play<R: Runtime>(
    app: AppHandle<R>,
    req: EffectRequest,
) -> std::result::Result<PlayResult, String> {
    app.haptics().play(req).map_err(|e| e.to_string())
}

#[command]
pub fn play_steps<R: Runtime>(
    app: AppHandle<R>,
    steps: Vec<CompiledStep>,
) -> std::result::Result<PlayResult, String> {
    app.haptics().play_steps(steps).map_err(|e| e.to_string())
}

#[command]
pub fn ui<R: Runtime>(app: AppHandle<R>, kind: UiKind) -> std::result::Result<PlayResult, String> {
    app.haptics().ui(kind).map_err(|e| e.to_string())
}

#[command]
pub fn stop<R: Runtime>(app: AppHandle<R>) -> std::result::Result<(), String> {
    app.haptics().stop().map_err(|e| e.to_string())
}
