# Changelog

## [0.1.1]

- [`ae1a21f`](https://github.com/liminal-hq/tauri-plugins-workspace/commit/ae1a21fccba223ac1fb37a9df0656418b094724b) Ships `LICENSE-MIT` and `LICENSE-APACHE` inside each published crate and npm package. The `package.json` `files` lists already named them, but the files were missing from the plugin directories, so the licence texts were not in the packages.

## [0.1.0]

- [`2a135eb`](https://github.com/liminal-hq/tauri-plugins-workspace/commit/2a135eb392a10d717984e495f2fc29ee42176f82) Add the `window-manager` plugin, which asks the compositor to show its own window menu, reports which window manager features are available, and reads the window manager's real Always on Top state (on X11 it also reports changes made from the window manager's own menu). The system window menu works on Linux (verified by hand on GNOME/Wayland; the X11 Always on Top read and change event are verified under XWayland; KDE and native X11 are untested). Windows shows its system menu and macOS pops up the app's Window menu; both are type-checked against their targets but have not been run on those systems, and log each native step so a crash names it.
