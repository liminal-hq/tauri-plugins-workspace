---
'share-sheet': minor
'share-sheet-js': minor
---

Adds the `share-sheet` plugin, which hands a file or text to the operating system's share sheet with `shareFile`, `shareText` and `canShare`. Android (through the plugin's own `FileProvider`, with no permissions), iOS, macOS and Windows are implemented but not yet run on a device; Linux has no standard share UI, so every share rejects with `unsupported`.
