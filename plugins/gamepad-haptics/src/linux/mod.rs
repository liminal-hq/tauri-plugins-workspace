// Plays rumble on Linux through evdev force feedback, without reading or grabbing the pad
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

mod device;
mod hotplug;

use std::{collections::HashMap, path::PathBuf, sync::Mutex};

use evdev::{Device, FFEffect, FFEffectData, FFEffectKind, FFReplay, FFTrigger};

use crate::{
    backend::{DiscoveredPad, Levels, Notify, RumbleBackend},
    Error, Result,
};

/// The longest an effect may last: the kernel stores its length as a signed 16-bit value.
const MAX_REPLAY_MS: u64 = 0x7fff;

struct Open {
    /// Kept so the effect's device stays open; the kernel clears the effect when it closes.
    _device: Device,
    effect: FFEffect,
}

pub struct EvdevBackend {
    paths: Mutex<HashMap<String, (PathBuf, u16, u16)>>,
    open: Mutex<HashMap<String, Open>>,
}

impl EvdevBackend {
    pub fn new() -> Self {
        Self {
            paths: Mutex::new(HashMap::new()),
            open: Mutex::new(HashMap::new()),
        }
    }
}

impl Default for EvdevBackend {
    fn default() -> Self {
        Self::new()
    }
}

fn magnitude(level: f64) -> u16 {
    (level.clamp(0.0, 1.0) * f64::from(u16::MAX)).round() as u16
}

fn failed(key: &str, e: std::io::Error) -> Error {
    log::warn!("gamepad-haptics: {key}: {e}");
    Error::Backend(format!("{key}: {e}"))
}

impl RumbleBackend for EvdevBackend {
    fn name(&self) -> &'static str {
        "evdev"
    }

    fn scan(&self) -> Vec<DiscoveredPad> {
        let found = device::scan();
        if let Ok(mut paths) = self.paths.lock() {
            paths.clear();
            for f in &found {
                paths.insert(
                    f.pad.key.clone(),
                    (f.path.clone(), f.pad.vendor_id, f.pad.product_id),
                );
            }
        }
        // Forget handles for pads that are gone, which also lets the kernel drop their effects.
        if let (Ok(mut open), Ok(paths)) = (self.open.lock(), self.paths.lock()) {
            open.retain(|key, _| paths.contains_key(key));
        }
        found.into_iter().map(|f| f.pad).collect()
    }

    fn set(&self, key: &str, levels: Levels, hold_ms: u64) -> Result<()> {
        let (path, vendor, product) = self
            .paths
            .lock()
            .ok()
            .and_then(|p| p.get(key).cloned())
            .ok_or_else(|| Error::UnknownPad(key.to_string()))?;

        let mut light = levels.light;
        if device::light_motor_is_binary(vendor, product) {
            light = if light >= 0.5 { 1.0 } else { 0.0 };
        }
        let data = FFEffectData {
            direction: 0,
            trigger: FFTrigger::default(),
            replay: FFReplay {
                length: hold_ms.clamp(1, MAX_REPLAY_MS) as u16,
                delay: 0,
            },
            kind: FFEffectKind::Rumble {
                strong_magnitude: magnitude(levels.heavy),
                weak_magnitude: magnitude(light),
            },
        };

        let mut open = self
            .open
            .lock()
            .map_err(|_| Error::Backend("poisoned".into()))?;
        if let Some(entry) = open.get_mut(key) {
            let result = entry.effect.update(data).and_then(|_| entry.effect.play(1));
            match result {
                Ok(()) => return Ok(()),
                // The handle went stale, for example after a reconnect: reopen below.
                Err(_) => {
                    open.remove(key);
                }
            }
        }
        let mut device = Device::open(&path).map_err(|e| failed(key, e))?;
        let mut effect = device.upload_ff_effect(data).map_err(|e| failed(key, e))?;
        effect.play(1).map_err(|e| failed(key, e))?;
        open.insert(
            key.to_string(),
            Open {
                _device: device,
                effect,
            },
        );
        Ok(())
    }

    fn silence(&self, key: &str) -> Result<()> {
        let mut open = self
            .open
            .lock()
            .map_err(|_| Error::Backend("poisoned".into()))?;
        if let Some(entry) = open.get_mut(key) {
            if let Err(e) = entry.effect.stop() {
                // The pad is gone or the handle is stale; dropping it clears the effect.
                open.remove(key);
                log::debug!("gamepad-haptics: {key}: {e}");
            }
        }
        Ok(())
    }

    fn watch(&self, notify: Notify) {
        hotplug::watch(notify);
    }
}
