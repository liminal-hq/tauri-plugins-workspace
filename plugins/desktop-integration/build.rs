// Generates plugin metadata and permission manifests for desktop integration
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

const COMMANDS: &[&str] = &[
    "register_shortcut",
    "check_shortcut_binding_complete",
    "check_shortcut_binding_error",
    "check_shortcut_trigger_description",
    "get_status",
    "notify",
    "withdraw_notification",
    "inhibit_sleep",
    "release_sleep_inhibit",
    "set_launcher_progress",
    "own_file_manager",
    "disown_file_manager",
    "register_global_shortcut",
    "unregister_global_shortcut",
    "set_app_user_model_id",
];

fn main() {
    tauri_plugin::Builder::new(COMMANDS).build();
}
