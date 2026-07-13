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
tauri-plugin-desktop-integration = "0.1"

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

### Generated types

`ShortcutBindingResult` and `ShortcutActivatedPayload` (the payloads of the
`shortcut-binding-result` and `shortcut-activated` events) are generated from their Rust
definitions via [`ts-rs`](https://github.com/Aleph-Alpha/ts-rs) into
`guest-js/bindings/` and re-exported from the package root, so the JS/Rust shapes can't
drift:

```typescript
import type {
	ShortcutActivatedPayload,
	ShortcutBindingResult,
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

## Platform Support

| Platform | Support Level | Notes                                                         |
| -------- | ------------- | ------------------------------------------------------------- |
| Windows  | None          | X11/Wayland activation helpers only                           |
| Linux    | Full          | X11 activation via `gdkx11`, Wayland shortcuts via the portal |
| macOS    | None          | X11/Wayland activation helpers only                           |
| Android  | None          | X11/Wayland activation helpers only                           |
| iOS      | None          | X11/Wayland activation helpers only                           |

## Licence

Apache-2.0 OR MIT
