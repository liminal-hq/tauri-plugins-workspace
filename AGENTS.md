# AGENTS.md

## Table of Contents

- [Localisation and Spelling](#localisation-and-spelling)
- [Commit Messages](#commit-messages)
- [Git Workflow](#git-workflow)
- [Repository Scope](#repository-scope)
- [Code Organisation](#code-organisation)
- [Best Practices](#best-practices)
- [Plugin Development](#plugin-development)
- [Release and Versioning](#release-and-versioning)
- [Tauri v2](#tauri-v2)

## Localisation and Spelling

**REQUIREMENT:** All UI strings, code comments, commit messages, pull request descriptions, and documentation MUST use **Canadian English** spelling.

Examples:

- `colour` instead of `color`
- `centre` instead of `center`
- `neighbour` instead of `neighbor`
- `cancelled` instead of `canceled`
- `licence` (noun) vs `license` (verb)

## Commit Messages

**Format:** Use Conventional Commits (for example: `feat: ...`, `fix: ...`, `docs: ...`, `chore: ...`, `test: ...`).

- Use `test:` for test-only changes, including test fixes.
- Use detailed markdown bodies with **bold labels**.
- Do not use markdown headings in commit message bodies.

**Body Requirements:**

- Explain what changed and why.
- Keep scope specific to the current commit.
- Prefer this structure:
  - `**What changed**`
  - `**Why**`
  - `**Scope**`

**Shell Interpolation Safety:**

- Do not pass markdown-heavy commit bodies directly via `git commit -m "..."` when they contain shell-sensitive characters.
- Prefer a single-quoted heredoc and `git commit -F <file>`.
- Verify the stored message after committing with `git log -1 --pretty=fuller`.

## Git Workflow

**REQUIREMENT:** Do not push changes (especially force pushes) unless explicitly requested by the user.

## Repository Scope

This repository hosts reusable Tauri v2 plugins and workspace infrastructure.

- Keep repository rules plugins-focused.
- Do not add Threshold app-specific assumptions to shared plugin guidance.
- Treat `setup-docs/` as local reference material during setup; do not commit it.

## Code Organisation

This is a `pnpm` workspace monorepo.

- `plugins/` contains custom Tauri plugins.
- `shared/` contains shared build/tooling helpers.
- `examples/` contains sample applications for integration verification.
- `docs/` contains repository-wide documentation and guides.
- `.changes/` contains Covector change files and configuration.

## Best Practices

- **NO BARREL FILES:** Do not use `index.ts` re-export barrels.
- **DIRECT IMPORTS:** Import directly from concrete module paths.
- **REUSE HELPERS:** Check `shared/` and plugin-local helpers before adding duplicate logic.
- **API STABILITY:** Keep Rust command names and guest JS APIs stable once released.

## Plugin Development

When creating or modifying plugins with Android support:

**Android Manifest Injection (Required):**

- Plugins MUST own Android permissions via build-time injection.
- Use `tauri_plugin::mobile::update_android_manifest()` in `build.rs`.
- Use block identifiers in this form: `tauri-plugin-{plugin-name}.permissions`.
- Inject permissions in build scripts and keep Android components in plugin manifests.
- Never require consuming applications to manually edit Android manifests for plugin permissions.

**Reference Pattern:**

```rust
const COMMANDS: &[&str] = &["command_1", "command_2"];

fn main() {
    tauri_plugin::Builder::new(COMMANDS).build();
    inject_android_permissions().expect("Failed to inject Android permissions");
}

fn inject_android_permissions() -> std::io::Result<()> {
    tauri_plugin::mobile::update_android_manifest(
        "tauri-plugin-example.permissions",
        "manifest",
        r#"<uses-permission android:name=\"android.permission.CAMERA\" />"#,
    )
}
```

**Per-plugin expectations:**

- Provide Rust crate metadata, permissions, and platform support notes.
- Provide guest JS bindings and package metadata.
- Include a plugin README with installation, usage, permissions, and platform support.
- Keep examples buildable for integration testing.

## Release and Versioning

- Use Covector change files for releasable changes.
- Keep Rust and JavaScript package versions aligned per plugin.
- Use semantic versioning and call out breaking changes clearly.
- Ensure CI passes (`fmt`, `clippy`, tests, lint, format checks, build) before release work.

## Tauri v2

This repository uses **Tauri v2**.

### Platform Detection

Use `@tauri-apps/plugin-os` for cross-platform detection when needed.

- `platform()` is synchronous and compile-time determined.
- Handle mobile (`ios`, `android`) and desktop targets explicitly.

### API and Plugin Usage

- Prefer Tauri plugins over web APIs when available.
- Most Tauri APIs are async; use `async/await`.
- Avoid Tauri v1 patterns and modules that moved to `@tauri-apps/plugin-*`.

### Permissions and Capabilities

- Tauri v1 `allowlist` is replaced by capabilities.
- Capabilities must explicitly grant plugin permissions.
- Installing a plugin alone is not enough; permissions must also be configured.
