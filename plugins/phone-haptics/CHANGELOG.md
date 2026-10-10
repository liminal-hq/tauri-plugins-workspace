# Changelog

## [0.1.1]

- [`ae1a21f`](https://github.com/liminal-hq/tauri-plugins-workspace/commit/ae1a21fccba223ac1fb37a9df0656418b094724b) Ships `LICENSE-MIT` and `LICENSE-APACHE` inside each published crate and npm package. The `package.json` `files` lists already named them, but the files were missing from the plugin directories, so the licence texts were not in the packages.

## [0.1.0]

- [`b7744fd`](https://github.com/liminal-hq/tauri-plugins-workspace/commit/b7744fd2992d5e56703999c6dc0d113780ff2694) Adds the `haptics` plugin, which authors and replays haptic patterns on Android. Portable patterns are registered once and compiled for each device down a five-tier ladder (envelope on API 36 and newer, primitives, amplitude, on/off, off), so a call always resolves with the tier that played and the reason for any downgrade instead of failing silently. The plugin plays one-shot, waveform, predefined, composition and envelope effects, reports per-primitive capabilities, exposes a UI lane backed by the view's haptic feedback, and applies per-pattern policies (`interrupt`, `queue`, `drop-if-busy` and `coalesce`). Raw requests are validated in Rust on every platform before the master scale, the tier cap or the hardware can affect them, so invalid input rejects the same way everywhere. Desktop builds resolve every call at tier 0 so patterns still validate and compile off-device. The pattern validator, compiler and scheduler are pure TypeScript with no `@tauri-apps` imports.
