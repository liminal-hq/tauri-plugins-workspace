---
'material-you': patch
'material-you-js': patch
'xdg-portal': patch
'xdg-portal-js': patch
'desktop-integration': patch
'desktop-integration-js': patch
'system-appearance': patch
'system-appearance-js': patch
'os-prefs': patch
'os-prefs-js': patch
'window-manager': patch
'window-manager-js': patch
'phone-haptics': patch
'phone-haptics-js': patch
'gamepad-haptics': patch
'gamepad-haptics-js': patch
---

Ships `LICENSE-MIT` and `LICENSE-APACHE` inside each published crate and npm package. The `package.json` `files` lists already named them, but the files were missing from the plugin directories, so the licence texts were not in the packages.
