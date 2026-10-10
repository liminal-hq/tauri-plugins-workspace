// The seam between the portable core and each platform's way of driving motors
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

use std::sync::Arc;

use crate::{models::Transport, Result};

/// Motor levels, 0 to 1, for one instant.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct Levels {
    pub heavy: f64,
    pub light: f64,
    pub left_trigger: f64,
    pub right_trigger: f64,
}

/// A pad a backend found, before the registry gives it a slot.
#[derive(Debug, Clone, PartialEq)]
pub struct DiscoveredPad {
    /// Stable across reconnects and unique among the pads present, for example vendor, product and
    /// serial. The backend uses it to address the pad again.
    pub key: String,
    pub name: String,
    pub vendor_id: u16,
    pub product_id: u16,
    pub serial: Option<String>,
    pub transport: Transport,
    pub guid: String,
    pub motors: u8,
    pub triggers: bool,
    /// The light motor only switches on and off.
    pub light_binary: bool,
    pub top_tier: u8,
    /// Why the pad cannot play, when `top_tier` is 0.
    pub reason: Option<String>,
}

/// Called when the set of pads may have changed.
pub type Notify = Arc<dyn Fn() + Send + Sync>;

pub trait RumbleBackend: Send + Sync + 'static {
    /// Short name reported in `PadInfo::backend`, for example `evdev`.
    fn name(&self) -> &'static str;

    /// Every pad present now.
    fn scan(&self) -> Vec<DiscoveredPad>;

    /// Drives the motors at `levels`, replacing whatever was playing. The device must fall silent
    /// by itself once `hold_ms` has passed, so a stalled caller cannot leave a motor running.
    fn set(&self, key: &str, levels: Levels, hold_ms: u64) -> Result<()>;

    /// Stops every motor at once.
    fn silence(&self, key: &str) -> Result<()>;

    /// Called when a pad appears, to clear whatever it was doing before the plugin saw it: some
    /// pads keep their last rumble command after a cable is pulled. Failures are ignored.
    fn reset(&self, key: &str) {
        let _ = self.set(key, Levels::default(), 1);
        let _ = self.silence(key);
    }

    /// Calls `notify` when pads may have been added or removed. The default does nothing, so
    /// callers still rescan when they list pads.
    fn watch(&self, _notify: Notify) {}
}

/// A backend that finds no pads, for platforms without a native path.
pub struct NullBackend;

impl RumbleBackend for NullBackend {
    fn name(&self) -> &'static str {
        "none"
    }

    fn scan(&self) -> Vec<DiscoveredPad> {
        Vec::new()
    }

    fn set(&self, key: &str, _levels: Levels, _hold_ms: u64) -> Result<()> {
        Err(crate::Error::UnknownPad(key.to_string()))
    }

    fn silence(&self, _key: &str) -> Result<()> {
        Ok(())
    }
}
