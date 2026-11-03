# Tauri Plugin Steamworks

Steamworks for Tauri v2 games: player identity, achievements and stats today, with presence, overlay and cloud files planned. One binary can ship inside and outside Steam, because the plugin reports a queryable _unavailable_ state instead of failing when Steam is missing.

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

| Function                                                     | Notes                                                                           |
| ------------------------------------------------------------ | ------------------------------------------------------------------------------- |
| `isAvailable()`, `getStatus()`                               | Never reject. The status carries a `reason` when unavailable.                   |
| `getPlayer()`                                                | `steamId` is a decimal string because JavaScript numbers lose 64-bit precision. |
| `getAchievement(id)`                                         | `{ id, achieved, unlockTime }`.                                                 |
| `unlockAchievement(id)`                                      | Sets the achievement and stores it so Steam shows its notification.             |
| `clearAchievement(id)`                                       | For development. Needs the `allow-achievements-dev` permission.                 |
| `indicateProgress(id, current, max)`                         | Progress toast for counted achievements. `current` must not exceed `max`.       |
| `getStat(name, kind = 'int')`, `setStat(name, value, kind?)` | `setStat` does not store; call `storeStats()` to batch writes.                  |
| `storeStats()`                                               | Persists pending stat and achievement changes.                                  |

`setStat` sends a whole number as an int and a fraction as a float. Pass `kind: 'float'` for a float stat that currently holds a whole number.

### Events

| Event                             | Helper                | Payload                |
| --------------------------------- | --------------------- | ---------------------- |
| `steamworks://stats-received`     | `onStatsReceived`     | `{ success, steamId }` |
| `steamworks://stats-stored`       | `onStatsStored`       | `{ success }`          |
| `steamworks://achievement-stored` | `onAchievementStored` | `{ id, current, max }` |

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
| `allow-achievements-dev` | `clearAchievement`, so a shipped app cannot wipe player progress | no           |

Individual `allow-<command>` and `deny-<command>` permissions also exist. Later releases add `allow-presence`, `allow-overlay` and `allow-cloud` sets that are never part of `default`.

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

## Steam overlay

The overlay is not part of this release. On Windows the Steam overlay cannot hook a WebView2 window by itself; a later release documents using this plugin's `preinit` with a separate overlay-surface plugin.
