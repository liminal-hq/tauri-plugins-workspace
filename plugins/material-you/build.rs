const COMMANDS: &[&str] = &["get_material_you_colours"];

fn main() {
    tauri_plugin::Builder::new(COMMANDS)
        .global_api_script_path("./api-iife.js")
        .android_path("android")
        .build();
}
