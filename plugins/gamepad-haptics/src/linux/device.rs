// Finds rumble-capable gamepads under /dev/input and describes them
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

use std::{
    fs::{self, OpenOptions},
    path::{Path, PathBuf},
};

use evdev::{BusType, Device, FFEffectCode, KeyCode};

use crate::{backend::DiscoveredPad, models::Transport};

/// A pad found on disk, with the path to open it for rumble.
pub struct Found {
    pub pad: DiscoveredPad,
    pub path: PathBuf,
}

/// The SDL-style GUID for a device, which SDL and gilrs report for the same pad: the bus, vendor,
/// product and version as little-endian 16-bit values, each followed by two zero bytes.
pub fn sdl_guid(bus: u16, vendor: u16, product: u16, version: u16) -> String {
    [bus, vendor, product, version]
        .iter()
        .flat_map(|value| {
            let [lo, hi] = value.to_le_bytes();
            [lo, hi, 0, 0]
        })
        .map(|b| format!("{b:02x}"))
        .collect()
}

/// A pad's stable key: vendor, product and its unique name (a Bluetooth address, or a serial over
/// USB), else its physical path, else the device path.
pub fn stable_key(
    vendor: u16,
    product: u16,
    uniq: Option<&str>,
    phys: Option<&str>,
    path: &Path,
) -> String {
    let tail = [uniq, phys]
        .into_iter()
        .flatten()
        .find(|s| !s.trim().is_empty())
        .map(str::to_string)
        .unwrap_or_else(|| path.display().to_string());
    format!("{vendor:04x}:{product:04x}:{tail}")
}

fn transport(bus: BusType) -> Transport {
    if bus == BusType::BUS_USB {
        Transport::Usb
    } else if bus == BusType::BUS_BLUETOOTH {
        Transport::Bluetooth
    } else {
        Transport::Unknown
    }
}

/// Describes the device at `path` when it is a gamepad that can rumble.
pub fn inspect(path: &Path) -> Option<Found> {
    let device = Device::open(path).ok()?;
    let has_rumble = device
        .supported_ff()
        .is_some_and(|ff| ff.contains(FFEffectCode::FF_RUMBLE));
    let is_gamepad = device
        .supported_keys()
        .is_some_and(|keys| keys.contains(KeyCode::BTN_SOUTH));
    if !has_rumble || !is_gamepad {
        return None;
    }

    let id = device.input_id();
    let uniq = device
        .unique_name()
        .map(str::to_string)
        .filter(|s| !s.is_empty());
    let writable = OpenOptions::new().write(true).open(path).is_ok();
    let (top_tier, reason) = if writable {
        (2, None)
    } else {
        (
            0,
            Some(format!(
                "No write access to {}; a udev rule or the input group is needed",
                path.display()
            )),
        )
    };

    Some(Found {
        pad: DiscoveredPad {
            key: stable_key(
                id.vendor(),
                id.product(),
                uniq.as_deref(),
                device.physical_path(),
                path,
            ),
            name: device.name().unwrap_or("Gamepad").to_string(),
            vendor_id: id.vendor(),
            product_id: id.product(),
            serial: uniq,
            transport: transport(id.bus_type()),
            guid: sdl_guid(id.bus_type().0, id.vendor(), id.product(), id.version()),
            motors: 2,
            triggers: false,
            light_binary: light_motor_is_binary(id.vendor(), id.product()),
            weak_heavy: heavy_motor_is_weak(id.vendor(), id.product()),
            top_tier,
            reason,
        },
        path: path.to_path_buf(),
    })
}

/// Every rumble-capable gamepad now present, in path order.
pub fn scan() -> Vec<Found> {
    let Ok(entries) = fs::read_dir("/dev/input") else {
        return Vec::new();
    };
    let mut paths: Vec<PathBuf> = entries
        .filter_map(|e| e.ok())
        .map(|e| e.path())
        .filter(|p| {
            p.file_name()
                .and_then(|n| n.to_str())
                .is_some_and(|n| n.starts_with("event"))
        })
        .collect();
    paths.sort_by_key(|p| {
        p.file_name()
            .and_then(|n| n.to_str())
            .and_then(|n| n.trim_start_matches("event").parse::<u32>().ok())
            .unwrap_or(u32::MAX)
    });
    paths.iter().filter_map(|p| inspect(p)).collect()
}

/// Pads whose light motor only switches on and off, so it is driven at full strength or not at all.
pub fn light_motor_is_binary(vendor: u16, product: u16) -> bool {
    // DualShock 3 under hid-sony.
    (vendor, product) == (0x054c, 0x0268)
}

/// Pads whose heavy motor does not spin up for a short soft tap: a DualShock 4 felt nothing from
/// 60 ms at 0.6 but felt 90 ms at 0.6 and 60 ms at 1.0.
pub fn heavy_motor_is_weak(vendor: u16, product: u16) -> bool {
    // DualShock 4, first and second revisions.
    vendor == 0x054c && matches!(product, 0x05c4 | 0x09cc)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn guids_match_the_sdl_layout() {
        // A DualShock 3 over USB: bus 3, vendor 054c, product 0268, version 8111.
        assert_eq!(
            sdl_guid(3, 0x054c, 0x0268, 0x8111),
            "030000004c0500006802000011810000"
        );
        assert_eq!(sdl_guid(5, 0x054c, 0x05c4, 0x8000).len(), 32);
    }

    #[test]
    fn keys_prefer_the_unique_name_then_the_physical_path() {
        let path = Path::new("/dev/input/event5");
        assert_eq!(
            stable_key(0x54c, 0x268, Some("aa:bb"), Some("usb-1"), path),
            "054c:0268:aa:bb"
        );
        assert_eq!(
            stable_key(0x54c, 0x268, Some(""), Some("usb-1"), path),
            "054c:0268:usb-1"
        );
        assert_eq!(
            stable_key(0x54c, 0x268, None, None, path),
            "054c:0268:/dev/input/event5"
        );
    }

    #[test]
    fn the_dualshock_4_has_a_weak_heavy_motor_and_the_dualshock_3_does_not() {
        assert!(heavy_motor_is_weak(0x054c, 0x05c4));
        assert!(heavy_motor_is_weak(0x054c, 0x09cc));
        assert!(!heavy_motor_is_weak(0x054c, 0x0268));
    }

    #[test]
    fn only_the_dualshock_3_has_a_binary_light_motor() {
        assert!(light_motor_is_binary(0x054c, 0x0268));
        assert!(!light_motor_is_binary(0x054c, 0x05c4));
    }
}
