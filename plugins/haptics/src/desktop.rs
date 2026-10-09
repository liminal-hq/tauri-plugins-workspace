// Desktop implementation, which has no vibrator and reports every request as downgraded
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

use crate::{config::Config, models::*, Result};

pub struct Haptics {
    config: Config,
}

impl Haptics {
    pub fn new(config: Config) -> Self {
        Self { config }
    }

    pub fn capabilities(&self) -> Result<Capabilities> {
        let limits = Limits {
            max_duration_ms: self.config.max_duration_ms.unwrap_or(10_000),
            max_amplitude: u16::from(self.config.max_amplitude.unwrap_or(255)),
            allow_repeating_waveforms: self.config.allow_repeating_waveforms.unwrap_or(false),
        };
        let device = DeviceInfo {
            manufacturer: String::new(),
            model: std::env::consts::OS.to_string(),
            release: String::new(),
        };
        Ok(Capabilities::none("desktop", limits, device))
    }

    pub fn play(&self, _req: EffectRequest) -> Result<PlayResult> {
        Ok(PlayResult::silent("No vibrator on this platform"))
    }

    pub fn play_steps(&self, _steps: Vec<CompiledStep>) -> Result<PlayResult> {
        Ok(PlayResult::silent("No vibrator on this platform"))
    }

    pub fn ui(&self, _kind: UiKind) -> Result<PlayResult> {
        Ok(PlayResult::silent("No vibrator on this platform"))
    }

    pub fn stop(&self) -> Result<()> {
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn haptics() -> Haptics {
        Haptics::new(Config::default())
    }

    #[test]
    fn play_resolves_at_tier_zero_with_a_reason() {
        let req: EffectRequest = serde_json::from_value(serde_json::json!({
            "effect": { "type": "predefined", "effectId": "click" }
        }))
        .expect("deserialize request");

        let res = haptics().play(req).expect("play resolves");
        assert_eq!(res.tier, 0);
        assert!(res.downgraded);
        assert_eq!(res.reason.as_deref(), Some("No vibrator on this platform"));
    }

    #[test]
    fn play_steps_and_ui_resolve_at_tier_zero() {
        let steps = haptics()
            .play_steps(Vec::new())
            .expect("play_steps resolves");
        assert_eq!(steps.tier, 0);

        let ui = haptics().ui(UiKind::Confirm).expect("ui resolves");
        assert_eq!(ui.tier, 0);
        assert_eq!(ui.reason.as_deref(), Some("No vibrator on this platform"));
    }

    #[test]
    fn capabilities_take_their_limits_from_the_config() {
        let caps = haptics().capabilities().expect("capabilities");
        assert_eq!(caps.platform, "desktop");
        assert_eq!(caps.limits.max_duration_ms, 10_000);
        assert!(!caps.limits.allow_repeating_waveforms);
    }
}
