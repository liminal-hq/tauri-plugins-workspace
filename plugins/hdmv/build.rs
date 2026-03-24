const COMMANDS: &[&str] = &[
    "hdmv_open_disc",
    "hdmv_close_disc",
    "hdmv_get_disc_info",
    "hdmv_list_titles",
    "hdmv_list_playlists",
    "hdmv_get_playlist",
    "hdmv_start_navigation",
    "hdmv_send_key",
    "hdmv_mouse_move",
    "hdmv_mouse_click",
    "hdmv_render_preview",
    "hdmv_get_menu_state",
    "hdmv_build_disc",
];

fn main() {
    tauri_plugin::Builder::new(COMMANDS)
        .global_api_script_path("./api-iife.js")
        .build();
}
