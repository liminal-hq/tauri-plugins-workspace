# Change Files

This directory contains change files for Covector-based version management.

## Creating a change file

When you make changes to a plugin, create a change file:

```bash
pnpm covector change
```

Or manually create a file in this directory with this format:

```markdown
---
"plugin-name": patch
"plugin-name-js": patch
---

Brief description of the change.
```

## Version types

- `major`: Breaking changes (bumps X.0.0)
- `minor`: New features, backward compatible (bumps 0.X.0)
- `patch`: Bug fixes, backward compatible (bumps 0.0.X)

## Best practices

- Keep Rust and JS versions synchronised.
- One change file per logical change.
- Clear, user-facing descriptions.
- Reference issue numbers when applicable.
