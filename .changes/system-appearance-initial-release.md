---
'system-appearance': minor
'system-appearance-js': minor
---

Adds the `system-appearance` plugin, which reads the operating system's window titlebar preferences (button layout and titlebar actions) for Tauri desktop apps. On Linux it reads xdg-desktop-portal, kwinrc, gsettings or xfconf depending on the desktop environment and pushes changes as they happen. On Windows and macOS it reports the fixed platform layout, and on macOS it also reads the double-click action from NSGlobalDomain; changes are not pushed on macOS. Its model types are generated from their Rust definitions via `ts-rs`.
