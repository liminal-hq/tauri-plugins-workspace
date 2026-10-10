// Error type shared by the haptics plugin
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

use thiserror::Error;

#[derive(Debug, Error)]
pub enum Error {
    #[error("plugin error: {0}")]
    Plugin(#[from] tauri::Error),
    #[error("mobile plugin invoke error: {0}")]
    MobilePluginInvoke(String),
    #[error("haptics unsupported on this platform")]
    Unsupported,
    #[error("invalid request: {0}")]
    InvalidRequest(String),
}

pub type Result<T> = std::result::Result<T, Error>;
