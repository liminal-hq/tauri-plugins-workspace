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
- **Breaking (desktop-integration):** `DesktopIntegrationExt` gains a new required
  trait method, `last_shortcut_trigger_description`, with no default implementation
  (matching every other method on this trait). Any crate implementing
  `DesktopIntegrationExt` for its own type — rather than relying on this crate's own
  `AppHandle<R>` implementation — must add it to keep compiling. The rest of the
  change (the `shortcut-changed` event, `ShortcutChangedPayload`, and the
  `check_shortcut_trigger_description` command/permission) is purely additive.
