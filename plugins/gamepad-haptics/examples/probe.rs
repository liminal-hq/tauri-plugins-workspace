// Lists the gamepads the plugin can address and, on request, rumbles one, for checking real hardware
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! `cargo run -p tauri-plugin-gamepad-haptics --example probe` lists pads.
//! `... --example probe -- identify gamepad:0` buzzes one pad.
//! `... --example probe -- play gamepad:0` plays a swell from the heavy to the light motor.
//! `... --example probe -- watch` prints pads as they are plugged in and removed.

#[cfg(target_os = "linux")]
fn main() {
    use std::{sync::Arc, thread, time::Duration};
    use tauri_plugin_gamepad_haptics::{
        config::Config, linux::EvdevBackend, Emit, Frame, GamepadHaptics, PlayFramesArgs,
    };

    let args: Vec<String> = std::env::args().skip(1).collect();
    let emit: Emit = Arc::new(|event| println!("event: {event:?}"));
    let haptics = GamepadHaptics::new(Arc::new(EvdevBackend::new()), Config::default(), emit);

    let pads = haptics.list_pads().expect("list pads");
    if pads.is_empty() {
        println!("no rumble-capable gamepads found");
    }
    for pad in &pads {
        println!(
            "{}  {}  {:04x}:{:04x}  {:?}  tier {}  guid {}{}",
            pad.id,
            pad.name,
            pad.vendor_id,
            pad.product_id,
            pad.transport,
            pad.top_tier,
            pad.guid,
            pad.reason
                .as_ref()
                .map(|r| format!("  ({r})"))
                .unwrap_or_default()
        );
    }

    match (args.first().map(String::as_str), args.get(1)) {
        (Some("identify"), Some(id)) => {
            println!("{:?}", haptics.identify(id));
            thread::sleep(Duration::from_millis(900));
        }
        (Some("play"), Some(id)) => {
            let frames: Vec<Frame> = (0..=20)
                .map(|i| {
                    let t = f64::from(i) / 20.0;
                    Frame {
                        duration_ms: 50,
                        heavy: 1.0 - t,
                        light: t,
                        left_trigger: None,
                        right_trigger: None,
                    }
                })
                .collect();
            let res = haptics.play_frames(PlayFramesArgs {
                pad_id: id.clone(),
                frames,
                scale: None,
            });
            println!("{res:?}");
            thread::sleep(Duration::from_millis(1_200));
        }
        (Some("watch"), _) => {
            println!("watching; press Ctrl-C to stop");
            loop {
                thread::sleep(Duration::from_secs(3600));
            }
        }
        _ => {}
    }
    haptics.stop(None).expect("stop");
}

#[cfg(not(target_os = "linux"))]
fn main() {
    println!("the probe drives evdev, which only Linux has");
}
