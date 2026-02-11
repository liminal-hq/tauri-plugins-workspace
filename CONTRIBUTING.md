# Contributing to Liminal Tauri Plugins

Thank you for contributing. This document describes the expected workflow.

## Code of Conduct

- Be respectful and inclusive.
- Focus on constructive feedback.
- Help keep the project welcoming and practical.

## Development Workflow

### 1. Fork and clone

```bash
git clone https://github.com/liminal-hq/tauri-plugins-workspace.git
cd tauri-plugins-workspace
pnpm install
```

### 2. Create a branch

```bash
git checkout -b feature/your-feature-name
# or
git switch -c feature/your-feature-name
```

### 3. Make changes

- Follow existing style and project conventions.
- Add or update tests where behaviour changes.
- Update documentation for user-visible changes.

### 4. Run checks

```bash
# Rust checks
cargo fmt --all
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace

# JavaScript checks
pnpm lint
pnpm format:check
pnpm build
```

### 5. Create a change file

```bash
pnpm covector change
```

### 6. Commit

- Use Conventional Commits.
- Include a markdown body with:
  - `**What changed**`
  - `**Why**`
  - `**Scope**`

### 7. Open a pull request

Open a PR with a clear summary, impact notes, and any migration details.

## Plugin Development Guidelines

### Rust

- Use `cargo fmt`.
- Follow Rust API guidelines.
- Use `thiserror` for public error types where appropriate.
- Document public APIs with rustdoc.

### TypeScript

- Use Prettier and ESLint.
- Export type definitions for public API surfaces.
- Document public APIs with TSDoc where useful.

### Android

- Follow Kotlin conventions.
- Keep Android permissions plugin-owned via manifest injection.
- Test behaviour on supported Android versions.

### Platform support metadata

Each plugin should declare platform support in `Cargo.toml`:

```toml
[package.metadata.platforms.support]
windows = { level = "full", notes = "" }
linux = { level = "full", notes = "" }
macos = { level = "full", notes = "" }
android = { level = "full", notes = "" }
ios = { level = "none", notes = "Not implemented" }
```

Support levels: `full`, `partial`, `none`.

## Release Process

Releases are managed with Covector:

1. Add change files as you work.
2. Maintainers run `covector version`.
3. Maintainers run `covector publish`.

## Questions

- Open an issue for defects or feature requests.
- Open a discussion for broader design topics.
