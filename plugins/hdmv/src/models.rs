use serde::{Deserialize, Serialize};

/// Summary of a disc's top-level structure.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DiscSummary {
    pub title_count: usize,
    pub version: String,
    pub first_play_object_id: u16,
    pub top_menu_object_id: u16,
}

/// Entry from the disc index title table.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TitleInfo {
    pub index: usize,
    pub object_type: String,
    pub playback_type: String,
    pub object_id_ref: u16,
}

/// Summary of a playlist.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PlaylistInfo {
    pub index: usize,
    pub play_item_count: usize,
    pub chapter_count: usize,
    pub duration_seconds: f64,
}

/// Detailed playlist information including streams and chapters.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PlaylistDetail {
    pub index: usize,
    pub play_items: Vec<PlayItemInfo>,
    pub chapters: Vec<ChapterInfo>,
}

/// A single play item within a playlist.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PlayItemInfo {
    pub clip_id: String,
    pub codec_id: String,
    pub in_time_seconds: f64,
    pub out_time_seconds: f64,
}

/// A chapter (play mark) within a playlist.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ChapterInfo {
    pub index: usize,
    pub play_item_ref: u16,
    pub time_seconds: f64,
}

/// Snapshot of the current menu state.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MenuStateSnapshot {
    pub has_menu: bool,
    pub current_page_id: Option<u8>,
    pub selected_button_id: Option<u16>,
    pub popup_visible: bool,
}

/// A navigation event emitted by the VM.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
#[serde(tag = "type")]
pub enum NavEventDto {
    PlayTitle { title_id: u16 },
    PlayPlaylist { playlist_id: u16 },
    PlayPlaylistItem { playlist_id: u16, play_item_id: u16 },
    SeekPlayMark { playlist_id: u16, play_mark_id: u16 },
    LinkPlayItem { play_item_id: u16 },
    LinkPlayMark { play_mark_id: u16 },
    PlayStop,
    StillOn,
    StillOff,
    SetButtonPage { page_id: u16 },
    EnableButton { button_id: u16 },
    DisableButton { button_id: u16 },
    PopupOff,
    SetOutputMode { mode: u32 },
    SetStream { stream_type: u8, stream_id: u16 },
    SetNvTimer { timer_id: u32, value: u32 },
}

/// Configuration for building a new BDMV disc structure.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DiscBuildConfig {
    pub output_path: String,
    pub titles: Vec<TitleSpecDto>,
}

/// Title specification for disc building.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TitleSpecDto {
    pub clip_id: String,
    pub codec_id: String,
    pub duration_seconds: u32,
    pub chapters: Vec<u32>,
}
