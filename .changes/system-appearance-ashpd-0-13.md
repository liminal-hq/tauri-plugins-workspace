---
'system-appearance': patch
---

Updates the `ashpd` dependency to 0.13, enabling only the portal features the plugin uses, so the workspace builds one `ashpd` and one `zbus`.
The `Settings` signal watcher builds the proxy first and keeps it alive for as long as it listens, because the `SettingChanged` stream now borrows it.
