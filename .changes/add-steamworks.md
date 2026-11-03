---
'steamworks': minor
'steamworks-js': minor
---

Adds the `steamworks` plugin, a Steamworks integration for Tauri games. The first release covers player identity, achievements (unlock, progress and, behind its own permission, clearing) and stats, with typed events for stats received, stats stored and achievement stored. Linking the Steamworks SDK is opt-in through the `steam` cargo feature, so builds outside Steam never carry it. When Steam is missing or fails to start, the plugin reports a queryable unavailable status with a reason instead of failing, and Android and iOS builds always report unavailable. The `mock` cargo feature backs the same commands with an in-memory fake for tests. The Rust crate is not published to crates.io because the name is taken there; use it from Git or by path.
