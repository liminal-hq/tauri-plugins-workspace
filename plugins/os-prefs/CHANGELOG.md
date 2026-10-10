# Changelog

## [0.1.1]

- [`ae1a21f`](https://github.com/liminal-hq/tauri-plugins-workspace/commit/ae1a21fccba223ac1fb37a9df0656418b094724b) Ships `LICENSE-MIT` and `LICENSE-APACHE` inside each published crate and npm package. The `package.json` `files` lists already named them, but the files were missing from the plugin directories, so the licence texts were not in the packages.

## [0.1.0]

- [`aafa221`](https://github.com/liminal-hq/tauri-plugins-workspace/commit/aafa2218a233bad4890693a9efdaa911b9317fd5) Adds the `os-prefs` plugin, which reports the user's real 12/24-hour clock preference instead of the locale's default, and pushes a `os-prefs://time-format-changed` event when the setting changes. On Linux it reads GNOME's `clock-format` through xdg-desktop-portal (falling back to `gsettings`), Cinnamon's `clock-use-24h` through `gsettings`, and the `LC_TIME` locale's convention on other desktops. On Windows it reads the Region short-time pattern and `LOCALE_ITIME` through `GetLocaleInfoEx` and polls for changes. On macOS and iOS it reads the hour pattern Foundation generates for the user's locale, and on macOS it follows locale-change notifications. Android keeps the `DateFormat.is24HourFormat`, animator duration scale and notification settings commands, with the wire format, permission identifiers and Android bridge unchanged apart from a `source` field added to the time format reply. `getStatus()` reports which features work and why the others do not. Its model types are generated from their Rust definitions via `ts-rs`.
lows locale-change notifications. Android keeps the `DateFormat.is24HourFormat`, animator duration scale and notification settings commands, with the wire format, permission identifiers and Android bridge unchanged apart from a `source` field added to the time format reply. `getStatus()` reports which features work and why the others do not. Its model types are generated from their Rust definitions via `ts-rs`.
