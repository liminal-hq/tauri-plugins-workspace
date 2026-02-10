# @liminal-hq/plugin-theme-utils

Access Material You colour palette data from Android system resources.

## Installation

### Rust

```toml
[dependencies]
tauri-plugin-theme-utils = "0.1"
```

### JavaScript

```bash
pnpm add @liminal-hq/plugin-theme-utils
```

## Usage

### Rust

```rust
fn main() {
    tauri::Builder::default()
        .plugin(tauri_plugin_theme_utils::init())
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
```

### JavaScript

```ts
import { getMaterialYouColours } from '@liminal-hq/plugin-theme-utils';

const materialYou = await getMaterialYouColours();
console.log(materialYou.supported, materialYou.palettes);
```

## Permissions

This plugin exposes one command via the Tauri permissions system:

- `get_material_you_colours`

## Platform Support

| Platform | Support Level | Notes |
|----------|---------------|-------|
| Windows | Partial | Returns `supported: false` |
| Linux | Partial | Returns `supported: false` |
| macOS | Partial | Returns `supported: false` |
| Android | Full | Returns Material You palettes |
| iOS | None | Not implemented |

## Licence

Apache-2.0 OR MIT
