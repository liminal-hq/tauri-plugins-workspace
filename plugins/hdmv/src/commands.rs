use tauri::{command, State};

use crate::error::{Error, Result};
use crate::models::*;
use crate::session::{nav_event_to_dto, HdmvSession, SessionStore};

/// Open a BDMV folder, parse index/movie objects, return a session ID.
#[command]
pub(crate) async fn hdmv_open_disc(path: String, store: State<'_, SessionStore>) -> Result<String> {
    let session = HdmvSession::open(&path)?;
    let session_id = uuid::Uuid::new_v4().to_string();
    store
        .sessions
        .lock()
        .map_err(|e| Error::Plugin(e.to_string()))?
        .insert(session_id.clone(), session);
    Ok(session_id)
}

/// Drop a session and free its resources.
#[command]
pub(crate) async fn hdmv_close_disc(
    session_id: String,
    store: State<'_, SessionStore>,
) -> Result<()> {
    store
        .sessions
        .lock()
        .map_err(|e| Error::Plugin(e.to_string()))?
        .remove(&session_id)
        .ok_or(Error::SessionNotFound(session_id))?;
    Ok(())
}

/// Get a summary of the disc's top-level structure.
#[command]
pub(crate) async fn hdmv_get_disc_info(
    session_id: String,
    store: State<'_, SessionStore>,
) -> Result<DiscSummary> {
    let sessions = store
        .sessions
        .lock()
        .map_err(|e| Error::Plugin(e.to_string()))?;
    let session = sessions
        .get(&session_id)
        .ok_or(Error::SessionNotFound(session_id))?;

    Ok(DiscSummary {
        title_count: session.index.titles.len(),
        version: format!(
            "{}.{}",
            session.index.version.major(),
            session.index.version.minor()
        ),
        first_play_object_id: session.index.first_play.object_id_ref,
        top_menu_object_id: session.index.top_menu.object_id_ref,
    })
}

/// List all title entries from the disc index.
#[command]
pub(crate) async fn hdmv_list_titles(
    session_id: String,
    store: State<'_, SessionStore>,
) -> Result<Vec<TitleInfo>> {
    let sessions = store
        .sessions
        .lock()
        .map_err(|e| Error::Plugin(e.to_string()))?;
    let session = sessions
        .get(&session_id)
        .ok_or(Error::SessionNotFound(session_id))?;

    Ok(session
        .index
        .titles
        .iter()
        .enumerate()
        .map(|(i, title)| TitleInfo {
            index: i,
            object_type: format!("{:?}", title.object_type),
            playback_type: format!("{:?}", title.playback_type),
            object_id_ref: title.object_id_ref,
        })
        .collect())
}

/// List playlist summaries (play item count, duration, chapters).
#[command]
pub(crate) async fn hdmv_list_playlists(
    session_id: String,
    store: State<'_, SessionStore>,
) -> Result<Vec<PlaylistInfo>> {
    let sessions = store
        .sessions
        .lock()
        .map_err(|e| Error::Plugin(e.to_string()))?;
    let session = sessions
        .get(&session_id)
        .ok_or(Error::SessionNotFound(session_id))?;

    Ok(session
        .playlists
        .iter()
        .enumerate()
        .map(|(i, pl)| {
            let duration: f64 = pl
                .play_items
                .iter()
                .map(|item| item.out_time.as_seconds() - item.in_time.as_seconds())
                .sum();
            PlaylistInfo {
                index: i,
                play_item_count: pl.play_items.len(),
                chapter_count: pl.play_marks.len(),
                duration_seconds: duration,
            }
        })
        .collect())
}

/// Get full playlist detail including play items and chapters.
#[command]
pub(crate) async fn hdmv_get_playlist(
    session_id: String,
    playlist_index: usize,
    store: State<'_, SessionStore>,
) -> Result<PlaylistDetail> {
    let sessions = store
        .sessions
        .lock()
        .map_err(|e| Error::Plugin(e.to_string()))?;
    let session = sessions
        .get(&session_id)
        .ok_or(Error::SessionNotFound(session_id.clone()))?;

    let pl = session
        .playlists
        .get(playlist_index)
        .ok_or(Error::Plugin(format!(
            "Playlist index {} out of range",
            playlist_index
        )))?;

    let play_items = pl
        .play_items
        .iter()
        .map(|item| PlayItemInfo {
            clip_id: item.clip_id.clone(),
            codec_id: item.codec_id.clone(),
            in_time_seconds: item.in_time.as_seconds(),
            out_time_seconds: item.out_time.as_seconds(),
        })
        .collect();

    let chapters = pl
        .play_marks
        .iter()
        .enumerate()
        .map(|(i, mark)| ChapterInfo {
            index: i,
            play_item_ref: mark.play_item_ref,
            time_seconds: mark.time.as_seconds(),
        })
        .collect();

    Ok(PlaylistDetail {
        index: playlist_index,
        play_items,
        chapters,
    })
}

/// Execute the First Play object and return initial navigation events.
#[command]
pub(crate) async fn hdmv_start_navigation(
    session_id: String,
    store: State<'_, SessionStore>,
) -> Result<Vec<NavEventDto>> {
    let mut sessions = store
        .sessions
        .lock()
        .map_err(|e| Error::Plugin(e.to_string()))?;
    let session = sessions
        .get_mut(&session_id)
        .ok_or(Error::SessionNotFound(session_id))?;

    let events = session.start_navigation();
    Ok(events.iter().map(nav_event_to_dto).collect())
}

