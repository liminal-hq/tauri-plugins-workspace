// Provides in-memory lobbies, Workshop items and Steam Input for the mock backend
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

use std::collections::BTreeMap;

use super::MockBackend;
use crate::{
    backend::{BackendReason, Done},
    error::{Error, Result},
    input::{
        ActionKind, AnalogActionState, DigitalActionState, GlyphSize, InputBackend, InputController,
    },
    lobby::{LobbyBackend, LobbyInfo, LobbyKind, LobbyMemberChange},
    models::SteamEvent,
    workshop::{WorkshopBackend, WorkshopItem},
};

/// A 1x1 transparent PNG, standing in for a button glyph.
const GLYPH_PNG: &[u8] = &[
    0x89, 0x50, 0x4e, 0x47, 0x0d, 0x0a, 0x1a, 0x0a, 0x00, 0x00, 0x00, 0x0d, 0x49, 0x48, 0x44, 0x52,
    0x00, 0x00, 0x00, 0x01, 0x00, 0x00, 0x00, 0x01, 0x08, 0x06, 0x00, 0x00, 0x00, 0x1f, 0x15, 0xc4,
    0x89, 0x00, 0x00, 0x00, 0x0d, 0x49, 0x44, 0x41, 0x54, 0x78, 0x9c, 0x63, 0x00, 0x01, 0x00, 0x00,
    0x05, 0x00, 0x01, 0x0d, 0x0a, 0x2d, 0xb4, 0x00, 0x00, 0x00, 0x00, 0x49, 0x45, 0x4e, 0x44, 0xae,
    0x42, 0x60, 0x82,
];

struct Lobby {
    kind: LobbyKind,
    max_members: u32,
    owner: u64,
    members: Vec<u64>,
    data: BTreeMap<String, String>,
    joinable: bool,
}

#[derive(Default)]
pub(super) struct Social {
    next_lobby: u64,
    lobbies: BTreeMap<u64, Lobby>,
    workshop: BTreeMap<u64, WorkshopItem>,
    input_ready: bool,
    next_handle: u64,
    action_sets: BTreeMap<String, u64>,
    actions: BTreeMap<(ActionKind, String), u64>,
    digital: BTreeMap<(u64, u64), bool>,
    analog: BTreeMap<(u64, u64), (f32, f32)>,
    active_sets: BTreeMap<u64, u64>,
}

impl Social {
    pub(super) fn new(workshop: &[WorkshopItem]) -> Self {
        Self {
            next_lobby: 109_775_240_000_000_001,
            next_handle: 1,
            workshop: workshop
                .iter()
                .filter_map(|item| Some((item.id.parse().ok()?, item.clone())))
                .collect(),
            ..Self::default()
        }
    }

    fn handle(&mut self) -> u64 {
        let handle = self.next_handle;
        self.next_handle += 1;
        handle
    }
}

fn unknown_lobby(id: u64) -> Error {
    Error::Steam(format!("unknown lobby {id}"))
}

impl MockBackend {
    fn me(&self) -> u64 {
        self.config.player.steam_id.parse().unwrap_or(1)
    }

