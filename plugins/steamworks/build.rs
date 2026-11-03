// Registers the plugin's commands and detects whether the Steamworks SDK is linked
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

const COMMANDS: &[&str] = &[
    "is_available",
    "get_status",
    "get_player",
    "get_achievement",
    "unlock_achievement",
    "clear_achievement",
    "indicate_achievement_progress",
    "get_stat",
    "set_stat",
    "store_stats",
    "set_rich_presence",
    "clear_rich_presence",
    "activate_overlay",
    "activate_overlay_to_web_page",
];

fn main() {
    // `steam_sdk` is set when the `steam` feature is on and the target can link the SDK. Mobile
    // targets never link it, so shared codebases can enable the feature unconditionally.
    println!("cargo::rustc-check-cfg=cfg(steam_sdk)");
    let os = std::env::var("CARGO_CFG_TARGET_OS").unwrap_or_default();
    let desktop = !matches!(os.as_str(), "android" | "ios");
    if std::env::var_os("CARGO_FEATURE_STEAM").is_some() && desktop {
        println!("cargo::rustc-cfg=steam_sdk");
    }

    tauri_plugin::Builder::new(COMMANDS)
        .global_api_script_path("./api-iife.js")
        .build();
}
