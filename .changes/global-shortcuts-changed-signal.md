---
'xdg-portal': minor
'xdg-portal-js': minor
'desktop-integration': minor
'desktop-integration-js': minor
---

`create_session` (xdg-portal) and `DesktopIntegrationExt` (desktop-integration) now
surface the GlobalShortcuts portal's `ShortcutsChanged` signal, which fires when the
user rebinds a shortcut's trigger through the compositor's own settings UI (e.g. GNOME
Settings → Apps → <App> → Global Shortcuts) instead of through the app.

- **Breaking (xdg-portal):** `create_session()` gains a new required
  `on_shortcuts_changed` callback parameter, inserted before `window_rx`. Any direct
  caller must update its call site.
- **Non-breaking (desktop-integration):** adds the `shortcut-changed` event, the
  `ShortcutChangedPayload` type, `DesktopIntegrationExt::last_shortcut_trigger_description`,
  and the `check_shortcut_trigger_description` command/permission as an additive race
  guard.