    fn social(&self) -> std::sync::MutexGuard<'_, Social> {
        self.social.lock().unwrap_or_else(|e| e.into_inner())
    }

    fn lobby_snapshot(id: u64, lobby: &Lobby) -> LobbyInfo {
        LobbyInfo {
            id: id.to_string(),
            owner: lobby.owner.to_string(),
            members: lobby.members.iter().map(u64::to_string).collect(),
            member_limit: Some(lobby.max_members),
            data: lobby.data.clone(),
        }
    }

    /// Makes another player enter or leave a lobby, as Steam reports when it happens remotely.
    pub fn simulate_lobby_member(&self, lobby: u64, member: u64, change: LobbyMemberChange) {
        {
            let mut social = self.social();
            let Some(entry) = social.lobbies.get_mut(&lobby) else {
                return;
            };
            match change {
                LobbyMemberChange::Entered => {
                    if !entry.members.contains(&member) {
                        entry.members.push(member);
                    }
                }
                _ => entry.members.retain(|m| *m != member),
            }
        }
        (self.emit)(SteamEvent::LobbyChatUpdate {
            lobby_id: lobby.to_string(),
            user_changed: member.to_string(),
            making_change: member.to_string(),
            change,
        });
    }

    /// Finishes installing a subscribed Workshop item, as Steam does once the download ends.
    pub fn finish_workshop_install(&self, id: u64, path: &str, size_on_disk: u64) {
        if let Some(item) = self.social().workshop.get_mut(&id) {
            item.installed = true;
            item.downloading = false;
            item.needs_update = false;
            item.install_path = Some(path.to_string());
            item.size_on_disk = Some(size_on_disk);
        }
    }

    /// Presses or releases a digital action, for tests that read input.
    pub fn set_digital_action(&self, controller: u64, action: u64, pressed: bool) {
        self.social().digital.insert((controller, action), pressed);
    }

    /// Moves an analog action, for tests that read input.
    pub fn set_analog_action(&self, controller: u64, action: u64, x: f32, y: f32) {
        self.social().analog.insert((controller, action), (x, y));
    }

    fn input_checked(&self) -> Result<std::sync::MutexGuard<'_, Social>> {
        let social = self.social();
        if social.input_ready {
            Ok(social)
        } else {
            Err(Error::Steam(
                "call input_init before using Steam Input".into(),
            ))
        }
    }

    fn known_controller(&self, handle: u64) -> bool {
        self.config
            .controllers
            .iter()
            .any(|c| c.handle.parse() == Ok(handle))
    }
}

impl BackendReason for MockBackend {}

impl LobbyBackend for MockBackend {
    fn create_lobby(&self, kind: LobbyKind, max_members: u32, done: Done<String>) {
        let me = self.me();
        let id = {
            let mut social = self.social();
            let id = social.next_lobby;
            social.next_lobby += 1;
            social.lobbies.insert(
                id,
                Lobby {
                    kind,
                    max_members,
                    owner: me,
                    members: vec![me],
                    data: BTreeMap::new(),
                    joinable: true,
                },
            );
            id
        };
        done(Ok(id.to_string()))
    }

    fn join_lobby(&self, id: u64, done: Done<String>) {
        let me = self.me();
        let result = {
            let mut social = self.social();
            match social.lobbies.get_mut(&id) {
                None => Err(unknown_lobby(id)),
                Some(lobby) if !lobby.joinable => Err(Error::Steam("the lobby is closed".into())),
                Some(lobby) if lobby.members.contains(&me) => Ok(id.to_string()),
                Some(lobby) if lobby.members.len() as u32 >= lobby.max_members => {
                    Err(Error::Steam("the lobby is full".into()))
                }
                Some(lobby) => {
                    lobby.members.push(me);
                    Ok(id.to_string())
                }
            }
        };
        done(result)
    }

    fn list_lobbies(&self, done: Done<Vec<LobbyInfo>>) {
        let list = self
            .social()
            .lobbies
            .iter()
            .filter(|(_, lobby)| lobby.kind == LobbyKind::Public && lobby.joinable)
            .map(|(id, lobby)| Self::lobby_snapshot(*id, lobby))
            .collect();
        done(Ok(list))
    }

    fn leave_lobby(&self, id: u64) -> Result<()> {
        let me = self.me();
        let mut social = self.social();
        let lobby = social
            .lobbies
            .get_mut(&id)
            .ok_or_else(|| unknown_lobby(id))?;
        lobby.members.retain(|m| *m != me);
        match lobby.members.first().copied() {
            None => {
                social.lobbies.remove(&id);
            }
            Some(next) if lobby.owner == me => lobby.owner = next,
            Some(_) => {}
        }
        Ok(())
    }

    fn lobby_info(&self, id: u64) -> Result<LobbyInfo> {
        let social = self.social();
        let lobby = social.lobbies.get(&id).ok_or_else(|| unknown_lobby(id))?;
        Ok(Self::lobby_snapshot(id, lobby))
    }

