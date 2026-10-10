# tauri-plugin-gamepad-haptics

Rumble for gamepads from one pattern language. It plays on the pad and never reads its input, so it sits beside the webview's Gamepad API without competing for the device.

## Package Links

- Rust crate: `tauri-plugin-gamepad-haptics`
- npm package: `@liminal-hq/plugin-gamepad-haptics`

## Installation

```toml
[dependencies]
tauri-plugin-gamepad-haptics = { path = "../../plugins/gamepad-haptics" }
```

```rust
tauri::Builder::default().plugin(tauri_plugin_gamepad_haptics::init())
```

Add `"gamepad-haptics:default"` to your capability, plus `core:event:default` to listen for pad events.

## Usage

```ts
import { createBackend, PATTERN_FORMAT } from '@liminal-hq/plugin-gamepad-haptics';

const pads = createBackend();
await pads.register('hurt', {
	format: PATTERN_FORMAT,
	events: [
		{ type: 'transient', at: 0, intensity: 1, sharpness: 0.9 },
		{
			type: 'continuous',
			at: 40,
			duration: 120,
			intensity: [
				{ t: 0, v: 0.9 },
				{ t: 1, v: 0 },
			],
			sharpness: 0.1,
		},
	],
});
await pads.trigger('hurt'); // plays on the first pad that can rumble
```

A pattern is transients (a tap with intensity and sharpness) and continuous events (a hum with curves), the same format the phone plugin uses. Sharpness decides which motor plays: crisp goes to the light, high-frequency motor and dull goes to the heavy, low-frequency one.

### The ladder

| Tier | Plays                                             |
| ---- | ------------------------------------------------- |
| 3    | Triggers and both body motors (not yet built)     |
| 2    | Both body motors, mixed by sharpness              |
| 1    | One motor, the stronger of the two                |
| 0    | Nothing; resolves `{ ok: true, tier: 0, reason }` |

A pad that cannot play never throws. Results report the tier that played and why it was lower.

## API

- `createBackend()` returns `register`, `trigger`, `setMasterScale`, `setMaxTier`, `stop` and `capabilities`.
- `listPads()`, `capabilities()`, `playFrames(padId, frames, scale?)`, `identify(padId)` and `stop(padId?)` call the plugin directly.
- `onPadConnected`, `onPadChanged` and `onPadDisconnected` follow hot-plug. Pads get a slot (`gamepad:0`, `gamepad:1`, ...) and a returning pad gets its old slot back while the app runs.
- `resolvePad(pads, hint)` finds the plugin's pad for a Web `Gamepad`, an SDL GUID, or a vendor, product and serial. The GUID is the same one SDL and gilrs report. Two identical pads resolve as `ambiguous`; call `identify(padId)` and ask the player which one buzzed.

## Where requests are validated

Rust validates every request before a backend sees it: at most 512 frames, levels within 0..1, durations of at least 1 ms, and a total no longer than `maxDurationMs`. A scale of 0, a pad that cannot play and `setMaxTier` never let an invalid request through. A motor is never left running longer than `maxContinuousMs`; the excess is cut and the timing after it kept. A shared corpus (`tests/conformance/frames.json`) is checked by both the Rust and the guest suites, and both sides are fuzzed.

## Configuration

```json
{
	"plugins": {
		"gamepad-haptics": {
			"maxDurationMs": 3000,
			"maxContinuousMs": 2000,
			"masterScale": 1,
			"stopOnBlur": true
		}
	}
}
```

Every pad stops when the app exits, when its pad is unplugged, and (with `stopOnBlur`) when the window loses focus.

## Living beside the Gamepad API

The plugin never opens a pad for input and never grabs it, so the webview reads buttons and sticks as usual. Rumble is the one thing two writers could fight over, so each pad has one writer:

- A pad the plugin can address is played natively, always.
- Only when no native pad fits does the guest play through `navigator.getGamepads()[n].vibrationActuator` (`dual-rumble`). That path needs a webview that supports it: WebView2 and WKWebView on macOS do; WebKitGTK does not yet in current releases, and Android's WebView does not.
- Do not call `playEffect` yourself on a pad this plugin plays. On Linux the kernel adds rumble from different writers together; elsewhere the last writer wins.
- The webview lists a pad only after a button is pressed on the page, so a web `Gamepad` hint may not exist yet at start-up. Native pads are listed at once.

## Platform support

| Platform | Backend              | Status                                        |
| -------- | -------------------- | --------------------------------------------- |
| Linux    | evdev force feedback | Works; checked on DualShock 3                 |
| Windows  | Web fallback only    | Native backend planned (Windows.Gaming.Input) |
| macOS    | Web fallback only    | Native backend planned (GameController)       |
| Android  | None                 | Planned (`InputDevice` vibrator, API 31+)     |
| iOS      | None                 | Planned (GameController)                      |

### Linux access

The plugin opens `/dev/input/event*` for writing. On most desktops the logged-in user already has access to gamepads through a udev `uaccess` rule. A pad that cannot be opened is listed at tier 0 with `No write access to /dev/input/eventN`. In Flatpak, grant `--device=input` (Flatpak 1.15.6 or later); in Snap, connect the `joystick` interface.

DualShock 3 pads use the `hid-sony` driver, whose light motor only switches on and off, so the plugin drives it at full strength or not at all.

### Checking a pad

```sh
cargo run -p tauri-plugin-gamepad-haptics --example probe                    # list pads
cargo run -p tauri-plugin-gamepad-haptics --example probe -- identify gamepad:0
cargo run -p tauri-plugin-gamepad-haptics --example probe -- play gamepad:0
cargo run -p tauri-plugin-gamepad-haptics --example probe -- watch           # hot-plug events
```

## Permissions

`gamepad-haptics:default` allows `capabilities`, `identify`, `list_pads`, `play_frames` and `stop`.

## Not yet tested on hardware

DualSense, Xbox and Switch Pro pads, triggers, Windows, macOS, Android and iOS.
