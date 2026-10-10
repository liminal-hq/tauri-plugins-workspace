---
'haptics': minor
'haptics-js': minor
---

Adds the `haptics` plugin, which authors and replays haptic patterns on Android. Portable patterns are registered once and compiled for each device down a five-tier ladder (envelope on API 36 and newer, primitives, amplitude, on/off, off), so a call always resolves with the tier that played and the reason for any downgrade instead of failing silently. The plugin plays one-shot, waveform, predefined, composition and envelope effects, reports per-primitive capabilities, exposes a UI lane backed by the view's haptic feedback, and applies per-pattern policies (`interrupt`, `queue`, `drop-if-busy` and `coalesce`). Raw requests are validated in Rust on every platform before the master scale, the tier cap or the hardware can affect them, so invalid input rejects the same way everywhere. Desktop builds resolve every call at tier 0 so patterns still validate and compile off-device. The pattern validator, compiler and scheduler are pure TypeScript with no `@tauri-apps` imports.
