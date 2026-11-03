// Implements lobbies, Workshop and Steam Input on the real Steam client
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

use std::{collections::BTreeMap, ffi::CStr, sync::atomic::Ordering};

use steamworks::{ItemState, LobbyId, LobbyType, PublishedFileId};

use super::SteamBackend;
use crate::{
    backend::{BackendReason, Done},
    error::{Error, Result},
    input::{
        ActionKind, AnalogActionState, DigitalActionState, GlyphSize, InputBackend, InputController,
    },
    lobby::{LobbyBackend, LobbyInfo, LobbyKind},
    workshop::{WorkshopBackend, WorkshopItem},
};

fn steam_error(error: impl std::fmt::Display) -> Error {
    Error::Steam(error.to_string())
}

fn lobby_snapshot(client: &steamworks::Client, id: LobbyId) -> LobbyInfo {
    let matchmaking = client.matchmaking();
    let mut data = BTreeMap::new();
    for index in 0..matchmaking.lobby_data_count(id) {
        if let Some((key, value)) = matchmaking.lobby_data_by_index(id, index) {
            data.insert(key, value);
        }
    }
    LobbyInfo {
        id: id.raw().to_string(),
        owner: matchmaking.lobby_owner(id).raw().to_string(),
        members: matchmaking
            .lobby_members(id)
            .iter()
            .map(|member| member.raw().to_string())
            .collect(),
        member_limit: matchmaking.lobby_member_limit(id).map(|n| n as u32),
        data,
    }
}

impl BackendReason for SteamBackend {}

impl LobbyBackend for SteamBackend {
    fn create_lobby(&self, kind: LobbyKind, max_members: u32, done: Done<String>) {
        let kind = match kind {
            LobbyKind::Private => LobbyType::Private,
            LobbyKind::FriendsOnly => LobbyType::FriendsOnly,
            LobbyKind::Public => LobbyType::Public,
            LobbyKind::Invisible => LobbyType::Invisible,
        };
        // `max_members` is already within Steam's limit, which `create_lobby` asserts.
        self.client
            .matchmaking()
            .create_lobby(kind, max_members, move |result| {
                done(result.map(|id| id.raw().to_string()).map_err(steam_error))
            });
    }

    fn join_lobby(&self, id: u64, done: Done<String>) {
        self.client
            .matchmaking()
            .join_lobby(LobbyId::from_raw(id), move |result| {
                done(
                    result
                        .map(|id| id.raw().to_string())
                        .map_err(|()| Error::Steam("Steam could not join the lobby".into())),
                )
            });
    }

    fn list_lobbies(&self, done: Done<Vec<LobbyInfo>>) {
        let client = self.client.clone();
        self.client.matchmaking().request_lobby_list(move |result| {
            done(result.map_err(steam_error).map(|ids| {
                ids.into_iter()
                    .map(|id| lobby_snapshot(&client, id))
                    .collect()
            }))
        });
    }

    fn leave_lobby(&self, id: u64) -> Result<()> {
        self.client.matchmaking().leave_lobby(LobbyId::from_raw(id));
        Ok(())
    }

    fn lobby_info(&self, id: u64) -> Result<LobbyInfo> {
        let info = lobby_snapshot(&self.client, LobbyId::from_raw(id));
        // Steam answers an unknown or unjoined lobby with a zero owner and no members.
        if info.members.is_empty() && info.owner == "0" {
            return Err(Error::Steam(format!("unknown lobby {id}")));
        }
        Ok(info)
    }

    fn set_lobby_data(&self, id: u64, key: &str, value: Option<&str>) -> Result<()> {
        let matchmaking = self.client.matchmaking();
        let lobby = LobbyId::from_raw(id);
        let accepted = match value {
            Some(value) => matchmaking.set_lobby_data(lobby, key, value),
            None => matchmaking.delete_lobby_data(lobby, key),
        };
        if accepted {
            Ok(())
        } else {
            Err(Error::Steam(
                "Steam refused the lobby data (only the owner can change it)".into(),
            ))
        }
    }

