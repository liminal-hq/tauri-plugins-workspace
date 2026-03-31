use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Mutex;

use libhdmv::{
    BdmvLayout, BdmvSource, CpuRenderer, DiscIndex, FolderSource, MenuScene, MovieObjectFile,
    NavEvent, Playlist, VmSession,
};

use crate::error::{Error, Result};
use crate::models::NavEventDto;

/// Holds all state for a single open disc.
#[allow(dead_code)]
pub struct HdmvSession {
    pub source: FolderSource,
    pub layout: BdmvLayout,
    pub index: DiscIndex,
    pub movie_objects: MovieObjectFile,
    pub playlists: Vec<Playlist>,
    pub vm: Option<VmSession>,
    pub scene: Option<MenuScene>,
    /// Decoded IGS objects for rendering, stored alongside the scene.
    pub ig_objects: Vec<libhdmv::igs::IgObject>,
    /// Decoded IGS palettes for rendering.
    pub ig_palettes: Vec<libhdmv::igs::IgPalette>,
}

impl HdmvSession {
    /// Open a BDMV folder and parse the core metadata.
    pub fn open(path: &str) -> Result<Self> {
        let source = FolderSource::new(PathBuf::from(path));
        let layout = BdmvLayout::discover(&source)?;

        let index_data = source.read_file(&layout.index)?;
        let index = DiscIndex::parse(&index_data).map_err(|e| Error::Parse(e.to_string()))?;

        let mobj_data = source.read_file(&layout.movie_object)?;
        let movie_objects =
            MovieObjectFile::parse(&mobj_data).map_err(|e| Error::Parse(e.to_string()))?;

        // Parse all playlists
        let mut playlists = Vec::new();
        for playlist_path in &layout.playlists {
            let data = source.read_file(playlist_path)?;
            let playlist = Playlist::parse(&data).map_err(|e| Error::Parse(e.to_string()))?;
            playlists.push(playlist);
        }

        Ok(Self {
            source,
            layout,
            index,
            movie_objects,
            playlists,
            vm: None,
            scene: None,
            ig_objects: Vec::new(),
            ig_palettes: Vec::new(),
        })
    }

    /// Create a VM session and execute the First Play object.
    ///
    /// Processes the resulting nav events to initialise the menu scene
    /// when IGS data has been loaded via `load_scene`.
    pub fn start_navigation(&mut self) -> Vec<NavEvent> {
        let commands: Vec<Vec<[u8; 12]>> = self
            .movie_objects
            .objects
            .iter()
            .map(|obj| obj.commands.iter().map(|cmd| cmd.bytes).collect())
            .collect();
        let mut vm = VmSession::new(commands);
        let events = vm.execute_object(self.index.first_play.object_id_ref as usize);

        // Process SetButtonPage events to update the scene if one is loaded
        if let Some(scene) = &mut self.scene {
            for event in &events {
                if let NavEvent::SetButtonPage { page_id } = event {
                    scene.set_page(*page_id as u8);
                }
            }
        }

        self.vm = Some(vm);
        events
    }

    /// Load a menu scene from an `InteractiveComposition` and its associated objects.
    ///
    /// This must be called before scene-dependent commands (`send_key`, `mouse_move`,
    /// `mouse_click`, `render_preview`) will work. IGS composition data must be
    /// extracted from the disc's transport streams externally, as libhdmv does not
    /// yet include a transport stream demuxer.
    pub fn load_scene(
        &mut self,
        composition: libhdmv::igs::InteractiveComposition,
        objects: Vec<libhdmv::igs::IgObject>,
        palettes: Vec<libhdmv::igs::IgPalette>,
    ) {
        self.scene = Some(MenuScene::with_objects(composition, &objects));
        self.ig_objects = objects;
        self.ig_palettes = palettes;
    }

    /// Send a remote key to the VM via the menu scene.
    pub fn send_key(&mut self, key: libhdmv::RemoteKey) -> Result<Vec<NavEvent>> {
        let scene = self
            .scene
            .as_mut()
            .ok_or(Error::NoMenuScene(String::new()))?;

        let update = scene.process_input(&libhdmv::SceneInput::Key(key));
        let events = execute_nav_commands(&update.nav_commands);
        Ok(events)
    }

    /// Process a mouse click and return resulting navigation events.
    pub fn mouse_click(&mut self, x: u16, y: u16) -> Result<Vec<NavEvent>> {
        let scene = self
            .scene
            .as_mut()
            .ok_or(Error::NoMenuScene(String::new()))?;

        let update = scene.process_input(&libhdmv::SceneInput::MouseClick { x, y });
        let events = execute_nav_commands(&update.nav_commands);
        Ok(events)
    }

