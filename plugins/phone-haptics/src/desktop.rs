// Desktop implementation, which has no vibrator and reports every request as downgraded
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

use crate::{config::Config, models::*, normalise::Normalised, Result};

const NO_VIBRATOR: &str = "No vibrator on this platform";

pub struct Haptics {
    config: Config,
}

impl Haptics {
    pub fn new(config: Config) -> Self {
        Self { config }
    }

    pub fn capabilities(&self) -> Result<Capabilities> {
        let device = DeviceInfo {
            manufacturer: String::new(),
            model: std::env::consts::OS.to_string(),
            release: String::new(),
        };
        let platform = if cfg!(target_os = "ios") {
            "ios"
        } else {
            "desktop"
        };
        Ok(Capabilities::none(platform, self.config.limits(), device))
    }

    pub fn play(&self, _args: &Normalised<PlayArgs>) -> Result<PlayResult> {
        Ok(PlayResult::silent(NO_VIBRATOR))
    }

    pub fn play_steps(&self, _args: &Normalised<PlayStepsArgs>) -> Result<PlayResult> {
        Ok(PlayResult::silent(NO_VIBRATOR))
    }

    pub fn ui(&self, _kind: UiKind) -> Result<PlayResult> {
        Ok(PlayResult::silent(NO_VIBRATOR))
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

    fn click() -> EffectRequest {
        serde_json::from_value(serde_json::json!({
            "effect": { "type": "predefined", "effectId": "click" }
        }))
        .expect("deserialize request")
    }

    #[test]
    fn play_resolves_at_tier_zero_with_a_reason() {
        let args = Normalised::unchecked(PlayArgs {
            req: click(),
            budget_ms: 10_000,
        });

        let res = haptics().play(&args).expect("play resolves");
        assert_eq!(res.tier, 0);
        assert!(res.downgraded);
        assert_eq!(res.reason.as_deref(), Some(NO_VIBRATOR));
    }

    #[test]
    fn play_steps_and_ui_resolve_at_tier_zero() {
        let args = Normalised::unchecked(PlayStepsArgs {
            steps: vec![PlannedStep {
                at_ms: 0,
                budget_ms: 10_000,
                request: click(),
            }],
        });
        let steps = haptics().play_steps(&args).expect("play_steps resolves");
        assert_eq!(steps.tier, 0);

        let ui = haptics().ui(UiKind::Confirm).expect("ui resolves");
        assert_eq!(ui.tier, 0);
        assert_eq!(ui.reason.as_deref(), Some(NO_VIBRATOR));
    }

    #[test]
    fn capabilities_take_their_limits_from_the_config() {
        let caps = haptics().capabilities().expect("capabilities");
        assert_eq!(
            caps.platform,
            if cfg!(target_os = "ios") {
                "ios"
            } else {
                "desktop"
            }
        );
        assert_eq!(caps.limits.max_duration_ms, 10_000);
        assert!(!caps.limits.allow_repeating_waveforms);
    }
}