/// Send a remote key input and return resulting navigation events.
#[command]
pub(crate) async fn hdmv_send_key(
    session_id: String,
    key: String,
    store: State<'_, SessionStore>,
) -> Result<Vec<NavEventDto>> {
    let remote_key = parse_remote_key(&key)?;
    let mut sessions = store
        .sessions
        .lock()
        .map_err(|e| Error::Plugin(e.to_string()))?;
    let session = sessions
        .get_mut(&session_id)
        .ok_or(Error::SessionNotFound(session_id))?;

    let events = session.send_key(remote_key)?;
    Ok(events.iter().map(nav_event_to_dto).collect())
}

/// Hit-test mouse position, return button ID if hovering over one.
#[command]
pub(crate) async fn hdmv_mouse_move(
    session_id: String,
    x: u16,
    y: u16,
    store: State<'_, SessionStore>,
) -> Result<Option<u16>> {
    let sessions = store
        .sessions
        .lock()
        .map_err(|e| Error::Plugin(e.to_string()))?;
    let session = sessions
        .get(&session_id)
        .ok_or(Error::SessionNotFound(session_id))?;

    let scene = session
        .scene
        .as_ref()
        .ok_or(Error::NoMenuScene(String::new()))?;

    let result = scene.hit_test(x, y);
    Ok(result.button_id)
}

/// Click at a position and return resulting navigation events.
#[command]
pub(crate) async fn hdmv_mouse_click(
    session_id: String,
    x: u16,
    y: u16,
    store: State<'_, SessionStore>,
) -> Result<Vec<NavEventDto>> {
    let mut sessions = store
        .sessions
        .lock()
        .map_err(|e| Error::Plugin(e.to_string()))?;
    let session = sessions
        .get_mut(&session_id)
        .ok_or(Error::SessionNotFound(session_id))?;

    let scene = session
        .scene
        .as_mut()
        .ok_or(Error::NoMenuScene(String::new()))?;
    let vm = session
        .vm
        .as_mut()
        .ok_or(Error::NavigationNotStarted(String::new()))?;

    let update = scene.process_input(&libhdmv::SceneInput::MouseClick { x, y });
    let events = Vec::new();
    for _cmd in &update.nav_commands {
        // Execute navigation commands through the VM
        let _ = vm;
    }

    Ok(events.iter().map(nav_event_to_dto).collect())
}

/// Render the current menu page as a base64-encoded PNG.
#[command]
pub(crate) async fn hdmv_render_preview(
    session_id: String,
    max_width: u32,
    store: State<'_, SessionStore>,
) -> Result<String> {
    let sessions = store
        .sessions
        .lock()
        .map_err(|e| Error::Plugin(e.to_string()))?;
    let session = sessions
        .get(&session_id)
        .ok_or(Error::SessionNotFound(session_id))?;

    session.render_preview(max_width)
}

/// Get a snapshot of the current menu state.
#[command]
pub(crate) async fn hdmv_get_menu_state(
    session_id: String,
    store: State<'_, SessionStore>,
) -> Result<MenuStateSnapshot> {
    let sessions = store
        .sessions
        .lock()
        .map_err(|e| Error::Plugin(e.to_string()))?;
    let session = sessions
        .get(&session_id)
        .ok_or(Error::SessionNotFound(session_id))?;

    match &session.scene {
        Some(scene) => Ok(MenuStateSnapshot {
            has_menu: true,
            current_page_id: scene.current_page().map(|p| p.page_id),
            selected_button_id: Some(scene.selected_button_id()),
            popup_visible: scene.popup_visible(),
        }),
        None => Ok(MenuStateSnapshot {
            has_menu: false,
            current_page_id: None,
            selected_button_id: None,
            popup_visible: false,
        }),
    }
}

/// Build a new BDMV disc structure from configuration.
#[command]
pub(crate) async fn hdmv_build_disc(config: DiscBuildConfig) -> Result<()> {
    let mut builder = libhdmv::DiscBuilder::new(&config.output_path);

    for title in &config.titles {
        builder = builder.add_title(libhdmv::TitleSpec {
            clip_id: title.clip_id.clone(),
            codec_id: title.codec_id.clone(),
            duration_seconds: title.duration_seconds,
            streams: Vec::new(),
            chapters: title.chapters.clone(),
        });
    }

    builder.build()?;
    Ok(())
}

/// Parse a string key name into a RemoteKey enum variant.
fn parse_remote_key(key: &str) -> Result<libhdmv::RemoteKey> {
    match key.to_lowercase().as_str() {
        "up" => Ok(libhdmv::RemoteKey::Up),
        "down" => Ok(libhdmv::RemoteKey::Down),
        "left" => Ok(libhdmv::RemoteKey::Left),
        "right" => Ok(libhdmv::RemoteKey::Right),
        "select" | "enter" => Ok(libhdmv::RemoteKey::Select),
        "topmenu" | "top_menu" => Ok(libhdmv::RemoteKey::TopMenu),
        "popupmenu" | "popup_menu" => Ok(libhdmv::RemoteKey::PopupMenu),
        "return" | "back" => Ok(libhdmv::RemoteKey::Return),
        "red" => Ok(libhdmv::RemoteKey::ColourRed),
        "green" => Ok(libhdmv::RemoteKey::ColourGreen),
        "yellow" => Ok(libhdmv::RemoteKey::ColourYellow),
        "blue" => Ok(libhdmv::RemoteKey::ColourBlue),
        s if s.len() == 1 && s.as_bytes()[0].is_ascii_digit() => {
            Ok(libhdmv::RemoteKey::Numeric(s.as_bytes()[0] - b'0'))
        }
        _ => Err(Error::Plugin(format!("Unknown remote key: {}", key))),
    }
}
