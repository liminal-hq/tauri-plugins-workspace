# @liminal-hq/plugin-desktop-integration

Desktop activation helpers for Linux Tauri apps: native X11 window activation and a
unified global-shortcut API that picks the right binding path (X11 direct grab vs.
Wayland portal) automatically based on session type.

- Requests native GTK window presentation with a real event timestamp and stamps
  `_NET_WM_USER_TIME` through `gdkx11`, so fresh windows look like legitimate
  user-driven activations under X11 window managers.
- Wraps `tauri-plugin-global-shortcut` on X11 and
  [`@liminal-hq/plugin-xdg-portal`](../xdg-portal)'s `GlobalShortcuts` portal binding
  on Wayland behind one `DesktopIntegrationExt` trait, so calling apps don't need to
  branch on session type themselves.
- On non-Linux platforms, the activation helper is a documented no-op — see
  [Platform Support](#platform-support).

## Installation

### Rust

```toml
[dependencies]
tauri-plugin-desktop-integration = "0.3"

# Alternatively with Git:
tauri-plugin-desktop-integration = { git = "https://github.com/liminal-hq/tauri-plugins-workspace", branch = "main" }
```

### JavaScript

```bash
pnpm add @liminal-hq/plugin-desktop-integration
```

## Usage

### Rust

```rust
use tauri_plugin_desktop_integration::DesktopIntegrationExt;

fn main() {
    tauri::Builder::default()
        .plugin(tauri_plugin_desktop_integration::init())
        .plugin(tauri_plugin_global_shortcut::Builder::new().build())
        .plugin(tauri_plugin_xdg_portal::init())
        .setup(|app| {
            let handle = app.handle().clone();
            handle.register_shortcut(
                "your-app-toggle",   // stable Wayland portal session id
                "Toggle Your App",   // shown in the compositor's shortcut dialog
                "Alt+Shift+T",
                move || { /* shortcut activated */ },
            );
            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
```

On Wayland, the portal `BindShortcuts` call requires a parent window for its
confirmation dialog. Call `set_shortcut_window(&window)` once your first window is
shown to kick off the deferred binding, and listen for the `shortcut-binding-result`
event to know when it resolves.

The compositor's own settings UI can also rebind the shortcut independently of the app
(e.g. GNOME Settings → Apps → <App> → Global Shortcuts) — listen for the
`shortcut-changed` event (Wayland-only) to keep your own UI in sync.
`DesktopIntegrationExt::last_shortcut_trigger_description` covers the race where the
rebind happens before your listener attaches.

### JavaScript

JS-only apps (no custom Rust command of their own) can register shortcuts directly —
activation is delivered as a `shortcut-activated` event under the hood, but
`registerShortcut` hides that and takes a plain callback:

```typescript
import { desktopIntegration } from '@liminal-hq/plugin-desktop-integration';

await desktopIntegration.registerShortcut(
	'your-app-toggle', // stable Wayland portal session id
	'Toggle Your App', // shown in the compositor's shortcut dialog
	'Alt+Shift+T',
	() => {
		/* shortcut activated */
	}
);

const complete = await desktopIntegration.checkShortcutBindingComplete();
const error = await desktopIntegration.checkShortcutBindingError();
```

Rust consumers should prefer calling `DesktopIntegrationExt::register_shortcut` directly
from `setup()` — it delivers activation via a real closure instead of an event
round-trip. The `register_shortcut` command exists specifically for JS-only consumers.

To detect an external rebind (Wayland-only), listen for `shortcut-changed` directly via
`@tauri-apps/api/event`, and use `checkShortcutTriggerDescription` as a race guard for
UI that mounts after a missed event:

```typescript
import { listen } from '@tauri-apps/api/event';
import {
	desktopIntegration,
	type ShortcutChangedPayload,
} from '@liminal-hq/plugin-desktop-integration';

await listen<ShortcutChangedPayload>('shortcut-changed', ({ payload }) => {
	console.log(`rebound externally to: ${payload.triggerDescription}`);
});

const lastKnownTrigger = await desktopIntegration.checkShortcutTriggerDescription();
```

### Services

Besides window activation and shortcuts, the plugin offers desktop services that are not behind a portal. Use [`@liminal-hq/plugin-xdg-portal`](../xdg-portal) first where the portal exists, and these where it does not (or on Windows). Every service rejects with a `ServiceError`, `{ kind, message }`, where `kind` is one of `unsupported-platform`, `unavailable`, `needs-app-id`, `invalid-argument`, `not-found`, `conflict`, `timeout` or `failed`. D-Bus work runs off the main thread and every call is bounded by a 5 second timeout.

`getStatus()` reports, per feature (`notify`, `notificationActions`, `inhibitSleep`, `launcherProgress`, `fileManager`, `globalShortcuts`), whether it works here and, if not, a typed `reason` (`platform-unsupported`, `no-session-bus`, `no-notification-server`, `no-logind`, `needs-app-id`, `no-display-server`, `actions-unsupported`) with the `detail` behind it. Hide options whose feature is unavailable.

- **Notifications:** `notify({ id, title, body, defaultAction, urgency })` shows a notification and replaces the one with the same `id`; `withdrawNotification(id)` closes it; `onNotificationAction` reports a click on it as `{ id, action }`. On Linux it calls `org.freedesktop.Notifications` and sets the `desktop-entry` hint from `desktopId` (default: the bundle identifier). Add up to three buttons with `actions: [{ id, label }]` (ids are 1 to 256 bytes, unique, not `default` and different from `defaultAction`; labels are 1 to 100 characters; further buttons are dropped with a log line): they go to the `actions` array of `Notify` after the `default` key that `defaultAction` adds, and a press arrives through `onNotificationAction` as `{ id, action }` with the button's id. Signals for notifications this plugin did not send are ignored. The `notificationActions` feature is available when the server lists the `actions` capability; some servers show only the default click, and `detail` says so. On Windows it shows a WinRT toast, which needs an AppUserModelID: set one with `setAppUserModelId(id)` (or `SetCurrentProcessExplicitAppUserModelID` yourself) and until then `notify` rejects with `needs-app-id`.
- **Sleep inhibit:** `inhibitSleep({ reason, kinds })` returns a `handle` for `releaseSleepInhibit`. On Linux it takes a blocking `sleep` (and, with the `idle` kind, `idle`) inhibitor from systemd-logind and holds its file descriptor; on Windows it holds a `PowerCreateRequest` for `PowerRequestSystemRequired`. Inhibitors end when released or when the app exits.
- **Launcher progress:** `setLauncherProgress({ progress, count, desktopId, windowLabel })` shows `{ state: 'value', value }` (0 to 1), `{ state: 'indeterminate' }` or `{ state: 'cleared' }`. On Linux it emits the `com.canonical.Unity.LauncherEntry` `Update` signal (`progress`, `progress-visible`, `count`, `count-visible`) for `application://<desktopId>.desktop`; docks have no indeterminate state, so it shows an empty bar, and the signal is a broadcast that succeeds whether or not a dock listens. On Windows it sets `ITaskbarList3` progress on the button of `windowLabel` (default: the focused window, else the first label starting with `main`, else the first label in alphabetical order).
- **File manager (Linux):** `ownFileManager()` takes `org.freedesktop.FileManager1` and serves `ShowFolders`, `ShowItems` and `ShowItemProperties`; each call arrives through `onFileManagerCall` as `{ method, targets, startupId }`, where each target has the URI and, for `file:` URIs on this machine, the decoded local `path`. A call with no usable URI is refused. Another process can take the name over (the app allows replacement); `onFileManagerOwnership` then reports `owned: false` (only for the ownership that lost the name, so giving the name back and taking it again is not undone by a late loss). `ownFileManager` rejects with `conflict` when another process holds the name and will not give it up, and `disownFileManager()` gives it back. Ship a D-Bus activation file for the name if the app should be started by other applications' calls.
- **Windows global shortcuts:** `registerGlobalShortcut({ id, accelerator })` registers `RegisterHotKey` on a message-only window thread (an accelerator such as `Ctrl+Alt+K` with at least one modifier) and `onShortcutPressed` reports `{ id }`; `unregisterGlobalShortcut(id)` removes it. Registering an id again replaces its accelerator (the same accelerator included); a combination another application holds rejects with `conflict`, and the earlier binding of that id then stays. A call that rejects with `timeout` did not take effect and can be retried. On Linux use `registerShortcut`.

```typescript
import { desktopIntegration, isFeatureAvailable } from '@liminal-hq/plugin-desktop-integration';

const status = await desktopIntegration.getStatus();
if (isFeatureAvailable(status, 'launcherProgress')) {
	await desktopIntegration.setLauncherProgress({
		progress: { state: 'value', value: 0.4 },
		count: 2,
		desktopId: null,
		windowLabel: null,
	});
}
```

From Rust, `app.desktop_services()` (the `DesktopServicesExt` trait) offers the same calls as async methods: `status`, `notify`, `withdraw_notification`, `inhibit_sleep`, `release_sleep_inhibit`, `set_launcher_progress`, `own_file_manager`, `disown_file_manager`, `register_global_shortcut`, `unregister_global_shortcut` and `set_app_user_model_id`. Events are emitted to every window, and Rust code can `listen` to them too.

### Generated types

`ShortcutBindingResult`, `ShortcutActivatedPayload`, and `ShortcutChangedPayload` (the
payloads of the `shortcut-binding-result`, `shortcut-activated`, and `shortcut-changed`
events) are generated from their Rust definitions via
[`ts-rs`](https://github.com/Aleph-Alpha/ts-rs) into `guest-js/bindings/` and re-exported
from the package root, so the JS/Rust shapes can't drift:

```typescript
import type {
	ShortcutActivatedPayload,
	ShortcutBindingResult,
	ShortcutChangedPayload,
} from '@liminal-hq/plugin-desktop-integration';
```

The bindings regenerate automatically as part of `cargo test` (each type's `#[ts(export)]`
attribute creates a test that writes its `.ts` file) — run `cargo test -p
tauri-plugin-desktop-integration` after changing either struct and commit the result.

## Permissions

This plugin requires these permissions:

- `allow-register-shortcut`: Grants access to `register_shortcut`
- `allow-check-shortcut-binding-complete`: Grants access to `check_shortcut_binding_complete`
- `allow-check-shortcut-binding-error`: Grants access to `check_shortcut_binding_error`
- `allow-check-shortcut-trigger-description`: Grants access to `check_shortcut_trigger_description`
- `allow-get-status`: Grants access to `get_status`
- `allow-notify`: Grants access to `notify`
- `allow-withdraw-notification`: Grants access to `withdraw_notification`
- `allow-set-app-user-model-id`: Grants access to `set_app_user_model_id`
- `allow-inhibit-sleep`: Grants access to `inhibit_sleep`
- `allow-release-sleep-inhibit`: Grants access to `release_sleep_inhibit`
- `allow-set-launcher-progress`: Grants access to `set_launcher_progress`
- `allow-own-file-manager`: Grants access to `own_file_manager`
- `allow-disown-file-manager`: Grants access to `disown_file_manager`
- `allow-register-global-shortcut`: Grants access to `register_global_shortcut`
- `allow-unregister-global-shortcut`: Grants access to `unregister_global_shortcut`

The `default` set grants the four shortcut commands and the read-only `get_status`; grant the rest explicitly.

## Platform Support

| Platform | Support Level | Notes                                                                                                                                                              |
| -------- | ------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------ |
| Windows  | Partial       | Toasts (with an AppUserModelID), sleep inhibit, taskbar progress and `RegisterHotKey` shortcuts; type-checked for `x86_64-pc-windows-msvc`, not yet run on Windows |
| Linux    | Full          | X11 activation via `gdkx11`, Wayland shortcuts via the portal, notifications, logind sleep inhibit, launcher progress and `FileManager1` over zbus                 |
| macOS    | None          | Every service reports `platform-unsupported`                                                                                                                       |
| Android  | None          | Every service reports `platform-unsupported`                                                                                                                       |
| iOS      | None          | Every service reports `platform-unsupported`                                                                                                                       |

## Testing

`cargo test -p tauri-plugin-desktop-integration` runs the headless tests (message and payload construction, id and handle bookkeeping, `FileManager1` URI parsing, accelerator parsing, status mapping). The `#[ignore]`d `live_*` tests touch the real session and are run by hand on a Linux desktop: `live_status` (read-only), `live_inhibit` (holds a logind inhibitor for 8 seconds; `systemd-inhibit --list` shows it), `live_launcher_entry` (emits three `Update` signals that `dbus-monitor "interface=com.canonical.Unity.LauncherEntry"` shows), `live_file_manager` and `live_file_manager_name_lost_and_taken` (briefly own `org.freedesktop.FileManager1`; run with `--test-threads=1`) and `live_notify` (shows one real notification).

## Licence

Apache-2.0 OR MIT
