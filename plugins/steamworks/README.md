# Tauri Plugin Steamworks

Steamworks for Tauri v2 games: player identity, achievements, stats, rich presence, the overlay, Steam Cloud files, DLC checks, Steam Deck detection, screenshots, lobbies, Workshop subscriptions and Steam Input. One binary can ship inside and outside Steam, because the plugin reports a queryable _unavailable_ state instead of failing when Steam is missing.

## Highlights

- **Never fatal.** If Steam is not running, the SDK is not linked or the platform is mobile, setup still succeeds and every command rejects with a stable `unavailable: <reason>` message. `getStatus()` and `isAvailable()` never reject.
- **Opt-in SDK.** The Steamworks SDK is linked only when the `steam` cargo feature is on, so non-Steam builds never carry it.
- **The plugin owns the callback pump.** A plugin thread runs Steam callbacks every 100 ms (configurable). Consumers never call `run_callbacks`.
- **Testable without Steam.** The `mock` cargo feature backs the same commands with an in-memory fake.
- **Typed guest API.** Wrappers and event helpers, so nobody hand-writes an `invoke` string.

## Package links

- Rust crate: `tauri-plugin-steamworks` (not published to crates.io, because the name is taken there; use Git or a path)
- npm package: `@liminal-hq/plugin-steamworks`

## Installation

**Rust (`Cargo.toml`):**

```toml
[dependencies]
tauri-plugin-steamworks = { git = "https://github.com/liminal-hq/tauri-plugins-workspace" }

[features]
# Enable this from your Steam distribution channel only.
steam = ["tauri-plugin-steamworks/steam"]
```

**JavaScript:**

```sh
pnpm add @liminal-hq/plugin-steamworks
```

**Register the plugin (`src-tauri/src/lib.rs`):**

```rust
tauri::Builder::default()
    .plugin(
        tauri_plugin_steamworks::Builder::new(480) // your Steam AppId
            .build(),
    )
```

Build with `--features steam` for the Steam channel. Without it the plugin compiles to the unavailable stub.

**Capability (`src-tauri/capabilities/default.json`):**

```json
{ "permissions": ["steamworks:default", "core:event:default"] }
```

`core:event:default` is only needed to listen for events.

## Usage

```ts
import {
	getStatus,
	unlockAchievement,
	indicateProgress,
	setStat,
	storeStats,
	onAchievementStored,
	isUnavailableError,
} from '@liminal-hq/plugin-steamworks';

const status = await getStatus();
if (status.state === 'unavailable') {
	console.info('Running without Steam:', status.reason);
}

try {
	await unlockAchievement('ACH_FIRST_CITY');
} catch (error) {
	if (!isUnavailableError(error)) throw error; // a bad id, say, is still a bug
}

await setStat('Population', 12400);
await storeStats();

const unlisten = await onAchievementStored(({ id }) => console.log('stored', id));
```

### Commands