    fn set_lobby_data(&self, id: u64, key: &str, value: Option<&str>) -> Result<()> {
        let me = self.me();
        {
            let mut social = self.social();
            let lobby = social
                .lobbies
                .get_mut(&id)
                .ok_or_else(|| unknown_lobby(id))?;
            if lobby.owner != me {
                return Err(Error::Steam(
                    "only the lobby owner can change its data".into(),
                ));
            }
            match value {
                Some(value) => {
                    lobby.data.insert(key.to_string(), value.to_string());
                }
                None => {
                    lobby.data.remove(key);
                }
            }
        }
        (self.emit)(SteamEvent::LobbyDataUpdate {
            lobby_id: id.to_string(),
            member_id: id.to_string(),
            success: true,
        });
        Ok(())
    }

    fn set_lobby_joinable(&self, id: u64, joinable: bool) -> Result<()> {
        let me = self.me();
        let mut social = self.social();
        let lobby = social
            .lobbies
            .get_mut(&id)
            .ok_or_else(|| unknown_lobby(id))?;
        if lobby.owner != me {
            return Err(Error::Steam("only the lobby owner can change it".into()));
        }
        lobby.joinable = joinable;
        Ok(())
    }
}

impl WorkshopBackend for MockBackend {
    fn workshop_items(&self) -> Result<Vec<WorkshopItem>> {
        Ok(self.social().workshop.values().cloned().collect())
    }

    fn subscribe_item(&self, id: u64, done: Done<()>) {
        self.social().workshop.insert(
            id,
            WorkshopItem {
                id: id.to_string(),
                subscribed: true,
                installed: false,
                needs_update: false,
                downloading: true,
                install_path: None,
                size_on_disk: None,
                timestamp: None,
            },
        );
        done(Ok(()))
    }

    fn unsubscribe_item(&self, id: u64, done: Done<()>) {
        self.social().workshop.remove(&id);
        done(Ok(()))
    }

    fn download_item(&self, id: u64, _high_priority: bool) -> Result<bool> {
        Ok(self.social().workshop.contains_key(&id))
    }
}

impl InputBackend for MockBackend {
    fn input_init(&self) -> Result<()> {
        self.social().input_ready = true;
        Ok(())
    }

    fn input_controllers(&self) -> Result<Vec<InputController>> {
        drop(self.input_checked()?);
        Ok(self.config.controllers.clone())
    }

    fn input_action_set(&self, name: &str) -> Result<u64> {
        let mut social = self.input_checked()?;
        if let Some(handle) = social.action_sets.get(name) {
            return Ok(*handle);
        }
        let handle = social.handle();
        social.action_sets.insert(name.to_string(), handle);
        Ok(handle)
    }

    fn input_activate_action_set(&self, controller: u64, action_set: u64) -> Result<()> {
        let mut social = self.input_checked()?;
        if !self.known_controller(controller) {
            return Err(Error::Steam("unknown controller".into()));
        }
        social.active_sets.insert(controller, action_set);
        Ok(())
    }

    fn input_action(&self, kind: ActionKind, name: &str) -> Result<u64> {
        let mut social = self.input_checked()?;
        let key = (kind, name.to_string());
        if let Some(handle) = social.actions.get(&key) {
            return Ok(*handle);
        }
        let handle = social.handle();
        social.actions.insert(key, handle);
        Ok(handle)
    }

    fn input_digital_state(&self, controller: u64, action: u64) -> Result<DigitalActionState> {
        let social = self.input_checked()?;
        Ok(DigitalActionState {
            active: self.known_controller(controller),
            pressed: social
                .digital
                .get(&(controller, action))
                .copied()
                .unwrap_or(false),
        })
    }

    fn input_analog_state(&self, controller: u64, action: u64) -> Result<AnalogActionState> {
        let social = self.input_checked()?;
        let (x, y) = social
            .analog
            .get(&(controller, action))
            .copied()
            .unwrap_or((0.0, 0.0));
        Ok(AnalogActionState {
            active: self.known_controller(controller),
            x,
            y,
            mode: "joystickMove".to_string(),
        })
    }

