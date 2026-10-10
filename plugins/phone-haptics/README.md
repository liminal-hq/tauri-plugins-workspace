# @liminal-hq/plugin-phone-haptics

Author portable haptic patterns once and replay them on Android, with capability reporting and a stepped fallback ladder instead of silent failures.

## Package Links

- crates.io: https://crates.io/crates/tauri-plugin-phone-haptics
- npm: https://www.npmjs.com/package/@liminal-hq/plugin-phone-haptics
- Report bugs: https://github.com/liminal-hq/tauri-plugins-workspace/issues

## Installation

### Rust

```toml
[dependencies]
tauri-plugin-phone-haptics = "0.1"

# Alternatively with Git:
tauri-plugin-phone-haptics = { git = "https://github.com/liminal-hq/tauri-plugins-workspace", branch = "main" }
```

### JavaScript

```bash
pnpm add @liminal-hq/plugin-phone-haptics
# or
npm add @liminal-hq/plugin-phone-haptics
# or
yarn add @liminal-hq/plugin-phone-haptics
```

## Usage

### Rust

```rust
fn main() {
    tauri::Builder::default()
        .plugin(tauri_plugin_phone_haptics::init())
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
```

Grant the plugin's permissions in a capability file:

```json
{
	"identifier": "default",
	"windows": ["main"],
	"permissions": ["phone-haptics:default"]
}
```

### JavaScript

```ts
import { capabilities, register, trigger, stop } from '@liminal-hq/plugin-phone-haptics';

const caps = await capabilities();
console.log(caps.topTier);

await register('thud', {
	format: 'haptics-lab/pattern@1',
	events: [{ type: 'transient', at: 0, intensity: 0.9, sharpness: 0.3 }],
});

const result = await trigger('thud');
console.log(result.tier, result.downgraded, result.reason);

await stop();
```

## Tiers

Every call resolves with the tier that actually played, and says why when it had to step down. A device with no vibrator resolves at tier 0 and never throws; invalid input rejects with `INVALID_EFFECT`. The guest bindings add that code to the rejection (`HapticsError.code`); a raw `invoke('plugin:phone-haptics|…')` call receives the message text only, such as `invalid request: controlPoints cannot be empty`. A failure that is not about the input, such as a missing `phone-haptics:allow-play` permission or a broken native bridge, rejects with code `PLUGIN_ERROR` instead, so the two can be told apart.

| Tier | Name       | Needs                              | How a pattern plays                                                    |
| ---- | ---------- | ---------------------------------- | ---------------------------------------------------------------------- |
| 4    | Envelope   | API 36 and an actuator that has it | Control points with amplitude and frequency                            |
| 3    | Primitives | API 30 and at least one primitive  | Primitives per event, swapped for neighbours when missing              |
| 2    | Amplitude  | Amplitude control                  | One waveform of one-shots sampled from the curves                      |
| 1    | On / off   | Any vibrator                       | Duty-cycled on a 20 ms period; segments quieter than amplitude 40 drop |
| 0    | Off        | Nothing                            | Resolves `ok` at tier 0                                                |

## API

| Function                                         | Purpose                                                                                                                               |
| ------------------------------------------------ | ------------------------------------------------------------------------------------------------------------------------------------- |
| `capabilities({ refresh? })`                     | Reports the vibrator, each primitive and predefined effect, envelope limits and `topTier`; cached after the first read                |
| `register(id, pattern, opts?)` and `registerAll` | Validates a portable pattern (every problem listed with a path and a fix) and returns how it compiles on this device                  |
| `trigger(id, opts?)`                             | Compiles at the master scale, applies the pattern's policy and resolves with `policy` of `played`, `queued`, `dropped` or `coalesced` |
| `unregister(id)`                                 | Forgets a pattern                                                                                                                     |
| `compile(pattern, { tier? })`                    | Synchronous, pure preview of the compiled pattern for the loaded capabilities                                                         |
| `play(request)`                                  | Raw escape hatch: one-shot, waveform, predefined, composition or envelope effect                                                      |
| `playSteps(steps)`                               | Plays `{ atMs, request }` steps scheduled natively from one start time                                                                |
| `ui(kind)`                                       | UI lane (`confirm`, `reject`, `tick`, `toggle-on`, `toggle-off`, `drag-start`) through the OS's view haptics                          |
| `setMasterScale(v)`, `setMaxTier(t)`, `stop()`   | Scale every intensity, cap the tier, and cancel playback plus every queue and timer                                                   |

