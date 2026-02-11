# @liminal-hq/plugin-material-you

Access Material You colour palette data from Android system resources.

## Package Links

- crates.io: https://crates.io/crates/tauri-plugin-material-you
- npm: https://www.npmjs.com/package/@liminal-hq/plugin-material-you
- Report bugs: https://github.com/liminal-hq/tauri-plugins-workspace/issues

## Installation

### Rust

```toml
[dependencies]
tauri-plugin-material-you = "0.1"

# Alternatively with Git:
tauri-plugin-material-you = { git = "https://github.com/liminal-hq/tauri-plugins-workspace", branch = "main" }
```

### JavaScript

```bash
pnpm add @liminal-hq/plugin-material-you
# or
npm add @liminal-hq/plugin-material-you
# or
yarn add @liminal-hq/plugin-material-you
```

## Usage

### Rust

```rust
fn main() {
    tauri::Builder::default()
        .plugin(tauri_plugin_material_you::init())
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
```

### JavaScript

```ts
import { getMaterialYouColours } from '@liminal-hq/plugin-material-you';

const materialYou = await getMaterialYouColours();
console.log(materialYou.supported, materialYou.palettes);
```

## Permissions

This plugin exposes one command via the Tauri permissions system:

- `get_material_you_colours`

## Platform Support

| Platform | Support Level | Notes                         |
| -------- | ------------- | ----------------------------- |
| Windows  | None          | Android-only plugin           |
| Linux    | None          | Android-only plugin           |
| macOS    | None          | Android-only plugin           |
| Android  | Full          | Returns Material You palettes |
| iOS      | None          | Not applicable (Android-only) |

## Licence

Apache-2.0 OR MIT
