# Liminal Tauri Plugins

A collection of Tauri v2 plugins for building privacy-focused, local-first applications.

## Plugins

| Plugin                  | Description                                       | Platforms        |
| ----------------------- | ------------------------------------------------- | ---------------- |
| `alarm-manager`         | Native alarm scheduling with Android AlarmManager | Android          |
| `time-prefs`            | System time format detection                      | Desktop, Android |
| `material-you`          | Material You theming support                      | Android          |
| `mobile-app-management` | Mobile app lifecycle management                   | Android, iOS     |

## Installation

### From Registry (stable releases)

**Rust (`Cargo.toml`):**

```toml
[dependencies]
tauri-plugin-alarm-manager = "0.1.0"
```

**JavaScript (`package.json`):**

```json
{
	"dependencies": {
		"@liminal-hq/plugin-alarm-manager": "^0.1.0"
	}
}
```

### From Git (development)

**Rust (`Cargo.toml`):**

```toml
[dependencies]
tauri-plugin-alarm-manager = { git = "https://github.com/liminal-hq/liminal-tauri-plugins", tag = "alarm-manager-v0.1.0" }
```

**JavaScript (`package.json`):**

```json
{
	"dependencies": {
		"@liminal-hq/plugin-alarm-manager": "github:liminal-hq/liminal-tauri-plugins#alarm-manager-v0.1.0&path:plugins/alarm-manager"
	}
}
```

## Development

### Prerequisites

- Rust 1.93.0+
- Node.js 22.21.1+
- pnpm 10+
- Android NDK r28 (for Android plugins)

### Setup

```bash
# Clone the repository
git clone https://github.com/liminal-hq/liminal-tauri-plugins.git
cd liminal-tauri-plugins

# Install dependencies
pnpm install

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
4. Choose **Reopen in Container** when prompted
5. Wait for the first build to complete

The devcontainer includes:

- Rust stable with Android targets
- Node.js 22 with pnpm
- Android SDK with NDK r28
- Tauri system dependencies
- VS Code extensions for Rust and TypeScript

### Manual setup

If you are not using devcontainers, install:

- Rust 1.93.0+ with clippy and rustfmt
- Node.js 22.21.1+ with pnpm
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
