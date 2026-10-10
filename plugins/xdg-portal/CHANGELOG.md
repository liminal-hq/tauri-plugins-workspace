# Changelog

## [0.3.1]

- [`ae1a21f`](https://github.com/liminal-hq/tauri-plugins-workspace/commit/ae1a21fccba223ac1fb37a9df0656418b094724b) Ships `LICENSE-MIT` and `LICENSE-APACHE` inside each published crate and npm package. The `package.json` `files` lists already named them, but the files were missing from the plugin directories, so the licence texts were not in the packages.

## [0.3.0]

- [`3030fc0`](https://github.com/liminal-hq/tauri-plugins-workspace/commit/3030fc09778cf7d5addcd0731cc03e95e718038b) Notifications can carry action buttons. `sendNotification` in `xdg-portal` and `notify` in `desktop-integration` take an optional `actions` list of `{ id, label }` (at most 3; extras are dropped and logged; ids and labels are validated). `xdg-portal` maps them to the Notification portal's `buttons`, and `desktop-integration` to the `actions` array of `org.freedesktop.Notifications.Notify` after the `default` key. A pressed button, like the default click, arrives through the existing `notification-action` event as `{ id, action }` with the notification's own `id` and the button's id as `action`; `desktop-integration` ignores signals for notifications it did not send. `get_status` gains a `notificationActions` feature: available when the Notification portal is version 1 or later (`xdg-portal`) or the server lists the `actions` capability (`desktop-integration`), otherwise unavailable with the new `actions-unsupported` reason and a `detail` explaining that only the default click is offered. Windows toasts do not show buttons yet and report the feature as unavailable.
- [`1803a95`](https://github.com/liminal-hq/tauri-plugins-workspace/commit/1803a95276c6007d736e1ac7eb402f905d998a66) Adds `get_status`, notifications, inhibit and open-URI to `xdg-portal`, without changing the existing commands. `get_status` reports per feature (`notification`, `inhibit`, `openUri`) whether the portal interface answers and, when it does not, a typed reason. `send_notification` and `withdraw_notification` go through `org.freedesktop.portal.Notification` with an id, title, body, optional default action and urgency, and the `xdg-portal://notification-action` event reports a clicked default action. `inhibit` and `release_inhibit` take and release an idle and suspend inhibitor through `org.freedesktop.portal.Inhibit` by handle, and live inhibitors are released when the app exits. `open_uri` opens a URI, file or folder through `org.freedesktop.portal.OpenURI` with the `ask` and `writable` options. The new commands reject with a typed `ServiceError` and bound every portal call with a timeout. Rust callers reach the same features through `PortalExt`, and the guest-side package adds `getStatus`, `sendNotification`, `withdrawNotification`, `inhibit`, `releaseInhibit`, `openUri` and `onNotificationAction`. The new permissions are `allow-get-status`, `allow-send-notification`, `allow-withdraw-notification`, `allow-inhibit`, `allow-release-inhibit` and `allow-open-uri`; only `allow-get-status` joins the `default` set.

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
d
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
