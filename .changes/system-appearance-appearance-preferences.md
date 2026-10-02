---
'system-appearance': minor
'system-appearance-js': minor
---

Adds the appearance preferences to `system-appearance`: `get_appearance` (`getAppearance()` in JavaScript) reports the colour scheme, accent colour, contrast, reduced motion, reduced transparency, text scale and icon theme with a `revision` and the source that supplied each value, and `system-appearance://appearance-changed` (`onAppearanceChanged()`) pushes changes as they happen. On Linux it reads the `org.freedesktop.appearance` portal settings and the GNOME keys the portal passes through, falling back to `gsettings`, to KDE's `kdeglobals` (watched for changes) and to Cinnamon's schemas. On Windows it reads `UISettings`, `AccessibilitySettings` and the light/dark registry value, and follows their change events. macOS and unsupported platforms report every feature unavailable. `getStatus()` gains `appearanceAvailable` and an `appearance` list that reports each feature's availability with a typed reason; `available`, `reason` and `features` keep describing the titlebar preferences only.
