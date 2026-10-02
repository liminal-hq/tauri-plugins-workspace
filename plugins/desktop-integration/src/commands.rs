// Implements the IPC commands of the notify, sleep, launcher, file manager and shortcut services
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

use tauri::{AppHandle, Runtime};

use crate::{
    error::ServiceError,
    models::{
        FileManagerOwnership, GlobalShortcutRequest, LauncherRequest, NotifyRequest, PluginStatus,
        SleepInhibitHandle, SleepInhibitRequest,
    },
    service::DesktopServicesExt,
};

#[tauri::command]
pub async fn get_status<R: Runtime>(app: AppHandle<R>) -> PluginStatus {
    app.desktop_services().status().await
}

#[tauri::command]
pub async fn notify<R: Runtime>(
    app: AppHandle<R>,
    request: NotifyRequest,
) -> Result<(), ServiceError> {
    app.desktop_services().notify(request).await
}

#[tauri::command]
pub async fn withdraw_notification<R: Runtime>(
    app: AppHandle<R>,
    id: String,
) -> Result<(), ServiceError> {
    app.desktop_services().withdraw_notification(id).await
}

#[tauri::command]
pub async fn inhibit_sleep<R: Runtime>(
    app: AppHandle<R>,
    request: SleepInhibitRequest,
) -> Result<SleepInhibitHandle, ServiceError> {
    app.desktop_services().inhibit_sleep(request).await
}

#[tauri::command]
pub async fn release_sleep_inhibit<R: Runtime>(
    app: AppHandle<R>,
    handle: u32,
) -> Result<(), ServiceError> {
    app.desktop_services().release_sleep_inhibit(handle).await
}

#[tauri::command]
pub async fn set_launcher_progress<R: Runtime>(
    app: AppHandle<R>,
    request: LauncherRequest,
) -> Result<(), ServiceError> {
    app.desktop_services().set_launcher_progress(request).await
}

#[tauri::command]
pub async fn own_file_manager<R: Runtime>(
    app: AppHandle<R>,
) -> Result<FileManagerOwnership, ServiceError> {
    app.desktop_services().own_file_manager().await
}

#[tauri::command]
pub async fn disown_file_manager<R: Runtime>(
    app: AppHandle<R>,
) -> Result<FileManagerOwnership, ServiceError> {
    app.desktop_services().disown_file_manager().await
}

#[tauri::command]
pub async fn register_global_shortcut<R: Runtime>(
    app: AppHandle<R>,
    request: GlobalShortcutRequest,
) -> Result<(), ServiceError> {
    app.desktop_services()
        .register_global_shortcut(request)
        .await
}

#[tauri::command]
pub async fn unregister_global_shortcut<R: Runtime>(
    app: AppHandle<R>,
    id: String,
) -> Result<(), ServiceError> {
    app.desktop_services().unregister_global_shortcut(id).await
}

#[tauri::command]
pub fn set_app_user_model_id<R: Runtime>(
    app: AppHandle<R>,
    id: String,
) -> Result<(), ServiceError> {
    app.desktop_services().set_app_user_model_id(&id)
}
