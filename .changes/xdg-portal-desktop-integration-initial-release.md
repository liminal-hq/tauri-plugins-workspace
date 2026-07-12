---
'xdg-portal': minor
'xdg-portal-js': minor
'desktop-integration': minor
'desktop-integration-js': minor
---

Release notes for the new `xdg-portal` and `desktop-integration` plugins, promoted from `liminal-hq/emoji-nook`:

- `xdg-portal` bridges Tauri apps to the Linux `xdg-desktop-portal` D-Bus interfaces: desktop theme detection (colour scheme, accent colour, high contrast) via the Settings portal, and Wayland global shortcut binding via the GlobalShortcuts portal.
- `desktop-integration` provides X11 window activation (`_NET_WM_USER_TIME` stamping via `gdkx11`) and a unified `DesktopIntegrationExt` trait that picks X11 direct shortcut binding or the Wayland portal automatically based on session type.
- Both are Linux-only; see each plugin's `[package.metadata.platforms.support]` and README for details.
