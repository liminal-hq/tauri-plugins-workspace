# @liminal-hq/plugin-xdg-portal

Bridges Tauri apps to the Linux `xdg-desktop-portal` D-Bus interfaces, so sandboxed
and Wayland apps can request system integration (theming, global shortcuts) through
the standard freedesktop.org portal APIs instead of platform-specific hacks.

## Installation

### Rust

```toml
[dependencies]
tauri-plugin-xdg-portal = "0.2"

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
    window_id_receiver,
)
.await?;
```

On X11, prefer `tauri-plugin-global-shortcut` directly — the portal path is Wayland-specific.
See [`@liminal-hq/plugin-desktop-integration`](../desktop-integration) for a helper that
picks the right path automatically based on session type.

## Permissions

This plugin requires these permissions:

- `allow-check-availability`: Grants access to `check_availability`
- `allow-get-theme-info`: Grants access to `get_theme_info`

## Platform Support

| Platform | Support Level | Notes                                           |
| -------- | ------------- | ----------------------------------------------- |
| Windows  | None          | `xdg-desktop-portal` is Linux-only              |
| Linux    | Full          | Bridges Settings and GlobalShortcuts interfaces |
| macOS    | None          | `xdg-desktop-portal` is Linux-only              |
| Android  | None          | `xdg-desktop-portal` is Linux-only              |
| iOS      | None          | `xdg-desktop-portal` is Linux-only              |

## Licence

Apache-2.0 OR MIT
