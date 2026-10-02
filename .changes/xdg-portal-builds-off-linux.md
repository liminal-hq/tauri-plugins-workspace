---
'xdg-portal': patch
---

Makes `ashpd` a Linux-only dependency so the crate builds on Windows and macOS, where every portal feature reports an unsupported platform. The `global_shortcuts` module keeps its API on those platforms, with `create_session` always failing with `UnsupportedPlatform`.
