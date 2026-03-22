---
'material-you': patch
'material-you-js': patch
---

Fixes the Android build for `material-you` by restoring the `tauri::Manager` import required by the plugin setup path, and keeps the Rust and JavaScript package versions aligned for the patch release.
