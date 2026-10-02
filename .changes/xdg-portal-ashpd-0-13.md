---
'xdg-portal': patch
---

Updates the `ashpd` dependency to 0.13, enabling only the portal features the plugin uses, so the workspace builds one `ashpd` and one `zbus`.
The `GlobalShortcuts` calls take the new session and bind option types, and `is_sandboxed` is now synchronous.
