// The plugin's state: the pads it can address and what it can play on them
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

use crate::{models::*, Result};

const NO_BACKEND: &str = "No native gamepad backend on this platform";

#[derive(Default)]
pub struct GamepadHaptics;

impl GamepadHaptics {
    pub fn new() -> Self {
        Self
    }

    pub fn capabilities(&self) -> Result<Capabilities> {
        Ok(Capabilities {
            platform: std::env::consts::OS.to_string(),
            backend: "none".to_string(),
            limits: Limits {
                max_duration_ms: 3_000,
                max_continuous_ms: 2_000,
            },
            pads: self.list_pads()?,
        })
    }

    pub fn list_pads(&self) -> Result<Vec<PadInfo>> {
        Ok(Vec::new())
    }

    pub fn play_frames(&self, args: PlayFramesArgs) -> Result<PlayResult> {
        Ok(PlayResult::silent(args.pad_id, NO_BACKEND))
    }

    pub fn stop(&self, _pad_id: Option<&str>) -> Result<()> {
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_platform_without_a_backend_lists_no_pads_and_plays_silently() {
        let haptics = GamepadHaptics::new();
        assert!(haptics.list_pads().expect("list").is_empty());

        let res = haptics
            .play_frames(PlayFramesArgs {
                pad_id: "gamepad:0".into(),
                frames: Vec::new(),
                scale: None,
            })
            .expect("play resolves");
        assert_eq!(res.tier, 0);
        assert!(res.downgraded);
    }
}
