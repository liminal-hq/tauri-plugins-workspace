# Changelog

## [0.2.0]

- [`9399b02`](https://github.com/liminal-hq/tauri-plugins-workspace/commit/9399b02dfa5eeaf9743d2f93da560673ebd814c1) `create_session` (xdg-portal) and `DesktopIntegrationExt` (desktop-integration) now
    surface the GlobalShortcuts portal's `ShortcutsChanged` signal, which fires when the
    user rebinds a shortcut's trigger through the compositor's own settings UI (e.g. GNOME
    Settings → Apps → <App> → Global Shortcuts) instead of through the app.
    
    - **Breaking (xdg-portal):** `create_session()` gains a new required
      `on_shortcuts_changed` callback parameter, inserted before `window_rx`. Any direct
      caller must update its call site.
    - **Breaking (desktop-integration):** `DesktopIntegrationExt` gains a new required
      trait method, `last_shortcut_trigger_description`, with no default implementation
      (matching every other method on this trait). Any crate implementing
      `DesktopIntegrationExt` for its own type — rather than relying on this crate's own
      `AppHandle<R>` implementation — must add it to keep compiling. The rest of the
      change (the `shortcut-changed` event, `ShortcutChangedPayload`, and the
      `check_shortcut_trigger_description` command/permission) is purely additive.

## \[0.1.0]

- [`5067f92`](https://github.com/liminal-hq/tauri-plugins-workspace/commit/5067f9203d48ed00cb9d1d39eeea3d8eeeaed4b0) Release notes for the new `xdg-portal` and `desktop-integration` plugins, promoted from `liminal-hq/emoji-nook`:

  - `xdg-portal` bridges Tauri apps to the Linux `xdg-desktop-portal` D-Bus interfaces: desktop theme detection (colour scheme, accent colour, high contrast) via the Settings portal, and Wayland global shortcut binding via the GlobalShortcuts portal. Its model types are generated from their Rust definitions via `ts-rs` rather than hand-mirrored.
  - `desktop-integration` provides X11 window activation (`_NET_WM_USER_TIME` stamping via `gdkx11`) and a unified `DesktopIntegrationExt` trait that picks X11 direct shortcut binding or the Wayland portal automatically based on session type.
  - `desktop-integration` also exposes a `register_shortcut` command for JS-only consumers, delivering activation via a `shortcut-activated` event instead of the closure Rust callers get from `DesktopIntegrationExt`. Its event payload types (`ShortcutBindingResult`, `ShortcutActivatedPayload`) are generated from their Rust definitions via `ts-rs` rather than hand-mirrored.
  - Both are Linux-only; see each plugin's `[package.metadata.platforms.support]` and README for details.
