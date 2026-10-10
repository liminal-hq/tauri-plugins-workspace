# Changelog

## [0.1.1]

- [`ae1a21f`](https://github.com/liminal-hq/tauri-plugins-workspace/commit/ae1a21fccba223ac1fb37a9df0656418b094724b) Ships `LICENSE-MIT` and `LICENSE-APACHE` inside each published crate and npm package. The `package.json` `files` lists already named them, but the files were missing from the plugin directories, so the licence texts were not in the packages.

## [0.1.0]

- [`e7b9252`](https://github.com/liminal-hq/tauri-plugins-workspace/commit/e7b9252c4d5f10ff2648b2d3f210dcc3bd3c48c9) Adds the `gamepad-haptics` plugin, which plays rumble on gamepads without reading their input, so it never competes with the webview's Gamepad API for the same device. Authors write one pattern of taps and hums with intensity and sharpness, the same format the phone plugin uses, and the plugin compiles it for the pad's motors: crisp hits go to the light motor, dull ones to the heavy motor, and a single-motor pad gets the stronger of the two. Rust validates every request, caps continuous runs and stops every pad on exit, unplug and window blur. On Linux it drives evdev force feedback, lists pads with a stable slot, an SDL-style GUID and hot-plug events, and `identify` buzzes one pad so a player can tell identical pads apart. Where no native pad can be addressed, the guest falls back to `vibrationActuator` in the webview. Triggers, Windows, macOS, Android and iOS backends are still to come.
