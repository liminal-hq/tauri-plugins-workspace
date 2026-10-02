// The Windows implementations of the desktop services
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

pub mod hotkeys;
pub mod power;
pub mod taskbar;
pub mod toast;

use crate::error::{ServiceError, ServiceErrorKind};

/// Wraps a Windows error with what was being done.
pub fn failed(what: &str, error: impl std::fmt::Display) -> ServiceError {
    ServiceError::new(ServiceErrorKind::Failed, format!("{what}: {error}"))
}
