---
'system-appearance': minor
'system-appearance-js': minor
---

Adds the colour palette to `system-appearance`: `get_palette` (`getPalette()` in JavaScript) reports twelve colours (the window and view backgrounds and text, the raised surface, the selection and its text, the border, the focus colour and the warning, error and success colours) with a `revision` and, for each, the source that supplied it or a typed reason it is unavailable, and `system-appearance://palette-changed` (`onPaletteChanged()`) pushes changes as they happen. On Linux it reads `kdeglobals` on KDE (watched for changes) and the GTK theme's named colours everywhere else, with the portal's accent colour filling a selection and focus the theme lacks, which adds a `gtk` dependency. On Windows it reads `UISettings` and, in a high-contrast theme, `GetSysColor`, and follows their change events. macOS and unsupported platforms report every colour unavailable. `getStatus()` gains a `palette` status that says whether the palette can be read and why not, and the `default` permission set gains `allow-get-palette`.
