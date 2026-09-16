---
'material-you': patch
'material-you-js': patch
---

Fixes `material-you` so it registers its command on every platform instead of Android only. Previously, calling `getMaterialYouColours()` from a desktop (or iOS) app threw a "command not found" error rather than resolving gracefully; it now returns `{ supported: false }` outside Android, matching the plugin's own documented platform-support metadata.
