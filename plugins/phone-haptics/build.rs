// Declares the haptics plugin commands and wires the Android project
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

const COMMANDS: &[&str] = &["capabilities", "play", "play_steps", "stop", "ui"];

fn main() {
    tauri_plugin::Builder::new(COMMANDS)
        .global_api_script_path("./api-iife.js")
        .android_path("android")
        .build();

    inject_android_permissions()
        .expect("Failed to inject Android manifest permissions for haptics");
}

fn inject_android_permissions() -> std::io::Result<()> {
    tauri_plugin::mobile::update_android_manifest(
        "tauri-plugin-phone-haptics.permissions",
        "manifest",
        r#"<uses-permission android:name="android.permission.VIBRATE" />"#.to_string(),
    )
    .map_err(std::io::Error::other)
}
