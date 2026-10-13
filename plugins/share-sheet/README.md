# @liminal-hq/plugin-share-sheet

Hands a file or a piece of text to the operating system's share sheet, so the person can send it to another app, AirDrop it, mail it and so on. It reports how the share ended as far as the platform can tell, and `canShare()` says whether there is a share sheet at all, so a UI can hide its Share button.

The plugin only shows the share UI. Saving a file somewhere the person chooses is the job of the official [dialog](https://v2.tauri.app/plugin/dialog/) and [fs](https://v2.tauri.app/plugin/file-system/) plugins. It has no network code, and on Android it adds no permissions at all: no Internet, no storage and no `READ_EXTERNAL_STORAGE`.

## Installation

### Rust

```toml
[dependencies]
tauri-plugin-share-sheet = "0.1"

# Alternatively with Git:
tauri-plugin-share-sheet = { git = "https://github.com/liminal-hq/tauri-plugins-workspace", branch = "main" }
```

### JavaScript

```bash
pnpm add @liminal-hq/plugin-share-sheet
```

## Usage

### Rust

```rust
fn main() {
    tauri::Builder::default()
        .plugin(tauri_plugin_share_sheet::init())
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
```

Grant the permission in a capability file, for example `src-tauri/capabilities/default.json`:

```json
{
	"identifier": "default",
	"windows": ["main"],
	"permissions": ["share-sheet:default"]
}
```

Without it, every JavaScript call rejects with a "not allowed" error, which the bindings report as a `ShareSheetError` with the code `failed`.

### JavaScript

```typescript
import { appCacheDir, join } from '@tauri-apps/api/path';
import { writeFile } from '@tauri-apps/plugin-fs';
import { canShare, shareFile, shareText, ShareSheetError } from '@liminal-hq/plugin-share-sheet';

const { files } = await canShare();
shareButton.hidden = !files;

// On Android the file must be in the app's cache or files directory, so write it there first.
const path = await join(await appCacheDir(), 'save.zip');
await writeFile(path, zipBytes);

try {
	const { status } = await shareFile({ path, mime: 'application/zip', title: 'My save' });
	if (status === 'cancelled') showToast('Not shared');
} catch (error) {
	if (error instanceof ShareSheetError && error.code === 'unsupported') {
		shareButton.hidden = true;
	} else {
		throw error;
	}
}

await shareText({ text: 'I beat level 3!', title: 'Lieutenant Fizz' });
```

The calls act on the window that made them: the share UI opens over it.

## API

| Function                            | Resolves to                                             |
| ----------------------------------- | ------------------------------------------------------- |
| `shareFile({ path, mime, title? })` | `{ status: 'shared' \| 'cancelled' \| 'unknown' }`      |
| `shareText({ text, title? })`       | `{ status: 'shared' \| 'cancelled' \| 'unknown' }`      |
| `canShare()`                        | `{ supported: boolean, files: boolean, text: boolean }` |

- **`path`** is an absolute file system path, not a `file://` or `content://` URI. It must name an existing regular file. On Android it must be inside the app's cache directory (`appCacheDir()`, `BaseDirectory.AppCache`) or its files directory (`Context.getFilesDir()`). Tauri's `appDataDir()` on Android is the app's data directory, one level above `files`, so it is not covered; use the cache directory.
- **`mime`** is a `type/subtype` MIME type such as `application/zip`, `image/*` or `*/*`, without parameters. Android uses it to choose which apps to offer; the other platforms work the type out from the file.
- **`title`** is optional. Where the platform has somewhere to put it, it becomes the share sheet's heading or the subject a mail app uses. Windows needs a title, so it falls back to the file name or the first line of the text.
- **`text`** must not be empty and is at most 100 KiB, because Android passes it through a Binder transaction.

### Statuses

`shared` means the person picked a share target. Only iOS also confirms that the target finished; elsewhere the target may still be abandoned (a mail draft discarded, for example). `cancelled` means the person dismissed the sheet without picking anything, and is reported only where the platform says so. `unknown` means the sheet was shown but the platform does not say how it ended.

| Platform | `shared`                                                          | `cancelled`              | `unknown`                                                                                    | Resolves                                                  |
| -------- | ----------------------------------------------------------------- | ------------------------ | -------------------------------------------------------------------------------------------- | --------------------------------------------------------- |
| Android  | The chooser reported the chosen target (`EXTRA_CHOSEN_COMPONENT`) | Never                    | The chooser closed without naming a target: backed out, or a share-sheet action such as Copy | When a target is picked, or just after the chooser closes |
| iOS      | The chosen activity completed                                     | The sheet was dismissed  | Never                                                                                        | When the sheet closes                                     |
| macOS    | A sharing service was chosen                                      | The picker was dismissed | The picker was replaced by a second share                                                    | When a service is picked or the picker closes             |
| Windows  | Never                                                             | Never                    | Always                                                                                       | As soon as the share UI is shown                          |
| Linux    | -                                                                 | -                        | -                                                                                            | Rejects with `unsupported`                                |

### Errors

Every failure rejects with a `ShareSheetError` whose `code` is one of:

| Code               | When                                                                                                                                                          |
| ------------------ | ------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `unsupported`      | There is no share UI here: Linux, any other target without one, or a Windows build where `DataTransferManager.IsSupported` is false.                          |
| `invalid-argument` | An empty or oversized text, a malformed MIME type or a title longer than 200 characters.                                                                      |
| `invalid-path`     | An empty, relative or URI path, a directory, or (on Android) a path outside the app's cache and files directories, including through `..` or a symbolic link. |
| `not-found`        | No file exists at the path.                                                                                                                                   |
| `failed`           | The platform could not show the share UI, or the call was not allowed by the app's capabilities.                                                              |

`unsupported` is checked first, so on Linux every call rejects with it whatever the arguments.

## Platform support

| Platform | Support | How                                                                                                                                                                                   | Verified                                                       |
| -------- | ------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | -------------------------------------------------------------- |
| Android  | Full    | `ACTION_SEND` with `EXTRA_STREAM` and `FLAG_GRANT_READ_URI_PERMISSION` through the plugin's own `FileProvider`, wrapped in `Intent.createChooser` with a chosen-target `IntentSender` | Rust and Kotlin compile and pass lint; not yet run on a device |
| iOS      | Full    | `UIActivityViewController` from the key window's top view controller, with a centred popover anchor on iPad                                                                           | Rust type-checked; the Swift code has not been compiled or run |
| macOS    | Full    | `NSSharingServicePicker` over the calling window's content view, on the main thread, with a delegate for the outcome                                                                  | Type-checked; not yet run on a Mac                             |
| Windows  | Full    | `IDataTransferManagerInterop::ShowShareUIForWindow` with the window's HWND (Windows 10 and later), with a `StorageFile` or text in the `DataPackage`                                  | Type-checked; not yet run on Windows                           |
| Linux    | None    | Every call rejects with `unsupported` and `canShare()` reports `false` for everything                                                                                                 | Tested                                                         |

**Why not Linux?** Linux desktops have no standard share UI. `xdg-desktop-portal`, which sandboxed apps use for file choosers, screenshots and opening URIs, has no share portal, and while some desktops have their own sharing framework (KDE's Purpose, for one), there is no interface that works across desktops. Rather than pretend, the plugin reports that there is nothing to show. On Linux, offer a "Save as…" through the dialog plugin instead.

### Android

The plugin's manifest declares its own provider, merged into the app automatically, so the app does not edit its manifest:

```xml
<provider
    android:name="ca.liminalhq.sharesheet.ShareSheetFileProvider"
    android:authorities="${applicationId}.share-sheet.fileprovider"
    android:exported="false"
    android:grantUriPermissions="true">
    <meta-data android:name="android.support.FILE_PROVIDER_PATHS" android:resource="@xml/share_sheet_file_paths" />
</provider>
```

The provider class and the authority (`<applicationId>.share-sheet.fileprovider`) belong to the plugin, so they never clash with a `FileProvider` the app or another plugin declares. `res/xml/share_sheet_file_paths.xml` exposes only the cache (`<cache-path>`) and files (`<files-path>`) directories. The path is checked three times: in Rust (lexically and after resolving symbolic links), in Kotlin against the canonical path, and by `FileProvider` itself. The chosen target gets a one-off read grant on that one content URI.

### Desktop paths

On macOS and Windows any existing regular file the app can read is accepted; nothing is shared until the person picks a target in the system UI.

## Manual test checklist

None of the native share paths has been run on a device yet. Before relying on a platform, work through its list with a `.zip` written to the app cache directory, and with a short text.

**Android phone**

- [ ] `canShare()` reports `{ supported: true, files: true, text: true }`.
- [ ] `shareFile` opens the share sheet with the file's name in the preview; sending it to Files, Drive, Gmail or Nearby Share delivers a valid `.zip`.
- [ ] Picking an app resolves `shared`; backing out of the sheet resolves `unknown` within about half a second; "Copy" resolves `unknown`.
- [ ] The receiving app can read the file without the game holding any storage permission; the merged manifest has no new `<uses-permission>`.
- [ ] A path under `appDataDir()` (outside `files/`) or with `../` rejects with `invalid-path`; a missing file in the cache rejects with `not-found`.
- [ ] `shareText` sends the text to a messaging app, with the title as the subject in a mail app.
- [ ] Two quick taps on Share leave only one sheet open and both promises settle.
- [ ] Android 13 or later and Android 12 or earlier both report `shared` (the receiver is registered differently from Android 13).

**iOS device (iPhone and iPad)**

- [ ] The share sheet opens over the game with the title in its header.
- [ ] Save to Files, AirDrop and Mail deliver a valid `.zip`; completing one resolves `shared`.
- [ ] Closing the sheet resolves `cancelled`.
- [ ] On iPad the sheet appears as a popover in the middle of the window, with no crash.
- [ ] `shareText` works in Messages and Notes.

**Mac**

- [ ] The picker opens over the calling window.
- [ ] Choosing AirDrop, Mail or Messages delivers the file and resolves `shared`; Mail's subject is the title.
- [ ] Clicking away from the picker resolves `cancelled`.
- [ ] `shareText` offers text targets such as Notes and Messages.

**Windows PC (Windows 10 and 11)**

- [ ] `canShare()` reports support on Windows 10 1709 or later.
- [ ] The share UI opens over the game window with the title; Nearby Sharing, Mail or another app receives a valid `.zip`.
- [ ] The promise resolves `unknown` as soon as the UI is up, and sharing a second file after the first shows the second file, not both.
- [ ] `shareText` sends text to Mail or another app.
- [ ] Logs show the `untested native path` steps in order, with no error.

## Permissions

| Permission         | Command      |
| ------------------ | ------------ |
| `allow-share-file` | `share_file` |
| `allow-share-text` | `share_text` |
| `allow-can-share`  | `can_share`  |

The `default` set allows all three.

## Development

Argument checks, path containment, MIME and title handling, error mapping and each platform's capabilities live in platform-independent modules (`src/validate.rs`, `src/error.rs`, `src/models.rs`) with unit tests that run on any host; `tests/commands.rs` drives the plugin through Tauri's mock runtime on Linux. The native code is in `src/android.rs` with `android/`, `src/ios.rs` with `ios/`, `src/macos.rs` and `src/windows.rs`. To regenerate the TypeScript bindings in `guest-js/bindings`, run `cargo test` in the plugin directory.

## Logging

The plugin logs through the `log` crate. The macOS and Windows paths have only been type-checked, so they log at `info` before each native step, on lines containing `untested native path`; if the process crashes inside one, the last such line names the step. The Android plugin logs under the `ShareSheetPlugin` tag.

## Licence

Licensed under either of Apache License 2.0 or MIT licence at your option.