    fn set_lobby_joinable(&self, id: u64, joinable: bool) -> Result<()> {
        if self
            .client
            .matchmaking()
            .set_lobby_joinable(LobbyId::from_raw(id), joinable)
        {
            Ok(())
        } else {
            Err(Error::Steam(
                "Steam refused the change (only the owner can change it)".into(),
            ))
        }
    }
}

impl WorkshopBackend for SteamBackend {
    fn workshop_items(&self) -> Result<Vec<WorkshopItem>> {
        let ugc = self.client.ugc();
        Ok(ugc
            .subscribed_items(false)
            .into_iter()
            .map(|id| {
                let state = ugc.item_state(id);
                let install = ugc.item_install_info(id);
                WorkshopItem {
                    id: id.0.to_string(),
                    subscribed: state.contains(ItemState::SUBSCRIBED),
                    installed: state.contains(ItemState::INSTALLED),
                    needs_update: state.contains(ItemState::NEEDS_UPDATE),
                    downloading: state
                        .intersects(ItemState::DOWNLOADING | ItemState::DOWNLOAD_PENDING),
                    install_path: install.as_ref().map(|i| i.folder.clone()),
                    size_on_disk: install.as_ref().map(|i| i.size_on_disk),
                    timestamp: install.map(|i| i.timestamp),
                }
            })
            .collect())
    }

    fn subscribe_item(&self, id: u64, done: Done<()>) {
        self.client
            .ugc()
            .subscribe_item(PublishedFileId(id), move |result| {
                done(result.map_err(steam_error))
            });
    }

    fn unsubscribe_item(&self, id: u64, done: Done<()>) {
        self.client
            .ugc()
            .unsubscribe_item(PublishedFileId(id), move |result| {
                done(result.map_err(steam_error))
            });
    }

    fn download_item(&self, id: u64, high_priority: bool) -> Result<bool> {
        Ok(self
            .client
            .ugc()
            .download_item(PublishedFileId(id), high_priority))
    }
}

fn controller_kind(kind: steamworks::InputType) -> &'static str {
    use steamworks::InputType::*;
    match kind {
        SteamController => "steamController",
        XBox360Controller => "xbox360",
        XBoxOneController => "xboxOne",
        GenericGamepad => "genericGamepad",
        PS4Controller => "ps4",
        AppleMFiController => "appleMfi",
        AndroidController => "android",
        SwitchJoyConPair => "switchJoyConPair",
        SwitchJoyConSingle => "switchJoyConSingle",
        SwitchProController => "switchPro",
        MobileTouch => "mobileTouch",
        PS3Controller => "ps3",
        PS5Controller => "ps5",
        SteamDeckController => "steamDeck",
        _ => "unknown",
    }
}

fn source_mode(mode: steamworks_sys::EInputSourceMode) -> &'static str {
    use steamworks_sys::EInputSourceMode::*;
    match mode {
        k_EInputSourceMode_None => "none",
        k_EInputSourceMode_Dpad => "dpad",
        k_EInputSourceMode_Buttons => "buttons",
        k_EInputSourceMode_FourButtons => "fourButtons",
        k_EInputSourceMode_AbsoluteMouse => "absoluteMouse",
        k_EInputSourceMode_RelativeMouse => "relativeMouse",
        k_EInputSourceMode_JoystickMove => "joystickMove",
        k_EInputSourceMode_JoystickMouse => "joystickMouse",
        k_EInputSourceMode_JoystickCamera => "joystickCamera",
        k_EInputSourceMode_ScrollWheel => "scrollWheel",
        k_EInputSourceMode_Trigger => "trigger",
        k_EInputSourceMode_TouchMenu => "touchMenu",
        k_EInputSourceMode_MouseJoystick => "mouseJoystick",
        k_EInputSourceMode_MouseRegion => "mouseRegion",
        k_EInputSourceMode_RadialMenu => "radialMenu",
        k_EInputSourceMode_SingleButton => "singleButton",
        k_EInputSourceMode_Switches => "switches",
        _ => "unknown",
    }
}

impl SteamBackend {
    /// The Steam Input interface, once `input_init` has run.
    fn input(&self) -> Result<steamworks::Input> {
        if self.input_ready.load(Ordering::SeqCst) {
            Ok(self.client.input())
        } else {
            Err(Error::Steam(
                "call input_init before using Steam Input".into(),
            ))
        }
    }
}

