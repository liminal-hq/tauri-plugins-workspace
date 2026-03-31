# @liminal-hq/plugin-hdmv

Tauri v2 plugin wrapping [libhdmv](https://github.com/liminal-hq/libhdmv) — a Rust-native HDMV/Blu-ray menu engine for disc inspection, menu navigation, rendering, and authoring.

## Installation

### Rust

```toml
[dependencies]
tauri-plugin-hdmv = { git = "https://github.com/liminal-hq/tauri-plugins-workspace", branch = "main" }
```

### JavaScript

```bash
pnpm add @liminal-hq/plugin-hdmv
```

## Usage

### Rust

```rust
fn main() {
    tauri::Builder::default()
        .plugin(tauri_plugin_hdmv::init())
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
```

### JavaScript

```typescript
import { openDisc, getDiscInfo, startNavigation, closeDisc } from '@liminal-hq/plugin-hdmv';

const sessionId = await openDisc('/path/to/BDMV');
const info = await getDiscInfo(sessionId);
console.log(`Disc has ${info.titleCount} titles`);

const events = await startNavigation(sessionId);
console.log('Navigation events:', events);

await closeDisc(sessionId);
```

## Commands

### Session Lifecycle

- `openDisc(path)` — Open a BDMV folder, returns session ID
- `closeDisc(sessionId)` — Close session and free resources

### Disc Inspection

- `getDiscInfo(sessionId)` — Title count, version, first play/top menu info
- `listTitles(sessionId)` — Title entries from index
- `listPlaylists(sessionId)` — Playlist summaries
- `getPlaylist(sessionId, index)` — Full playlist with streams and chapters

### Navigation

- `startNavigation(sessionId)` — Execute First Play object
- `sendKey(sessionId, key)` — Send remote key input
- `mouseMove(sessionId, x, y)` — Hit-test, returns button ID
- `mouseClick(sessionId, x, y)` — Click and process activation

### Rendering

- `renderPreview(sessionId, maxWidth)` — Render current menu as base64 PNG
- `getMenuState(sessionId)` — Current page, focused button, states

### Authoring

- `buildDisc(config)` — Create a BDMV structure from configuration

## Permissions

All commands have individual permissions. The default permission set grants access to all commands. See `permissions/default.toml`.

## Platform Support

| Platform | Support Level | Notes        |
| -------- | ------------- | ------------ |
| Windows  | Full          |              |
| Linux    | Full          |              |
| macOS    | Full          |              |
| Android  | None          | Desktop only |
| iOS      | None          | Desktop only |

## Licence

Apache-2.0 OR MIT
