# Changelog

## [0.3.0]

- [`6010b8d`](https://github.com/liminal-hq/tauri-plugins-workspace/commit/6010b8d3a5a3a3dfa5be8aaf8c64f074dd797b76) Adds desktop services to `desktop-integration`, without changing the existing X11 and Wayland shortcut commands. `get_status` reports per feature whether it works and why not. `notify` and `withdraw_notification` show notifications outside the portal: `org.freedesktop.Notifications` over `zbus` on Linux, and a WinRT toast on Windows when the process has an AppUserModelID (otherwise `needs-app-id`, with `set_app_user_model_id` to set one), and the `desktop-integration://notification-action` event reports a clicked default action. `inhibit_sleep` and `release_sleep_inhibit` hold a systemd-logind inhibitor file descriptor on Linux and a `PowerCreateRequest` on Windows, by handle. `set_launcher_progress` shows one combined progress value, an indeterminate state or nothing, with a badge count: the `com.canonical.Unity.LauncherEntry` `Update` signal on Linux and `ITaskbarList3` on Windows. `own_file_manager` and `disown_file_manager` own `org.freedesktop.FileManager1` on Linux, forwarding `ShowFolders`, `ShowItems` and `ShowItemProperties` as `desktop-integration://file-manager` events and reporting a lost name through `desktop-integration://file-manager-ownership`. `register_global_shortcut` and `unregister_global_shortcut` register Windows hotkeys on a message-only window thread and report presses through `desktop-integration://shortcut-pressed`. The commands reject with a typed `ServiceError` and bound every call with a timeout, and Rust callers reach the same calls through `DesktopServicesExt`. The new permissions are `allow-get-status`, `allow-notify`, `allow-withdraw-notification`, `allow-set-app-user-model-id`, `allow-inhibit-sleep`, `allow-release-sleep-inhibit`, `allow-set-launcher-progress`, `allow-own-file-manager`, `allow-disown-file-manager`, `allow-register-global-shortcut` and `allow-unregister-global-shortcut`; only `allow-get-status` joins the `default` set. The plugin now builds on Windows: it no longer links `ashpd` directly.
- [`3030fc0`](https://github.com/liminal-hq/tauri-plugins-workspace/commit/3030fc09778cf7d5addcd0731cc03e95e718038b) Notifications can carry action buttons. `sendNotification` in `xdg-portal` and `notify` in `desktop-integration` take an optional `actions` list of `{ id, label }` (at most 3; extras are dropped and logged; ids and labels are validated). `xdg-portal` maps them to the Notification portal's `buttons`, and `desktop-integration` to the `actions` array of `org.freedesktop.Notifications.Notify` after the `default` key. A pressed button, like the default click, arrives through the existing `notification-action` event as `{ id, action }` with the notification's own `id` and the button's id as `action`; `desktop-integration` ignores signals for notifications it did not send. `get_status` gains a `notificationActions` feature: available when the Notification portal is version 1 or later (`xdg-portal`) or the server lists the `actions` capability (`desktop-integration`), otherwise unavailable with the new `actions-unsupported` reason and a `detail` explaining that only the default click is offered. Windows toasts do not show buttons yet and report the feature as unavailable.

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
ation — must add it to keep compiling. The rest of the
      change (the `shortcut-changed` event, `ShortcutChangedPayload`, and the
      `check_shortcut_trigger_description` command/permission) is purely additive.

## \[0.1.0]

- [`5067f92`](https://github.com/liminal-hq/tauri-plugins-workspace/commit/5067f9203d48ed00cb9d1d39eeea3d8eeeaed4b0) Release notes for the new `xdg-portal` and `desktop-integration` plugins, promoted from `liminal-hq/emoji-nook`:

  - `xdg-portal` bridges Tauri apps to the Linux `xdg-desktop-portal` D-Bus interfaces: desktop theme detection (colour scheme, accent colour, high contrast) via the Settings portal, and Wayland global shortcut binding via the GlobalShortcuts portal. Its model types are generated from their Rust definitions via `ts-rs` rather than hand-mirrored.
  - `desktop-integration` provides X11 window activation (`_NET_WM_USER_TIME` stamping via `gdkx11`) and a unified `DesktopIntegrationExt` trait that picks X11 direct shortcut binding or the Wayland portal automatically based on session type.
  - `desktop-integration` also exposes a `register_shortcut` command for JS-only consumers, delivering activation via a `shortcut-activated` event instead of the closure Rust callers get from `DesktopIntegrationExt`. Its event payload types (`ShortcutBindingResult`, `ShortcutActivatedPayload`) are generated from their Rust definitions via `ts-rs` rather than hand-mirrored.
  - Both are Linux-only; see each plugin's `[package.metadata.platforms.support]` and README for details.
