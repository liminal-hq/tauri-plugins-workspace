// Wire types shared by the commands, the backends and the guest API
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

use serde::{Deserialize, Serialize};

/// How a pad is attached to the machine.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Transport {
    Usb,
    Bluetooth,
    Unknown,
}

/// A pad the plugin can address.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PadInfo {
    /// `gamepad:N`, where N is the pad's slot.
    pub id: String,
    pub slot: u32,
    pub name: String,
    pub vendor_id: u16,
    pub product_id: u16,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub serial: Option<String>,
    pub transport: Transport,
    /// SDL-style GUID, which gilrs and SDL report for the same device.
    pub guid: String,
    /// Rumble motors in the body: 0, 1 or 2.
    pub motors: u8,
    /// Whether the pad has trigger motors.
    pub triggers: bool,
    /// Whether the light motor only switches on and off. The plugin then pulses it to approximate
    /// in-between strengths.
    #[serde(default)]
    pub light_binary: bool,
    /// Whether the heavy motor cannot spin up for a short soft tap. The plugin then lengthens such
    /// taps into the silence after them.
    #[serde(default)]
    pub weak_heavy: bool,
    /// The highest tier this pad can play: 0 none, 1 single motor, 2 dual motor, 3 triggers.
    pub top_tier: u8,
    /// Why the pad cannot play, when `top_tier` is 0.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reason: Option<String>,
    /// The backend that drives the pad, for example `evdev`.
    pub backend: String,
}

/// Motor levels held for a duration. Levels run from 0 to 1.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Frame {
    pub duration_ms: u64,
    /// The heavy, low-frequency motor.
    pub heavy: f64,
    /// The light, high-frequency motor.
    pub light: f64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub left_trigger: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub right_trigger: Option<f64>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PlayFramesArgs {
    pub pad_id: String,
    pub frames: Vec<Frame>,
    /// Scale for this call, 0 to 1. It multiplies with the configured master scale.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub scale: Option<f64>,
}

/// What a play call did. A pad that cannot play resolves at tier 0 rather than failing.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PlayResult {
    pub ok: bool,
    pub tier: u8,
    pub target: String,
    pub downgraded: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reason: Option<String>,
}

impl PlayResult {
    pub fn silent(target: impl Into<String>, reason: impl Into<String>) -> Self {
        Self {
            ok: true,
            tier: 0,
            target: target.into(),
            downgraded: true,
            reason: Some(reason.into()),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Limits {
    pub max_duration_ms: u64,
    pub max_continuous_ms: u64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Capabilities {
    pub platform: String,
    /// The native backend, or `none` when the guest has to use the web fallback.
    pub backend: String,
    pub limits: Limits,
    pub pads: Vec<PadInfo>,
}

/// Emitted to the webview when the set of pads changes.
#[derive(Debug, Clone, PartialEq)]
pub enum PadEvent {
    Connected(PadInfo),
    Changed(PadInfo),
    Disconnected { id: String, slot: u32 },
}

pub const CONNECTED_EVENT: &str = "gamepad-haptics://connected";
pub const CHANGED_EVENT: &str = "gamepad-haptics://changed";
pub const DISCONNECTED_EVENT: &str = "gamepad-haptics://disconnected";
