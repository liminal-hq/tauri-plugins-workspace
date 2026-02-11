# Changelog

## \[0.1.2]

- [`d604a44`](https://github.com/liminal-hq/tauri-plugins-workspace/commit/d604a4420aeef36466cec363ea68ef112e046df7) Release updates for `material-you` and `material-you-js`:
  - Adds concrete guest TypeScript response types for `getMaterialYouColours`.
  - Updates the guest API to return `Promise<MaterialYouResponse>` for stronger IDE and compile-time feedback.
  - Aligns IIFE build output with guarded Tauri global registration behaviour.

## \[0.1.1]

- [`bee66cb`](https://github.com/liminal-hq/tauri-plugins-workspace/commit/bee66cb65f6d9adb822576ce3a721617c3db6b57) Release updates for `material-you` and `material-you-js`:
  - Improves plugin metadata discoverability across npm and crates.io.
  - Updates plugin documentation with package links, install alternatives, and issue reporting guidance.
  - Fixes guest JavaScript bundle configuration warnings in Rollup output.
  - Aligns repository references and links with `tauri-plugins-workspace`.

## \[0.1.0]

- [`cd20a4b`](https://github.com/liminal-hq/tauri-plugins-workspace/commit/cd20a4bec1c2b523cc397733242496d5e971f10a) Release notes for the new `material-you` plugin:
  - Adds an Android-only Material You plugin with Rust and guest JavaScript packages.
  - Exposes `get_material_you_colours` for retrieving dynamic colour palette data from Android system resources.
  - Includes plugin documentation, permissions wiring, and automated test coverage for Rust and guest JavaScript bindings.
