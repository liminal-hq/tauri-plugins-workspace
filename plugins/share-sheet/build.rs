// Generates the plugin's command permissions and wires the Android and iOS projects
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

const COMMANDS: &[&str] = &["share_file", "share_text", "can_share"];

fn main() {
    tauri_plugin::Builder::new(COMMANDS)
        .android_path("android")
        .ios_path("ios")
        .build();

    inject_android_permissions()
        .expect("Failed to inject Android manifest permissions for share-sheet");
}

fn inject_android_permissions() -> std::io::Result<()> {
    // The share sheet needs no Android permissions: the FileProvider grants the chosen target
    // read access to one file at a time. The block keeps the injection mechanism in place.
    tauri_plugin::mobile::update_android_manifest(
        "tauri-plugin-share-sheet.permissions",
        "manifest",
        String::new(),
    )
    .map_err(std::io::Error::other)
}
