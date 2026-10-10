// Plugin configuration read from `tauri.conf.json`
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

use serde::{Deserialize, Serialize};

use crate::models::Limits;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Config {
    pub default_usage: Option<String>,
    /// Respect the system touch-feedback setting for `touch` usage. Other usages ignore it unless
    /// a request sets `respectSystemSettings`.
    pub respect_system_haptics_setting: Option<bool>,
    pub stop_before_play: Option<bool>,
    pub max_duration_ms: Option<u64>,
    pub max_amplitude: Option<u8>,
    pub allow_repeating_waveforms: Option<bool>,

    #[serde(default)]
    pub android: AndroidConfig,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AndroidConfig {
    pub foreground_audio_usage: Option<String>,
    pub background_audio_usage: Option<String>,
}

impl Config {
    /// The limits requests are held to, with the same floors the Android plugin applies.
    pub fn limits(&self) -> Limits {
        Limits {
            max_duration_ms: self.max_duration_ms.unwrap_or(10_000).max(1),
            max_amplitude: u16::from(self.max_amplitude.unwrap_or(255).clamp(1, 255)),
            allow_repeating_waveforms: self.allow_repeating_waveforms.unwrap_or(false),
        }
    }
}

impl Default for Config {
    fn default() -> Self {
        Self {
            default_usage: Some("touch".into()),
            respect_system_haptics_setting: Some(true),
            stop_before_play: Some(true),
            max_duration_ms: Some(10_000),
            max_amplitude: Some(255),
            allow_repeating_waveforms: Some(false),
            android: AndroidConfig {
                foreground_audio_usage: Some("USAGE_ASSISTANCE_SONIFICATION".into()),
                background_audio_usage: Some("USAGE_ALARM".into()),
            },
        }
    }
}