Pattern policies are `interrupt` (default), `queue` (four deep), `drop-if-busy` and `{ coalesce: ms }`. The `ui` lane always follows the system touch-feedback setting and ignores the master scale, policies and tier cap.

### Where requests are validated

The plugin's Rust layer validates every raw request (`play` and `play_steps`) on every platform, before anything else happens to it. A request that no device could play rejects with `INVALID_EFFECT`, and it rejects the same way whatever the master scale, the `setMaxTier` cap or the hardware is, so an invalid request never succeeds quietly on a device without a vibrator or at a scale of 0. The rules are:

- One-shots need a positive duration and, when given, an amplitude within 1 to 255.
- Waveforms need at least one non-zero timing, amplitudes (when given) within 0 to 255 and one per timing, and a `repeat` of -1 or an index into `timingsMs`.
- Predefined effects and composition primitives must be known ids, and a composition step's `scale` must be within 0 to 1.
- Envelopes need at least one control point, amplitudes within 0 to 1, positive frequencies and durations, and a total that fits `maxDurationMs`; an envelope is never shortened, because a shorter point can fall under the device's minimum.
- A step list needs between 1 and 512 steps, each starting before `maxDurationMs`, and each step may only use the time left after its start.

The `setMaxTier` cap classifies a raw request by what it asks for: a one-shot or waveform with no amplitudes is tier 1 on any device, the same one with amplitudes is tier 2 where the motor has amplitude control, a predefined effect is at most tier 3, a composition is tier 3 and an envelope is tier 4. A request above the cap resolves at tier 0 with the reason; it does not step down to a lower form.

Only after validation does the Rust layer apply the master scale and the tier cap, then truncate one-shots and waveforms to `maxDurationMs` and drop repeats the config does not allow. Each change is reported in `reason`. What depends on the device, such as primitive substitution, the amplitude-control fallback and the API-level fallbacks, stays in the Android plugin.

The pure pattern logic (validation, compiler, scheduler and tables) lives in `guest-js/pattern/` and imports nothing from `@tauri-apps/*`, so it can be extracted into its own package later.

## Configuration

Set these under `plugins.haptics` in `tauri.conf.json`; all are optional.

| Key                           | Default | Meaning                                                                                |
| ----------------------------- | ------- | -------------------------------------------------------------------------------------- |
| `defaultUsage`                | `touch` | Usage for requests that do not set one                                                 |
| `respectSystemHapticsSetting` | `true`  | Silence `touch` usage (and the UI lane) when system touch feedback is off              |
| `stopBeforePlay`              | `true`  | Cancel the current effect before starting another                                      |
| `maxDurationMs`               | `10000` | Cap on any effect's duration; truncation is reported                                   |
| `maxAmplitude`                | `255`   | Cap on waveform amplitude                                                              |
| `allowRepeatingWaveforms`     | `false` | Repeating waveforms play once unless enabled, and `reason` says the repeat was ignored |

## Permissions

`phone-haptics:default` allows `capabilities`, `play`, `play_steps`, `stop` and `ui`. See `permissions/autogenerated/reference.md` for the individual permissions.

The plugin's Android manifest declares `android.permission.VIBRATE`; consuming apps do not need to edit their manifests.

## Platform support

| Platform | Support                                                                                                                       |
| -------- | ----------------------------------------------------------------------------------------------------------------------------- |
| Android  | Full: one-shot, waveform, predefined, composition and envelope (API 36+) effects; envelope falls back where hardware lacks it |
| Linux    | No vibrator: every call resolves at tier 0 with a reason, so patterns still validate and compile                              |
| Windows  | Same as Linux                                                                                                                 |
| macOS    | Same as Linux                                                                                                                 |
| iOS      | Not implemented: every call resolves at tier 0 like desktop                                                                   |

The Android Kotlin is compiled by an app's Android build. It has been exercised on a Pixel 8 Pro, which has primitives and amplitude control but no envelope hardware, so tier 4 is covered by unit tests only.
