// Generates plugin metadata and permission manifests for commands
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

const COMMANDS: &[&str] = &[
    "check_availability",
    "get_theme_info",
    "get_status",
    "send_notification",
    "withdraw_notification",
    "inhibit",
    "release_inhibit",
    "open_uri",
];

fn main() {
    tauri_plugin::Builder::new(COMMANDS)
        .global_api_script_path("./api-iife.js")
        .build();
}
