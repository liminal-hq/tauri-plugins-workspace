---
'steamworks': minor
'steamworks-js': minor
---

Adds DLC checks (`getDlc`, `isDlcInstalled`), device detection (`getDeviceInfo`, `isSteamDeck`) and Steam screenshot integration (`hookScreenshots`, `triggerScreenshot`, `addScreenshotToLibrary` and a `screenshot-requested` event) to the `steamworks` plugin. Each group sits behind its own opt-in permission set (`allow-dlc`, `allow-device`, `allow-screenshots`). Adding a screenshot accepts only an absolute path to an existing image file.