    /// Render the current menu page as a PNG and return base64-encoded data.
    pub fn render_preview(&self, max_width: u32) -> Result<String> {
        let scene = self
            .scene
            .as_ref()
            .ok_or(Error::NoMenuScene(String::new()))?;

        let page = scene
            .current_page()
            .ok_or(Error::Plugin("No current page".into()))?;

        let renderer = CpuRenderer::new(max_width, max_width * 9 / 16);

        // Build palette from stored IG palettes
        let palette = if let Some(ig_palette) = self
            .ig_palettes
            .iter()
            .find(|p| p.palette_id == page.palette_id)
        {
            libhdmv::pgs::PgsPalette::from_entries(
                &ig_palette
                    .entries
                    .iter()
                    .map(|e| libhdmv::pgs::PaletteEntry {
                        id: e.id,
                        y: e.y,
                        cr: e.cr,
                        cb: e.cb,
                        alpha: e.alpha,
                    })
                    .collect::<Vec<_>>(),
            )
        } else {
            libhdmv::pgs::PgsPalette::new()
        };

        let frame = renderer.render_page_with_selection(
            page,
            &self.ig_objects,
            &palette,
            scene.selected_button_id(),
        );

        encode_frame_as_base64_png(&frame)
    }
}

/// Execute button navigation commands by creating a temporary VM.
///
/// Button nav commands are raw 12-byte HDMV instructions. We execute them
/// in a temporary VM session to produce navigation events.
fn execute_nav_commands(commands: &[[u8; 12]]) -> Vec<NavEvent> {
    if commands.is_empty() {
        return Vec::new();
    }
    let mut temp_vm = VmSession::new(vec![commands.to_vec()]);
    temp_vm.execute_object(0)
}

/// Encode an OverlayFrame as a base64-encoded PNG string.
fn encode_frame_as_base64_png(frame: &libhdmv::OverlayFrame) -> Result<String> {
    use base64::Engine;

    let mut buf = Vec::new();
    {
        let mut encoder = png::Encoder::new(&mut buf, frame.width, frame.height);
        encoder.set_color(png::ColorType::Rgba);
        encoder.set_depth(png::BitDepth::Eight);
        let mut writer = encoder
            .write_header()
            .map_err(|e| Error::Plugin(e.to_string()))?;
        writer
            .write_image_data(&frame.data)
            .map_err(|e| Error::Plugin(e.to_string()))?;
    }

    Ok(base64::engine::general_purpose::STANDARD.encode(&buf))
}

/// Convert libhdmv NavEvent to our serialisable DTO.
pub fn nav_event_to_dto(event: &NavEvent) -> NavEventDto {
    match event {
        NavEvent::PlayTitle { title_id } => NavEventDto::PlayTitle {
            title_id: *title_id,
        },
        NavEvent::PlayPlaylist { playlist_id } => NavEventDto::PlayPlaylist {
            playlist_id: *playlist_id,
        },
        NavEvent::PlayPlaylistItem {
            playlist_id,
            play_item_id,
        } => NavEventDto::PlayPlaylistItem {
            playlist_id: *playlist_id,
            play_item_id: *play_item_id,
        },
        NavEvent::SeekPlayMark {
            playlist_id,
            play_mark_id,
        } => NavEventDto::SeekPlayMark {
            playlist_id: *playlist_id,
            play_mark_id: *play_mark_id,
        },
        NavEvent::LinkPlayItem { play_item_id } => NavEventDto::LinkPlayItem {
            play_item_id: *play_item_id,
        },
        NavEvent::LinkPlayMark { play_mark_id } => NavEventDto::LinkPlayMark {
            play_mark_id: *play_mark_id,
        },
        NavEvent::PlayStop => NavEventDto::PlayStop,
        NavEvent::StillOn => NavEventDto::StillOn,
        NavEvent::StillOff => NavEventDto::StillOff,
        NavEvent::SetButtonPage { page_id } => NavEventDto::SetButtonPage { page_id: *page_id },
        NavEvent::EnableButton { button_id } => NavEventDto::EnableButton {
            button_id: *button_id,
        },
        NavEvent::DisableButton { button_id } => NavEventDto::DisableButton {
            button_id: *button_id,
        },
        NavEvent::PopupOff => NavEventDto::PopupOff,
        NavEvent::SetOutputMode { mode } => NavEventDto::SetOutputMode { mode: *mode },
        NavEvent::SetStream {
            stream_type,
            stream_id,
        } => NavEventDto::SetStream {
            stream_type: *stream_type,
            stream_id: *stream_id,
        },
        NavEvent::SetNvTimer { timer_id, value } => NavEventDto::SetNvTimer {
            timer_id: *timer_id,
            value: *value,
        },
    }
}

/// Thread-safe session store managed as Tauri state.
pub struct SessionStore {
    pub sessions: Mutex<HashMap<String, HdmvSession>>,
}

impl SessionStore {
    pub fn new() -> Self {
        Self {
            sessions: Mutex::new(HashMap::new()),
        }
    }
}
