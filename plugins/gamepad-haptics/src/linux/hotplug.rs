// Watches /dev/input for pads being plugged in or removed
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

use std::{thread, time::Duration};

use inotify::{Inotify, WatchMask};

use crate::backend::Notify;

/// Time to let udev finish applying permissions to a new node before it is opened.
const SETTLE: Duration = Duration::from_millis(250);

/// Starts a thread that calls `notify` whenever an event node appears or disappears. The thread
/// ends when `notify` reports it should (by returning from the loop on an inotify error).
pub fn watch(notify: Notify) {
    let spawned = thread::Builder::new()
        .name("gamepad-haptics-hotplug".into())
        .spawn(move || {
            let Ok(mut inotify) = Inotify::init() else {
                log::warn!("gamepad-haptics: could not start the hot-plug watcher");
                return;
            };
            if inotify
                .watches()
                .add(
                    "/dev/input",
                    WatchMask::CREATE | WatchMask::DELETE | WatchMask::ATTRIB,
                )
                .is_err()
            {
                log::warn!("gamepad-haptics: could not watch /dev/input");
                return;
            }
            let mut buffer = [0u8; 4096];
            loop {
                match inotify.read_events_blocking(&mut buffer) {
                    Ok(mut events) => {
                        let relevant = events.any(|e| {
                            e.name
                                .is_some_and(|n| n.to_string_lossy().starts_with("event"))
                        });
                        if relevant {
                            thread::sleep(SETTLE);
                            notify();
                        }
                    }
                    Err(e) => {
                        log::warn!("gamepad-haptics: hot-plug watcher stopped: {e}");
                        return;
                    }
                }
            }
        });
    if let Err(e) = spawned {
        log::warn!("gamepad-haptics: could not start the hot-plug thread: {e}");
    }
}
