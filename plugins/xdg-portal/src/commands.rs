// Implements IPC commands exposed by the XDG portal plugin
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

use tauri::{AppHandle, Runtime};

use crate::{
    error::{PortalError, ServiceError},
    linux,
    models::{
        AvailabilityInfo, InhibitHandle, InhibitRequest, NotificationRequest, OpenUriRequest,
        PortalStatus, ThemeInfo,
    },
    service::PortalExt,
};

#[tauri::command]
pub async fn check_availability() -> Result<AvailabilityInfo, PortalError> {
    linux::check_availability().await
}

#[tauri::command]
pub async fn get_theme_info() -> Result<ThemeInfo, PortalError> {
    linux::get_theme_info().await
}

#[tauri::command]
pub async fn get_status<R: Runtime>(app: AppHandle<R>) -> PortalStatus {
    app.portal().status().await
}

#[tauri::command]
pub async fn send_notification<R: Runtime>(
    app: AppHandle<R>,
    request: NotificationRequest,
) -> Result<(), ServiceError> {
    app.portal().send_notification(request).await
}

#[tauri::command]
pub async fn withdraw_notification<R: Runtime>(
    app: AppHandle<R>,
    id: String,
) -> Result<(), ServiceError> {
    app.portal().withdraw_notification(id).await
}

#[tauri::command]
pub async fn inhibit<R: Runtime>(
    app: AppHandle<R>,
    request: InhibitRequest,
) -> Result<InhibitHandle, ServiceError> {
    app.portal().inhibit(request).await
}

#[tauri::command]
pub async fn release_inhibit<R: Runtime>(
    app: AppHandle<R>,
    handle: u32,
) -> Result<(), ServiceError> {
    app.portal().release_inhibit(handle).await
}

#[tauri::command]
pub async fn open_uri<R: Runtime>(
    app: AppHandle<R>,
    request: OpenUriRequest,
) -> Result<(), ServiceError> {
    app.portal().open_uri(request).await
}
