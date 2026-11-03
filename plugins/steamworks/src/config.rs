// Reads the plugin's tauri.conf.json settings
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

use std::time::Duration;

use serde::Deserialize;

pub const DEFAULT_CALLBACK_INTERVAL: Duration = Duration::from_millis(100);
const MIN_CALLBACK_INTERVAL: Duration = Duration::from_millis(10);
const MAX_CALLBACK_INTERVAL: Duration = Duration::from_millis(1000);

/// Settings under `plugins.steamworks` in `tauri.conf.json`. The AppId is not here because it
/// has to be known before the Tauri builder runs; pass it to [`crate::Builder::new`].
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Config {
    /// How often Steam callbacks are pumped, in milliseconds. Clamped to 10–1000.
    pub callback_interval_ms: Option<u64>,
    /// Runs the in-memory fake instead of Steam. Honoured only when the `mock` feature is compiled.
    pub mock: Option<bool>,
}

impl Config {
    pub fn callback_interval(&self) -> Option<Duration> {
        self.callback_interval_ms.map(Duration::from_millis)
    }

    pub fn wants_mock(&self) -> bool {
        self.mock.unwrap_or(false)
    }
}

/// Keeps a requested pump interval inside the supported range.
pub fn clamp_interval(interval: Duration) -> Duration {
    interval.clamp(MIN_CALLBACK_INTERVAL, MAX_CALLBACK_INTERVAL)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn clamps_to_supported_range() {
        assert_eq!(
            clamp_interval(Duration::from_millis(1)),
            MIN_CALLBACK_INTERVAL
        );
        assert_eq!(
            clamp_interval(Duration::from_secs(60)),
            MAX_CALLBACK_INTERVAL
        );
        assert_eq!(
            clamp_interval(Duration::from_millis(250)),
            Duration::from_millis(250)
        );
    }

    #[test]
    fn reads_camel_case_keys() {
        let config: Config =
            serde_json::from_str(r#"{"callbackIntervalMs":50,"mock":true}"#).unwrap();
        assert_eq!(config.callback_interval(), Some(Duration::from_millis(50)));
        assert!(config.wants_mock());
    }

    #[test]
    fn defaults_are_empty() {
        let config = Config::default();
        assert_eq!(config.callback_interval(), None);
        assert!(!config.wants_mock());
    }
}