| Function                                                                                                                                                                                                        | Notes                                                                                     |
| --------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | ----------------------------------------------------------------------------------------- |
| `isAvailable()`, `getStatus()`                                                                                                                                                                                  | Never reject. The status carries a `reason` when unavailable.                             |
| `getPlayer()`                                                                                                                                                                                                   | `steamId` is a decimal string because JavaScript numbers lose 64-bit precision.           |
| `getAchievement(id)`                                                                                                                                                                                            | `{ id, achieved, unlockTime }`.                                                           |
| `unlockAchievement(id)`                                                                                                                                                                                         | Sets the achievement and stores it so Steam shows its notification.                       |
| `clearAchievement(id)`                                                                                                                                                                                          | For development. Needs the `allow-achievements-dev` permission.                           |
| `indicateProgress(id, current, max)`                                                                                                                                                                            | Progress toast for counted achievements. `current` must not exceed `max`.                 |
| `getStat(name, kind = 'int')`, `setStat(name, value, kind?)`                                                                                                                                                    | `setStat` does not store; call `storeStats()` to batch writes.                            |
| `storeStats()`                                                                                                                                                                                                  | Persists pending stat and achievement changes.                                            |
| `setRichPresence(entries)`, `clearRichPresence()`                                                                                                                                                               | The status line friends see. A `null` value removes a key. Needs `allow-presence`.        |
| `activateOverlay(page)`, `activateOverlayToWebPage(url)`                                                                                                                                                        | Opens an overlay page, or an `http(s)` address. Needs `allow-overlay`.                    |
| `cloudWrite(name, data)`, `cloudRead(name)`, `cloudReadText(name)`, `cloudDelete(name)`                                                                                                                         | Whole-file Steam Cloud access. Needs `allow-cloud`. See [Steam Cloud](#steam-cloud).      |
| `cloudList()`, `cloudQuota()`, `cloudStatus()`                                                                                                                                                                  | Files, quota in bytes, and whether the account and app allow Cloud. Needs `allow-cloud`.  |
| `getDlc()`, `isDlcInstalled(appId)`                                                                                                                                                                             | The app's downloadable content and whether each is installed. Needs `allow-dlc`.          |
| `createLobby(kind, maxMembers)`, `joinLobby(id)`, `listLobbies()`, `leaveLobby(id)`, `getLobby(id)`, `setLobbyData(id, key, value)`, `setLobbyJoinable(id, joinable)`                                           | Steam lobbies. Needs `allow-lobbies`. See [Lobbies](#lobbies).                            |
| `getWorkshopItems()`, `subscribeWorkshopItem(id)`, `unsubscribeWorkshopItem(id)`, `downloadWorkshopItem(id, highPriority?)`                                                                                     | Workshop subscriptions and downloads. Needs `allow-workshop`. See [Workshop](#workshop).  |
| `inputInit()`, `getInputControllers()`, `getInputActionSet(name)`, `activateInputActionSet(...)`, `getInputAction(kind, name)`, `getDigitalActionState(...)`, `getAnalogActionState(...)`, `getInputGlyph(...)` | Steam Input. Needs `allow-input`. See [Steam Input](#steam-input).                        |
| `getDeviceInfo()`, `isSteamDeck()`                                                                                                                                                                              | Whether the app runs on a Steam Deck or in Big Picture mode. Needs `allow-device`.        |
| `hookScreenshots(hook)`, `triggerScreenshot()`, `addScreenshotToLibrary(path, w, h)`                                                                                                                            | Steam screenshot integration. Needs `allow-screenshots`. See [Screenshots](#screenshots). |

`setStat` sends a whole number as an int and a fraction as a float. Pass `kind: 'float'` for a float stat that currently holds a whole number.

### Events

| Event                               | Helper                  | Payload                                          |
| ----------------------------------- | ----------------------- | ------------------------------------------------ |
| `steamworks://stats-received`       | `onStatsReceived`       | `{ success, steamId }`                           |
| `steamworks://stats-stored`         | `onStatsStored`         | `{ success }`                                    |
| `steamworks://achievement-stored`   | `onAchievementStored`   | `{ id, current, max }`                           |
| `steamworks://overlay-toggled`      | `onOverlayToggled`      | `{ active }`                                     |
| `steamworks://screenshot-requested` | `onScreenshotRequested` | `{}` (needs `hookScreenshots(true)`)             |
| `steamworks://lobby-chat-update`    | `onLobbyChatUpdate`     | `{ lobbyId, userChanged, makingChange, change }` |
| `steamworks://lobby-data-update`    | `onLobbyDataUpdate`     | `{ lobbyId, memberId, success }`                 |

## Rust API

```rust
use tauri_plugin_steamworks::SteamworksExt;

app.steamworks().unlock_achievement("ACH_FIRST_CITY")?;
```

`Builder` options: `restart_if_necessary(bool)` relaunches through Steam and exits when the app was started outside it (off by default), `callback_interval(Duration)` (clamped to 10–1000 ms), and, with the `mock` feature, `mock(MockConfig)`.

### Initialising before the Tauri builder

Some integrations need Steam initialised before `tauri::Builder` runs. Call `preinit` first and hand the result to the plugin; it never panics.

```rust
let pre = tauri_plugin_steamworks::preinit(480, false);
// pre.client() is the steamworks::Client, for other integrations.
tauri::Builder::default().plugin(tauri_plugin_steamworks::Builder::from_preinit(pre).build())
```

The `steamworks` crate is re-exported as `tauri_plugin_steamworks::steamworks` so your app and the plugin cannot end up on different versions.

### Configuration

Optional, in `tauri.conf.json`:

```json
{ "plugins": { "steamworks": { "callbackIntervalMs": 100, "mock": false } } }
```

`mock: true` is honoured only when the `mock` feature is compiled.

## Permissions

| Permission set           | Grants                                                           | In `default` |
| ------------------------ | ---------------------------------------------------------------- | ------------ |
| `allow-player-info`      | `isAvailable`, `getStatus`, `getPlayer`                          | yes          |
| `allow-achievements`     | `getAchievement`, `unlockAchievement`, `indicateProgress`        | yes          |
| `allow-stats`            | `getStat`, `setStat`, `storeStats`                               | yes          |
| `allow-presence`         | `setRichPresence`, `clearRichPresence`                           | no           |
| `allow-overlay`          | `activateOverlay`, `activateOverlayToWebPage`                    | no           |
| `allow-cloud`            | the `cloud*` functions                                           | no           |
| `allow-lobbies`          | the lobby functions                                              | no           |
| `allow-workshop`         | the Workshop functions                                           | no           |
| `allow-input`            | the Steam Input functions                                        | no           |
| `allow-dlc`              | `getDlc`, `isDlcInstalled`                                       | no           |
| `allow-device`           | `getDeviceInfo`, `isSteamDeck`                                   | no           |
| `allow-screenshots`      | `hookScreenshots`, `triggerScreenshot`, `addScreenshotToLibrary` | no           |
| `allow-achievements-dev` | `clearAchievement`, so a shipped app cannot wipe player progress | no           |

Individual `allow-<command>` and `deny-<command>` permissions also exist. Sets that reach outside the game, such as presence, overlay and cloud, are never part of `default`.

## Testing without Steam

Enable the `mock` feature and give the plugin a fake:

```rust
Builder::new(480)
    .mock(MockConfig {
        achievements: vec!["ACH_FIRST_CITY".into()],
        stats: vec![("Population".into(), StatValue::Int(0))],
        ..MockConfig::default()
    })
    .build()
```

Like Steam, the fake rejects achievements and stats you did not declare. `MockBackend::calls()` returns what it received, for assertions.

## Manual check against Spacewar (appid 480)

1. Start Steam and sign in.
2. Run your app built with `--features steam` and `Builder::new(480)`.
3. `getStatus()` should report `available` with `backend: "steam"`, and `getPlayer()` your persona name.
4. `unlockAchievement('ACH_WIN_ONE_GAME')` should show Steam's notification and raise `achievement-stored`. `clearAchievement` (with `allow-achievements-dev`) resets it.
5. `setStat('NumGames', 1)` then `storeStats()` should raise `stats-stored` with `success: true`.
6. Quit Steam and relaunch the app. `getStatus()` should report `unavailable` / `steamNotRunning`, and the app should carry on.

## Licence and redistribution

This plugin is `Apache-2.0 OR MIT` and contains no Steamworks SDK code or binaries. The optional `steam` feature depends on the `steamworks` and `steamworks-sys` crates, which are `MIT / Apache-2.0`. `steamworks-sys` bundles Valve's prebuilt `steam_api` libraries, which are covered by Valve's Steamworks SDK Access Agreement and not by those licences. By enabling `steam` you accept Valve's terms, and your Steam build must ship `steam_api64.dll`, `libsteam_api.so` or `libsteam_api.dylib` next to the app. To build against an SDK you downloaded yourself, set `STEAM_SDK_LOCATION`. Read the current agreement before shipping; this README is not legal advice.

## Platform support

| Platform | Support                                                                                |
| -------- | -------------------------------------------------------------------------------------- |
| Linux    | With the `steam` feature and a running Steam client; otherwise reports unavailable     |
| Windows  | With the `steam` feature and a running Steam client; otherwise reports unavailable     |
| macOS    | With the `steam` feature and a running Steam client; otherwise reports unavailable     |
| Android  | Stub; always reports `unsupportedPlatform`. The SDK is never linked, even with `steam` |
| iOS      | Stub; always reports `unsupportedPlatform`. The SDK is never linked, even with `steam` |

## Steam Cloud

Most games should use Steam's **Auto-Cloud**, which syncs a save directory with no code at all: configure it for your app in the Steamworks partner settings (Application → Steam Cloud → Auto-Cloud) and nothing in this plugin is needed. The `cloud*` functions are for apps that cannot use Auto-Cloud.

- Files are written and read whole; there is no streaming. A file is at most 100 MiB, and a read or write holds the call until Steam finishes.
- Names use ASCII letters, digits and space `. _ -`, with `/` between folders. Empty, `.` and `..` segments, absolute paths and other characters are rejected, so a name is always a safe relative path.
- Bytes cross the IPC boundary as the raw request and response body, not as JSON.
- Steam enforces a per-account quota. `cloudQuota()` reports it, and a write over quota rejects.
- `cloudStatus()` says whether the player has Cloud turned on for your app; writes can fail when they have not.

```ts
import { cloudWrite, cloudReadText, cloudList } from '@liminal-hq/plugin-steamworks';

await cloudWrite('settings.json', JSON.stringify({ volume: 0.8 }));
const settings = JSON.parse(await cloudReadText('settings.json'));
console.log(await cloudList());
```

## Lobbies

Lobbies group players before a match and carry shared data. This plugin covers the lobby itself: create, join, list, read, set data, and open or close it. Steam IDs are decimal strings throughout, because JavaScript numbers lose 64-bit precision.

```ts
import { createLobby, setLobbyData, onLobbyChatUpdate } from '@liminal-hq/plugin-steamworks';

const id = await createLobby('friendsOnly', 4);
await setLobbyData(id, 'map', 'fizzton'); // only the owner can change lobby data
await onLobbyChatUpdate(({ change, userChanged }) => console.log(userChanged, change));
```

- Kinds are `private`, `friendsOnly`, `public` and `invisible`. A lobby holds 1 to 250 members.
- `listLobbies()` uses Steam's default search and returns open public lobbies; it does not take filters.
- Lobby data keys are at most 255 bytes and values at most 8192 bytes.
- Not included: lobby chat messages, invites, game-server hand-off and the networking layer that real-time play needs. They are separate work.

## Workshop

`getWorkshopItems()` lists the items the player is subscribed to with their install state, folder and size. `subscribeWorkshopItem`, `unsubscribeWorkshopItem` and `downloadWorkshopItem` change subscriptions and start downloads; poll `getWorkshopItems()` to watch an install finish. The install folder is a path on the player's machine for your app to read content from.

Not included: publishing or updating items and searching the Workshop.

## Steam Input

Steam Input lets players rebind controllers, and gives you the glyph for the button an action is bound to, so prompts match their hardware (for example a PlayStation or Steam Deck button). It needs an action manifest configured for the app in Steamworks.

```ts
import {
	inputInit,
	getInputControllers,
	getInputActionSet,
	getInputAction,
	activateInputActionSet,
	getDigitalActionState,
	getInputGlyph,
} from '@liminal-hq/plugin-steamworks';

await inputInit();
const [pad] = await getInputControllers();
const set = await getInputActionSet('Gameplay');
const jump = await getInputAction('digital', 'jump');
await activateInputActionSet(pad.handle, set);

const { pressed } = await getDigitalActionState(pad.handle, jump);
const png = await getInputGlyph(pad.handle, set, 'digital', jump, 'small');
const url = URL.createObjectURL(new Blob([png], { type: 'image/png' }));
```

- Handles are decimal strings. Look actions and sets up once and reuse the handles.
- Steam updates input whenever callbacks run, so the default 100 ms callback interval makes polling feel sluggish. Set `callbackIntervalMs` to about 16 while using Steam Input.
- Each state read is an IPC call. That suits menus and prompts; for per-frame gameplay input prefer the browser's Gamepad API, and use Steam Input for bindings and glyphs.
- `getInputGlyph` rejects when the action has no button bound on that controller.

## Screenshots

By default Steam takes screenshots itself (F12 or the overlay), and nothing here is needed. Hooking is for apps whose window Steam cannot capture, such as a webview game on Windows:

1. `hookScreenshots(true)` makes Steam send `screenshot-requested` instead of capturing.
2. In `onScreenshotRequested`, capture the game, save it as an image file, then call `addScreenshotToLibrary(path, width, height)`.

`addScreenshotToLibrary` accepts only an absolute path to an existing `png`, `jpg`, `jpeg` or `tga` file, so the webview cannot make Steam read other files. If the overlay-surface plugin already hooks screenshots and captures them, leave `hookScreenshots` alone; both would answer the same request.

## Steam overlay

Rich presence needs nothing special. The overlay is best-effort, because on Windows it cannot hook a WebView2 window by itself: WebView2 renders in separate processes, so the game process never presents a frame for Steam's injected overlay to attach to. The separate [`tauri-plugin-steam-overlay-surface`](https://github.com/PSG-Team/tauri-steam-overlay-surface) plugin works around that with a transparent, click-through window that presents empty frames for Steam to draw on. This plugin does not reimplement that; it works alongside it.

To use both:

1. Initialise Steam before the Tauri builder with `preinit`, and build this plugin from the result with `Builder::from_preinit`. The overlay surface requires Steam to be initialised first, and a second `steamworks::Client` must not be created.
2. Add the overlay-surface plugin. It requires Tauri's `unstable` feature and Windows.
3. Forward overlay toggles to it from Rust. This plugin already registers Steam's overlay callback and runs the callback pump, so the surface plugin needs neither:

```rust
use tauri_plugin_steamworks::SteamworksExt;

// in the Tauri builder's setup
let handle = app.handle().clone();
app.steamworks().on_overlay_toggled(move |active| {
    tauri_plugin_steam_overlay_surface::on_overlay_activated(&handle, active);
});
```

Follow the overlay-surface README for its other requirements, such as forwarding Shift+Tab from the webview. On other platforms `activateOverlay` works as Steam provides it. In all cases `onOverlayToggled` in the webview reports when the overlay opens and closes.
