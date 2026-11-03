---
'steamworks': minor
'steamworks-js': minor
---

Adds Steam lobbies, Workshop subscriptions and Steam Input to the `steamworks` plugin. Lobbies cover create, join, list, read, data and joinable state with `lobby-chat-update` and `lobby-data-update` events. Workshop covers listing, subscribing, unsubscribing and downloading items. Steam Input covers controllers, action sets, digital and analog action state, and button glyphs returned as PNG bytes. Each group sits behind its own opt-in permission set (`allow-lobbies`, `allow-workshop`, `allow-input`). Steam IDs and handles cross the IPC boundary as decimal strings. The `MockConfig` struct gains `workshop` and `controllers` fields, and the mock can simulate lobby members, Workshop installs and input.