impl InputBackend for SteamBackend {
    fn input_init(&self) -> Result<()> {
        // Steam updates Steam Input whenever callbacks run, so the pump drives it.
        if self.client.input().init(false) {
            self.input_ready.store(true, Ordering::SeqCst);
            Ok(())
        } else {
            Err(Error::Steam("Steam Input could not start".into()))
        }
    }

    fn input_controllers(&self) -> Result<Vec<InputController>> {
        let input = self.input()?;
        let mut handles = [0u64; steamworks_sys::STEAM_INPUT_MAX_COUNT as usize];
        let count = input
            .get_connected_controllers_slice(&mut handles)
            .min(handles.len());
        Ok(handles[..count]
            .iter()
            .map(|handle| InputController {
                handle: handle.to_string(),
                kind: controller_kind(input.get_input_type_for_handle(*handle)).to_string(),
            })
            .collect())
    }

    fn input_action_set(&self, name: &str) -> Result<u64> {
        match self.input()?.get_action_set_handle(name) {
            0 => Err(Error::Steam(format!(
                "no action set named `{name}` in the Steam Input manifest"
            ))),
            handle => Ok(handle),
        }
    }

    fn input_activate_action_set(&self, controller: u64, action_set: u64) -> Result<()> {
        self.input()?
            .activate_action_set_handle(controller, action_set);
        Ok(())
    }

    fn input_action(&self, kind: ActionKind, name: &str) -> Result<u64> {
        let input = self.input()?;
        let handle = match kind {
            ActionKind::Digital => input.get_digital_action_handle(name),
            ActionKind::Analog => input.get_analog_action_handle(name),
        };
        if handle == 0 {
            Err(Error::Steam(format!(
                "no action named `{name}` in the Steam Input manifest"
            )))
        } else {
            Ok(handle)
        }
    }

    fn input_digital_state(&self, controller: u64, action: u64) -> Result<DigitalActionState> {
        let data = self.input()?.get_digital_action_data(controller, action);
        Ok(DigitalActionState {
            active: data.bActive,
            pressed: data.bState,
        })
    }

    fn input_analog_state(&self, controller: u64, action: u64) -> Result<AnalogActionState> {
        let data = self.input()?.get_analog_action_data(controller, action);
        Ok(AnalogActionState {
            active: data.bActive,
            x: data.x,
            y: data.y,
            mode: source_mode(data.eMode).to_string(),
        })
    }

    fn input_glyph(
        &self,
        controller: u64,
        action_set: u64,
        kind: ActionKind,
        action: u64,
        size: GlyphSize,
    ) -> Result<Vec<u8>> {
        use steamworks_sys::ESteamInputGlyphSize::*;

        let input = self.input()?;
        let origins = match kind {
            ActionKind::Digital => input.get_digital_action_origins(controller, action_set, action),
            ActionKind::Analog => input.get_analog_action_origins(controller, action_set, action),
        };
        let origin = origins
            .into_iter()
            .find(|origin| *origin as u32 != 0)
            .ok_or_else(|| {
                Error::Steam("that action has no button bound on this controller".into())
            })?;
        let size = match size {
            GlyphSize::Small => k_ESteamInputGlyphSize_Small,
            GlyphSize::Medium => k_ESteamInputGlyphSize_Medium,
            GlyphSize::Large => k_ESteamInputGlyphSize_Large,
        };
        // SAFETY: the interface pointer stays valid while `self.client` is alive. Steam returns
        // a NUL-terminated path that stays valid until the next call, and it is copied at once.
        let path = unsafe {
            let raw = steamworks_sys::SteamAPI_ISteamInput_GetGlyphPNGForActionOrigin(
                steamworks_sys::SteamAPI_SteamInput_v006(),
                origin,
                size,
                0,
            );
            if raw.is_null() {
                String::new()
            } else {
                CStr::from_ptr(raw).to_string_lossy().into_owned()
            }
        };
        if path.is_empty() {
            return Err(Error::Steam("Steam has no glyph for that button".into()));
        }
        std::fs::read(&path).map_err(|e| Error::Steam(format!("could not read the glyph: {e}")))
    }
}
