// Plugin configuration read from `tauri.conf.json`
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

use serde::{Deserialize, Serialize};

use crate::models::Limits;

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Config {
    /// The longest a request may run in total. Defaults to 3000.
    pub max_duration_ms: Option<u64>,
    /// The longest a motor may run without a silent frame. Longer runs are cut. Defaults to 2000.
    pub max_continuous_ms: Option<u64>,
    /// The master scale applied to every level, 0 to 1. Defaults to 1.
    pub master_scale: Option<f64>,
    /// Stops every pad when the window loses focus. Defaults to true.
    pub stop_on_blur: Option<bool>,
}

/// The longest limit that can be honoured: the kernel stores an effect's length in 15 bits, and a
/// pad may hold a level for a frame's time plus a little slack.
pub const MAX_DURATION_CAP_MS: u64 = 30_000;

impl Config {
    pub fn limits(&self) -> Limits {
        let max_duration_ms = self
            .max_duration_ms
            .unwrap_or(3_000)
            .clamp(1, MAX_DURATION_CAP_MS);
        Limits {
            max_duration_ms,
            max_continuous_ms: self
                .max_continuous_ms
                .unwrap_or(2_000)
                .clamp(1, max_duration_ms),
        }
    }

    pub fn master_scale(&self) -> f64 {
        match self.master_scale {
            Some(s) if s.is_finite() => s.clamp(0.0, 1.0),
            _ => 1.0,
        }
    }

    pub fn stop_on_blur(&self) -> bool {
        self.stop_on_blur.unwrap_or(true)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn defaults_and_floors() {
        let limits = Config::default().limits();
        assert_eq!(limits.max_duration_ms, 3_000);
        assert_eq!(limits.max_continuous_ms, 2_000);

        let config = Config {
            max_duration_ms: Some(0),
            max_continuous_ms: Some(9_000),
            master_scale: Some(7.0),
            stop_on_blur: None,
        };
        let limits = config.limits();
        assert_eq!(limits.max_duration_ms, 1);
        assert_eq!(limits.max_continuous_ms, 1);
        assert_eq!(config.master_scale(), 1.0);

        let long = Config {
            max_duration_ms: Some(120_000),
            ..Config::default()
        };
        assert_eq!(long.limits().max_duration_ms, MAX_DURATION_CAP_MS);
        assert!(config.stop_on_blur());
    }
}
