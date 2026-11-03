// Defines plugin error types for steamworks command failures
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

use serde::{Serialize, Serializer};

use crate::models::UnavailableReason;

#[derive(Debug, thiserror::Error)]
pub enum Error {
    /// Steam cannot be used right now. The message prefix `unavailable:` is stable so guest code
    /// can recognise it.
    #[error("unavailable: {}", .0.as_str())]
    Unavailable(UnavailableReason),
    #[error("invalid request: {0}")]
    InvalidRequest(String),
    #[error("unknown achievement `{0}`")]
    UnknownAchievement(String),
    #[error("unknown stat `{0}`")]
    UnknownStat(String),
    #[error("steam: {0}")]
    Steam(String),
}

impl Serialize for Error {
    fn serialize<S>(&self, serializer: S) -> std::result::Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_str(self.to_string().as_ref())
    }
}

pub type Result<T> = std::result::Result<T, Error>;
