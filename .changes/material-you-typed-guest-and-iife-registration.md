---
'material-you': patch
'material-you-js': patch
---

Release updates for `material-you` and `material-you-js`:

- Adds concrete guest TypeScript response types for `getMaterialYouColours`.
- Updates the guest API to return `Promise<MaterialYouResponse>` for stronger IDE and compile-time feedback.
- Aligns IIFE build output with guarded Tauri global registration behaviour.
