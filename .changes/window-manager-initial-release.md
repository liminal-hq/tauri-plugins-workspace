---
'window-manager': minor
'window-manager-js': minor
---

Add the `window-manager` plugin, which asks the compositor to show its own window menu, reports which window manager features are available, and reads the window manager's real Always on Top state (on X11 it also reports changes made from the window manager's own menu). The system window menu works on Linux (verified by hand on GNOME/Wayland; the X11 Always on Top read and change event are verified under XWayland; KDE and native X11 are untested). Windows shows its system menu and macOS pops up the app's Window menu; both are type-checked against their targets but have not been run on those systems, and log each native step so a crash names it.
