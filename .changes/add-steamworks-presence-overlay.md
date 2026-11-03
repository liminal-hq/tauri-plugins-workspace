---
'steamworks': minor
'steamworks-js': minor
---

Adds rich presence (`setRichPresence`, `clearRichPresence`) and overlay commands (`activateOverlay`, `activateOverlayToWebPage`) to the `steamworks` plugin, with an `overlay-toggled` event and a Rust `on_overlay_toggled` hook for forwarding toggles to an overlay-surface plugin. Both groups are opt-in through the new `allow-presence` and `allow-overlay` permission sets and are not part of the default permissions. Presence entries are checked against Steam's limits before they reach Steam, and the overlay web page command accepts only `http` and `https` addresses.
