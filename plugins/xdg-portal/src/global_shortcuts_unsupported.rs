// Reports the GlobalShortcuts portal as unsupported on platforms without xdg-desktop-portal
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

use crate::error::PortalError;

/// Stands in for the portal window identifier, which only exists on Linux.
#[derive(Debug, Clone)]
pub struct WindowIdentifier;

/// Dropping this handle would cancel a shortcut listener; none is ever created here.
pub struct ShortcutHandle;

/// Always fails with [`PortalError::UnsupportedPlatform`]: the same signature as the Linux
/// implementation, so code that is written once compiles everywhere.
pub async fn create_session<F, B, C>(
    _shortcut_id: &str,
    _description: &str,
    _preferred_trigger: Option<&str>,
    _on_activated: F,
    _on_binding_result: B,
    _on_shortcuts_changed: C,
    _window_rx: tokio::sync::oneshot::Receiver<Option<WindowIdentifier>>,
) -> Result<ShortcutHandle, PortalError>
where
    F: Fn() + Send + Sync + 'static,
    B: Fn(Result<(), String>) + Send + 'static,
    C: Fn(String) + Send + 'static,
{
    Err(PortalError::UnsupportedPlatform)
}
