// Defines Steam Input types and the backend trait for controllers, actions and glyphs
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

use serde::{Deserialize, Serialize};

use crate::{
    backend::{unsupported, BackendReason},
    error::Result,
};

/// A connected controller. The handle is a decimal string, because JavaScript numbers lose 64-bit
/// precision.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct InputController {
    pub handle: String,
    /// The controller family, such as `steamDeck`, `ps5` or `xboxOne`.
    pub kind: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DigitalActionState {
    /// Whether the action is bound in the active action set.
    pub active: bool,
    pub pressed: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AnalogActionState {
    pub active: bool,
    pub x: f32,
    pub y: f32,
    /// How the action is bound, such as `joystickMove` or `trigger`.
    pub mode: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ActionKind {
    Digital,
    Analog,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum GlyphSize {
    Small,
    Medium,
    Large,
}

/// Steam Input calls. Handles are raw 64-bit values; zero is never valid. The defaults report the
/// backend as unable.
pub trait InputBackend: BackendReason {
    /// Starts Steam Input. Steam updates it whenever callbacks run, so a short callback interval
    /// makes input more responsive.
    fn input_init(&self) -> Result<()> {
        unsupported(self.unavailable_reason())
    }
    fn input_controllers(&self) -> Result<Vec<InputController>> {
        unsupported(self.unavailable_reason())
    }
    /// Looks up an action set declared in the app's Steam Input action manifest.
    fn input_action_set(&self, _name: &str) -> Result<u64> {
        unsupported(self.unavailable_reason())
    }
    fn input_activate_action_set(&self, _controller: u64, _action_set: u64) -> Result<()> {
        unsupported(self.unavailable_reason())
    }
    fn input_action(&self, _kind: ActionKind, _name: &str) -> Result<u64> {
        unsupported(self.unavailable_reason())
    }
    fn input_digital_state(&self, _controller: u64, _action: u64) -> Result<DigitalActionState> {
        unsupported(self.unavailable_reason())
    }
    fn input_analog_state(&self, _controller: u64, _action: u64) -> Result<AnalogActionState> {
        unsupported(self.unavailable_reason())
    }
    /// The PNG for the first button bound to an action on this controller.
    fn input_glyph(
        &self,
        _controller: u64,
        _action_set: u64,
        _kind: ActionKind,
        _action: u64,
        _size: GlyphSize,
    ) -> Result<Vec<u8>> {
        unsupported(self.unavailable_reason())
    }
}
