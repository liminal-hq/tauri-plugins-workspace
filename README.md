# Liminal Tauri Plugins

A collection of Tauri v2 plugins for building privacy-focused, local-first applications.

## Plugins

| Plugin                                               | Description                                                                                                                                            | Platforms                                 |
| ---------------------------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------ | ----------------------------------------- |
| [`material-you`](plugins/material-you)               | Material You theming support                                                                                                                           | Android                                   |
| [`phone-haptics`](plugins/phone-haptics)             | Portable haptic patterns with capability reporting and a stepped fallback ladder                                                                       | Android                                   |
| [`xdg-portal`](plugins/xdg-portal)                   | `xdg-desktop-portal` theming and global shortcuts                                                                                                      | Linux                                     |
| [`desktop-integration`](plugins/desktop-integration) | X11 window activation and unified global-shortcut binding                                                                                              | Linux                                     |
| [`system-appearance`](plugins/system-appearance)     | Window titlebar layout and actions, and the appearance preferences (colour scheme, accent, contrast, motion, text scale, icon theme), pushed on change | Linux, Windows, macOS                     |
| [`os-prefs`](plugins/os-prefs)                       | The user's 12/24-hour clock setting, pushed on change                                                                                                  | Linux, Windows, macOS, Android, iOS       |
| [`gamepad-haptics`](plugins/gamepad-haptics)         | Rumble for gamepads from one pattern language, plus a web fallback when no native path exists                                                          | Linux (native), web fallback elsewhere    |
| [`hdmv`](plugins/hdmv)                               | HDMV/Blu-ray menu engine access through libhdmv (not published; install from Git)                                                                      | Windows, Linux, macOS                     |
| [`window-manager`](plugins/window-manager)           | Compositor window menu and window manager features                                                                                                     | Linux, Windows (partial), macOS (partial) |

## Installation

Every plugin is published twice, as a Rust crate `tauri-plugin-<name>` on crates.io and as an npm package `@liminal-hq/plugin-<name>`, except `hdmv`, which needs libhdmv and is installed from Git. The plugin's own README lists its current version, platforms and permissions.

### From Registry (stable releases)

**Rust (`Cargo.toml`):**

```toml
[dependencies]
tauri-plugin-phone-haptics = "0.1"
```

**JavaScript (`package.json`):**

```json
{
	"dependencies": {
		"@liminal-hq/plugin-phone-haptics": "^0.1.0"
	}
}
```

Pre-1.0 versions treat a minor bump as a breaking change, so `"0.1"` stays on 0.1.x. Use each plugin's current minor version.

### From Git (development)

Each release is tagged `<plugin>-v<version>` for the crate and `<plugin>-js-v<version>` for the npm package.

**Rust (`Cargo.toml`):**

```toml
[dependencies]
tauri-plugin-phone-haptics = { git = "https://github.com/liminal-hq/tauri-plugins-workspace", tag = "phone-haptics-v0.1.0" }
```

**JavaScript (`package.json`):**

```json
{
	"dependencies": {
		"@liminal-hq/plugin-phone-haptics": "github:liminal-hq/tauri-plugins-workspace#phone-haptics-js-v0.1.0&path:plugins/phone-haptics"
	}
}
```

## Development

### Prerequisites

- Rust 1.93.0+
- Node.js 24.14.0+
- pnpm 10+
- Android NDK r28 (for Android plugins)

### Setup

```bash
# Clone the repository
git clone https://github.com/liminal-hq/tauri-plugins-workspace.git
cd tauri-plugins-workspace

# Install dependencies
pnpm install

# Install Git hooks
pnpm hooks:install

# Build all plugins
pnpm build

# Run tests
cargo test --workspace
```

## Development Environment

### Using VS Code Devcontainer (recommended)

1. Install [Docker](https://www.docker.com/products/docker-desktop)
2. Install [VS Code](https://code.visualstudio.com/) and the [Dev Containers extension](https://marketplace.visualstudio.com/items?itemName=ms-vscode-remote.remote-containers)
3. Open this repository in VS Code
4. Choose **Reopen in Container** when prompted for the default mobile image
5. If you only need desktop workflows, choose the explicit desktop profile at `.devcontainer/desktop/devcontainer.json`
6. Wait for the container setup to complete

The devcontainer profiles use shared Liminal HQ GHCR images:

- Mobile default: `ghcr.io/liminal-hq/tauri-dev-mobile:latest`
- Desktop profile: `ghcr.io/liminal-hq/tauri-dev-desktop:latest`

The devcontainer includes:

- Shared Liminal HQ Tauri tooling images
- Rust stable with Android targets in the mobile profile
- Node.js 24 with pnpm
- Android SDK with NDK r28
- Tauri system dependencies
- VS Code extensions for Rust and TypeScript

Profile guidance:

- Default mobile profile: use for Android plugin work, mobile validation, and cross-platform maintenance
- Desktop profile: use for faster desktop-only editing, linting, and Rust and JavaScript quality checks

The checked-in `.devcontainer/Dockerfile` remains as local reference material for the toolchain shape, but this repository no longer publishes a repo-specific devcontainer image.

### Manual setup

If you are not using devcontainers, install:

- Rust 1.93.0+ with clippy and rustfmt
- Node.js 24.14.0+ with pnpm
- Android NDK r28 (for Android plugins)
- Tauri system dependencies

## Philosophy

These plugins follow Liminal HQ principles:

- **Privacy-first**: No unnecessary off-device data transfer
- **Local-first**: Core functionality works offline
- **User agency**: Users control experience and data
- **Calm computing**: Thoughtful, non-intrusive interactions

## Licence

Licensed under either of:

- Apache Licence, Version 2.0 (`LICENSE-APACHE`)
- MIT Licence (`LICENSE-MIT`)

at your option.

## Contributing

Contributions are welcome. See `CONTRIBUTING.md`.