    fn input_glyph(
        &self,
        controller: u64,
        _action_set: u64,
        _kind: ActionKind,
        _action: u64,
        _size: GlyphSize,
    ) -> Result<Vec<u8>> {
        drop(self.input_checked()?);
        if !self.known_controller(controller) {
            return Err(Error::Steam("unknown controller".into()));
        }
        Ok(GLYPH_PNG.to_vec())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{backend::Emit, mock::MockConfig};
    use std::sync::{Arc, Mutex};

    type Events = Arc<Mutex<Vec<SteamEvent>>>;

    fn mock(controllers: Vec<InputController>) -> (MockBackend, Events) {
        let events: Events = Arc::new(Mutex::new(Vec::new()));
        let sink = events.clone();
        let emit: Emit = Arc::new(move |e| sink.lock().unwrap().push(e));
        let config = MockConfig {
            controllers,
            ..MockConfig::default()
        };
        (MockBackend::new(config, emit), events)
    }

    fn lobby_id(mock: &MockBackend, kind: LobbyKind, max: u32) -> u64 {
        let out = Arc::new(Mutex::new(None));
        let slot = out.clone();
        mock.create_lobby(
            kind,
            max,
            Box::new(move |r| *slot.lock().unwrap() = Some(r)),
        );
        let id = out.lock().unwrap().take().unwrap().unwrap();
        id.parse().unwrap()
    }

    #[test]
    fn a_new_lobby_has_its_creator_as_owner_and_member() {
        let (mock, _) = mock(vec![]);
        let id = lobby_id(&mock, LobbyKind::Public, 4);
        let info = mock.lobby_info(id).unwrap();
        assert_eq!(info.owner, mock.config.player.steam_id);
        assert_eq!(info.members, vec![mock.config.player.steam_id.clone()]);
        assert_eq!(info.member_limit, Some(4));
    }

    #[test]
    fn lobby_data_is_owner_only_and_raises_an_update() {
        let (mock, events) = mock(vec![]);
        let id = lobby_id(&mock, LobbyKind::Private, 2);
        mock.set_lobby_data(id, "map", Some("fizzton")).unwrap();
        assert_eq!(mock.lobby_info(id).unwrap().data["map"], "fizzton");
        mock.set_lobby_data(id, "map", None).unwrap();
        assert!(mock.lobby_info(id).unwrap().data.is_empty());
        assert_eq!(events.lock().unwrap().len(), 2);
        assert!(mock.set_lobby_data(999, "k", Some("v")).is_err());
    }

    #[test]
    fn remote_members_raise_chat_updates_and_fill_the_lobby() {
        let (mock, events) = mock(vec![]);
        let id = lobby_id(&mock, LobbyKind::Public, 2);
        mock.simulate_lobby_member(id, 42, LobbyMemberChange::Entered);
        assert_eq!(mock.lobby_info(id).unwrap().members.len(), 2);
        assert!(matches!(
            events.lock().unwrap()[0],
            SteamEvent::LobbyChatUpdate {
                change: LobbyMemberChange::Entered,
                ..
            }
        ));
        mock.simulate_lobby_member(id, 42, LobbyMemberChange::Left);
        assert_eq!(mock.lobby_info(id).unwrap().members.len(), 1);
    }

    fn join(mock: &MockBackend, id: u64) -> bool {
        let outcome = Arc::new(Mutex::new(None));
        let slot = outcome.clone();
        mock.join_lobby(
            id,
            Box::new(move |r| *slot.lock().unwrap() = Some(r.is_ok())),
        );
        let joined = outcome.lock().unwrap().take().unwrap();
        joined
    }

    #[test]
    fn joining_respects_capacity() {
        let (mock, _) = mock(vec![]);
        let id = lobby_id(&mock, LobbyKind::Public, 1);
        assert!(join(&mock, id), "a member can rejoin their own lobby");

        // Another player takes over the only seat once the creator leaves.
        mock.simulate_lobby_member(id, 7, LobbyMemberChange::Entered);
        mock.leave_lobby(id).unwrap();
        assert!(!join(&mock, id), "the lobby is full");
    }

    #[test]
    fn a_closed_lobby_refuses_joins() {
        let (mock, _) = mock(vec![]);
        let id = lobby_id(&mock, LobbyKind::Public, 4);
        mock.set_lobby_joinable(id, false).unwrap();
        assert!(!join(&mock, id));
        mock.set_lobby_joinable(id, true).unwrap();
        assert!(join(&mock, id));
    }

    #[test]
    fn listing_shows_only_open_public_lobbies() {
        let (mock, _) = mock(vec![]);
        let public = lobby_id(&mock, LobbyKind::Public, 4);
        lobby_id(&mock, LobbyKind::Private, 4);
        let listed = Arc::new(Mutex::new(Vec::new()));
        let slot = listed.clone();
        mock.list_lobbies(Box::new(move |r| *slot.lock().unwrap() = r.unwrap()));
        let listed = listed.lock().unwrap();
        assert_eq!(listed.len(), 1);
        assert_eq!(listed[0].id, public.to_string());
    }

    #[test]
    fn leaving_the_last_seat_closes_the_lobby() {
        let (mock, _) = mock(vec![]);
        let id = lobby_id(&mock, LobbyKind::Private, 2);
        mock.leave_lobby(id).unwrap();
        assert!(mock.lobby_info(id).is_err());
        assert!(mock.leave_lobby(id).is_err());
    }

    #[test]
    fn workshop_items_follow_subscribe_install_and_unsubscribe() {
        let (mock, _) = mock(vec![]);
        mock.subscribe_item(55, Box::new(|r| r.unwrap()));
        let items = mock.workshop_items().unwrap();
        assert_eq!(items.len(), 1);
        assert!(items[0].subscribed && items[0].downloading && !items[0].installed);
        assert!(mock.download_item(55, true).unwrap());
        assert!(!mock.download_item(56, true).unwrap());
        mock.finish_workshop_install(55, "/workshop/55", 1024);
        let item = &mock.workshop_items().unwrap()[0];
        assert!(item.installed && !item.downloading);
        assert_eq!(item.install_path.as_deref(), Some("/workshop/55"));
        mock.unsubscribe_item(55, Box::new(|r| r.unwrap()));
        assert!(mock.workshop_items().unwrap().is_empty());
    }

    #[test]
    fn input_needs_init_and_reads_action_state() {
        let pad = InputController {
            handle: "9".into(),
            kind: "steamDeck".into(),
        };
        let (mock, _) = mock(vec![pad.clone()]);
        assert!(mock.input_controllers().is_err(), "before init");
        mock.input_init().unwrap();
        assert_eq!(mock.input_controllers().unwrap(), vec![pad]);

        let set = mock.input_action_set("Gameplay").unwrap();
        assert_eq!(
            mock.input_action_set("Gameplay").unwrap(),
            set,
            "stable handles"
        );
        mock.input_activate_action_set(9, set).unwrap();
        assert!(mock.input_activate_action_set(10, set).is_err());

        let jump = mock.input_action(ActionKind::Digital, "jump").unwrap();
        let walk = mock.input_action(ActionKind::Analog, "walk").unwrap();
        assert_ne!(jump, walk);
        assert!(!mock.input_digital_state(9, jump).unwrap().pressed);
        mock.set_digital_action(9, jump, true);
        mock.set_analog_action(9, walk, 0.5, -1.0);
        assert!(mock.input_digital_state(9, jump).unwrap().pressed);
        let walk_state = mock.input_analog_state(9, walk).unwrap();
        assert_eq!((walk_state.x, walk_state.y), (0.5, -1.0));
    }

    #[test]
    fn glyphs_are_png_bytes_for_known_controllers_only() {
        let pad = InputController {
            handle: "9".into(),
            kind: "ps5".into(),
        };
        let (mock, _) = mock(vec![pad]);
        mock.input_init().unwrap();
        let png = mock
            .input_glyph(9, 1, ActionKind::Digital, 2, GlyphSize::Small)
            .unwrap();
        assert_eq!(&png[..8], &[0x89, b'P', b'N', b'G', 0x0d, 0x0a, 0x1a, 0x0a]);
        assert!(mock
            .input_glyph(8, 1, ActionKind::Digital, 2, GlyphSize::Small)
            .is_err());
    }
}
