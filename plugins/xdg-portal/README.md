# @liminal-hq/plugin-xdg-portal

Bridges Tauri apps to the Linux `xdg-desktop-portal` D-Bus interfaces, so sandboxed
and Wayland apps can request system integration (theming, global shortcuts) through
the standard freedesktop.org portal APIs instead of platform-specific hacks.

## Installation

### Rust

```toml
[dependencies]
tauri-plugin-xdg-portal = "0.1"

# Alternatively with Git:
tauri-plugin-xdg-portal = { git = "https://github.com/liminal-hq/tauri-plugins-workspace", branch = "main" }
```

### JavaScript

```bash
pnpm add @liminal-hq/plugin-xdg-portal
```

## Usage

### Rust

```rust
fn main() {
    tauri::Builder::default()
        .plugin(tauri_plugin_xdg_portal::init())
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
```

### JavaScript

```typescript
import { portal } from '@liminal-hq/plugin-xdg-portal';

const availability = await portal.checkAvailability();
const theme = await portal.getThemeInfo();
```

### Status

`getStatus()` probes the notification, inhibit and open-URI portals and reports, per feature, whether it works and, when it does not, a typed `reason`: `platform-unsupported`, `no-portal`, `interface-missing`, `no-response` or `not-sandboxed`. Hide the options whose feature is unavailable and show the `reason` and `detail` on a status panel.

```typescript
import { portal, isFeatureAvailable } from '@liminal-hq/plugin-xdg-portal';

const status = await portal.getStatus();
if (isFeatureAvailable(status, 'notification')) {
	await portal.sendNotification({
		id: 'copy-12',
		title: 'Copy finished',
		body: '3 files',
		defaultAction: 'show-job',
		urgency: 'normal',
	});
}
```

### Notifications

`sendNotification` shows a notification through `org.freedesktop.portal.Notification`; sending the same `id` again replaces it and `withdrawNotification` takes it off screen. When the user clicks the notification, the `xdg-portal://notification-action` event delivers `{ id, action }` with the request's `defaultAction`; `portal.onNotificationAction` subscribes. An action id that starts with `app.` is activated through `org.freedesktop.Application` instead and never reaches the event. The portal never reports whether a notification was actually presented.

### Inhibit

`inhibit({ reason, kinds })` asks the session not to go idle and not to suspend (`kinds` of `idle` and `suspend`; empty means both) and returns a `handle` for `releaseInhibit`. The inhibitor ends when it is released or the app exits.

### Open a URI

`openUri({ uri, ask, writable })` opens a URI, or with a `file:` URI a file or folder, in the user's chosen application. `ask` always asks which application; `writable` opens a file for writing. The command resolves once the portal has accepted the request, not when the user has chosen.

### Errors

The notification, inhibit and open-URI commands reject with a `ServiceError`, `{ kind, message }`, where `kind` is one of `unsupported-platform`, `portal-unavailable`, `not-allowed`, `invalid-argument`, `not-found`, `timeout` or `failed`; `isServiceError` narrows a caught value. Every portal call is bounded by a 5 second timeout and runs off the main thread. The two earlier commands keep rejecting with a string.

### Rust

The same features are available from Rust through `PortalExt`: `app.portal().status().await`, `send_notification`, `withdraw_notification`, `inhibit`, `release_inhibit` and `open_uri`.

### Global shortcuts (Rust-only)

The `global_shortcuts` module implements the portal `GlobalShortcuts` interface for
Wayland, where raw keyboard grabs are not available to applications. Binding a
shortcut through the portal shows a one-time compositor confirmation dialog, so the
call is asynchronous and needs a parent window once one exists:

```rust
use tauri_plugin_xdg_portal::global_shortcuts::create_session;

let handle = create_session(
    "your-app-toggle",       // stable session/shortcut id
    "Toggle Your App",       // human-readable description shown in the compositor dialog
    Some("<Alt><Shift>t"),   // GTK/libxkbcommon accelerator format
    move || { /* shortcut activated */ },
    move |result| { /* bind result */ },
    move |trigger_description| { /* rebound externally, e.g. "Super+E" */ },
    window_id_receiver,
)
.await?;
```

`trigger_description` is delivered whenever the compositor's own settings UI reports
the shortcut's trigger has changed independently of this call (the portal's
`ShortcutsChanged` signal) — e.g. GNOME Settings → Apps → <App> → Global Shortcuts.

On X11, prefer `tauri-plugin-global-shortcut` directly — the portal path is Wayland-specific.
See [`@liminal-hq/plugin-desktop-integration`](../desktop-integration) for a helper that
picks the right path automatically based on session type.

### Generated types

`AvailabilityInfo`, `ColourScheme`, `DesktopEnvironment`, `AccentColour`, `ThemeInfo` and the status, notification, inhibit, open-URI and error types
are generated from their Rust definitions via [`ts-rs`](https://github.com/Aleph-Alpha/ts-rs)
into `guest-js/bindings/` and re-exported from the package root, so the JS/Rust shapes
can't drift:

```typescript
import type { ThemeInfo, ColourScheme } from '@liminal-hq/plugin-xdg-portal';
```

The bindings regenerate automatically as part of `cargo test` (each type's `#[ts(export)]`
attribute creates a test that writes its `.ts` file) — run `cargo test -p
tauri-plugin-xdg-portal` after changing any model and commit the result.

## Permissions

This plugin requires these permissions:

- `allow-check-availability`: Grants access to `check_availability`
- `allow-get-theme-info`: Grants access to `get_theme_info`
- `allow-get-status`: Grants access to `get_status`
- `allow-send-notification`: Grants access to `send_notification`
- `allow-withdraw-notification`: Grants access to `withdraw_notification`
- `allow-inhibit`: Grants access to `inhibit`
- `allow-release-inhibit`: Grants access to `release_inhibit`
- `allow-open-uri`: Grants access to `open_uri`

The `default` set grants only the read-only `check_availability`, `get_theme_info` and `get_status`; grant the others explicitly.

## Platform Support

| Platform | Support Level | Notes                                                                |
| -------- | ------------- | -------------------------------------------------------------------- |
| Windows  | None          | `xdg-desktop-portal` is Linux-only                                   |
| Linux    | Full          | Bridges Settings, GlobalShortcuts, Notification, Inhibit and OpenURI |
| macOS    | None          | `xdg-desktop-portal` is Linux-only                                   |
| Android  | None          | `xdg-desktop-portal` is Linux-only                                   |
| iOS      | None          | `xdg-desktop-portal` is Linux-only                                   |

## Licence

Apache-2.0 OR MIT
