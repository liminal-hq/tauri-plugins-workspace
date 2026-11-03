// Exposes lobbies, Workshop and Steam Input on the plugin state, with request validation
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

use std::time::Duration;

use super::{check_id, Steamworks};
use crate::{
    backend::{Done, SteamworksBackend},
    error::{Error, Result},
    input::{ActionKind, AnalogActionState, DigitalActionState, GlyphSize, InputController},
    lobby::{LobbyInfo, LobbyKind, MAX_LOBBY_KEY_BYTES, MAX_LOBBY_MEMBERS, MAX_LOBBY_VALUE_BYTES},
    workshop::WorkshopItem,
};

/// How long to wait for Steam to answer a request that completes through a callback.
const REQUEST_TIMEOUT: Duration = Duration::from_secs(30);

/// Reads a Steam ID, Workshop ID or handle sent as a decimal string.
fn parse_id(text: &str, what: &str) -> Result<u64> {
    match text.parse::<u64>() {
        Ok(id) if id != 0 => Ok(id),
        _ => Err(Error::InvalidRequest(format!(
            "{what} must be a non-zero decimal number"
        ))),
    }
}

impl Steamworks {
    /// Starts a request that Steam answers from the callback pump, and waits for the answer.
    async fn request<T: Send + 'static>(
        &self,
        start: impl FnOnce(&dyn SteamworksBackend, Done<T>),
    ) -> Result<T> {
        let (sender, receiver) = tokio::sync::oneshot::channel();
        let backend = self.backend();
        start(
            backend.as_ref(),
            Box::new(move |result| {
                let _ = sender.send(result);
            }),
        );
        match tokio::time::timeout(REQUEST_TIMEOUT, receiver).await {
            Ok(Ok(result)) => result,
            Ok(Err(_)) => Err(Error::Steam("Steam dropped the request".into())),
            Err(_) => Err(Error::Steam("Steam did not answer in time".into())),
        }
    }

    /// Creates a lobby and returns its ID. Steam allows up to 250 members.
    pub async fn create_lobby(&self, kind: LobbyKind, max_members: u32) -> Result<String> {
        if !(1..=MAX_LOBBY_MEMBERS).contains(&max_members) {
            return Err(Error::InvalidRequest(format!(
                "a lobby holds 1 to {MAX_LOBBY_MEMBERS} members"
            )));
        }
        self.request(|backend, done| backend.create_lobby(kind, max_members, done))
            .await
    }

    pub async fn join_lobby(&self, id: &str) -> Result<String> {
        let id = parse_id(id, "a lobby ID")?;
        self.request(|backend, done| backend.join_lobby(id, done))
            .await
    }

    /// Lists open public lobbies using Steam's default search.
    pub async fn list_lobbies(&self) -> Result<Vec<LobbyInfo>> {
        self.request(|backend, done| backend.list_lobbies(done))
            .await
    }

    pub fn leave_lobby(&self, id: &str) -> Result<()> {
        self.backend().leave_lobby(parse_id(id, "a lobby ID")?)
    }

    pub fn lobby_info(&self, id: &str) -> Result<LobbyInfo> {
        self.backend().lobby_info(parse_id(id, "a lobby ID")?)
    }

    /// Sets a lobby data key, or removes it with `None`. Only the owner may change lobby data.
    pub fn set_lobby_data(&self, id: &str, key: &str, value: Option<&str>) -> Result<()> {
        let id = parse_id(id, "a lobby ID")?;
        check_id(key)?;
        if key.len() > MAX_LOBBY_KEY_BYTES {
            return Err(Error::InvalidRequest(format!(
                "a lobby key is at most {MAX_LOBBY_KEY_BYTES} bytes"
            )));
        }
        if let Some(value) = value {
            if value.contains('\0') || value.len() > MAX_LOBBY_VALUE_BYTES {
                return Err(Error::InvalidRequest(format!(
                    "a lobby value is at most {MAX_LOBBY_VALUE_BYTES} bytes and free of NUL"
                )));
            }
        }
        self.backend().set_lobby_data(id, key, value)
    }

    pub fn set_lobby_joinable(&self, id: &str, joinable: bool) -> Result<()> {
        self.backend()
            .set_lobby_joinable(parse_id(id, "a lobby ID")?, joinable)
    }

    pub fn workshop_items(&self) -> Result<Vec<WorkshopItem>> {
        self.backend().workshop_items()
    }

    pub async fn subscribe_item(&self, id: &str) -> Result<()> {
        let id = parse_id(id, "a Workshop item ID")?;
        self.request(|backend, done| backend.subscribe_item(id, done))
            .await
    }

    pub async fn unsubscribe_item(&self, id: &str) -> Result<()> {
        let id = parse_id(id, "a Workshop item ID")?;
        self.request(|backend, done| backend.unsubscribe_item(id, done))
            .await
    }

    pub fn download_item(&self, id: &str, high_priority: bool) -> Result<bool> {
        self.backend()
            .download_item(parse_id(id, "a Workshop item ID")?, high_priority)
    }

    /// Starts Steam Input. Call it once before the other input methods.
    pub fn input_init(&self) -> Result<()> {
        self.backend().input_init()
    }

    pub fn input_controllers(&self) -> Result<Vec<InputController>> {
        self.backend().input_controllers()
    }

    /// Looks up an action set by name and returns its handle.
    pub fn input_action_set(&self, name: &str) -> Result<String> {
        check_id(name)?;
        Ok(self.backend().input_action_set(name)?.to_string())
    }

    pub fn input_activate_action_set(&self, controller: &str, action_set: &str) -> Result<()> {
        self.backend().input_activate_action_set(
            parse_id(controller, "a controller handle")?,
            parse_id(action_set, "an action set handle")?,
        )
    }

    /// Looks up a digital or analog action by name and returns its handle.
    pub fn input_action(&self, kind: ActionKind, name: &str) -> Result<String> {
        check_id(name)?;
        Ok(self.backend().input_action(kind, name)?.to_string())
    }

    pub fn input_digital_state(
        &self,
        controller: &str,
        action: &str,
    ) -> Result<DigitalActionState> {
        self.backend().input_digital_state(
            parse_id(controller, "a controller handle")?,
            parse_id(action, "an action handle")?,
        )
    }

    pub fn input_analog_state(&self, controller: &str, action: &str) -> Result<AnalogActionState> {
        self.backend().input_analog_state(
            parse_id(controller, "a controller handle")?,
            parse_id(action, "an action handle")?,
        )
    }

    /// The PNG for the first button bound to an action on a controller.
    pub fn input_glyph(
        &self,
        controller: &str,
        action_set: &str,
        kind: ActionKind,
        action: &str,
        size: GlyphSize,
    ) -> Result<Vec<u8>> {
        self.backend().input_glyph(
            parse_id(controller, "a controller handle")?,
            parse_id(action_set, "an action set handle")?,
            kind,
            parse_id(action, "an action handle")?,
            size,
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{backend::Emit, config::Config, mock::MockConfig, steam::StartOptions};
    use std::sync::Arc;

    fn steam_with(config: MockConfig) -> Steamworks {
        let emit: Emit = Arc::new(|_| {});
        let options = StartOptions {
            app_id: 480,
            restart_if_necessary: false,
            interval: None,
            preinit: None,
            mock: Some(config),
        };
        Steamworks::start(options, &Config::default(), emit)
    }

    #[test]
    fn ids_must_be_non_zero_decimal_numbers() {
        assert_eq!(
            parse_id("76561198000000000", "x").unwrap(),
            76561198000000000
        );
        for bad in ["", "0", "-1", "abc", "1.5", " 7", "18446744073709551616"] {
            assert!(
                matches!(parse_id(bad, "x"), Err(Error::InvalidRequest(_))),
                "{bad:?} should be refused"
            );
        }
    }

    #[tokio::test]
    async fn a_lobby_can_be_created_found_changed_and_left() {
        let steam = steam_with(MockConfig::default());
        let id = steam.create_lobby(LobbyKind::Public, 4).await.unwrap();

        steam.set_lobby_data(&id, "map", Some("fizzton")).unwrap();
        assert_eq!(steam.lobby_info(&id).unwrap().data["map"], "fizzton");
        assert_eq!(steam.list_lobbies().await.unwrap().len(), 1);
        assert_eq!(steam.join_lobby(&id).await.unwrap(), id);

        steam.leave_lobby(&id).unwrap();
        assert!(steam.lobby_info(&id).is_err());
    }

    #[tokio::test]
    async fn lobby_requests_are_validated_before_steam_sees_them() {
        let steam = steam_with(MockConfig::default());
        for bad in [0, 251] {
            assert!(matches!(
                steam.create_lobby(LobbyKind::Private, bad).await,
                Err(Error::InvalidRequest(_))
            ));
        }
        steam.create_lobby(LobbyKind::Private, 250).await.unwrap();

        assert!(steam.join_lobby("not a number").await.is_err());
        let id = steam.create_lobby(LobbyKind::Private, 2).await.unwrap();
        assert!(steam.set_lobby_data(&id, "", Some("v")).is_err());
        assert!(steam
            .set_lobby_data(&id, &"k".repeat(256), Some("v"))
            .is_err());
        assert!(steam
            .set_lobby_data(&id, "k", Some(&"v".repeat(8193)))
            .is_err());
        assert!(steam.set_lobby_data(&id, "k", Some("a\0b")).is_err());
        steam
            .set_lobby_data(&id, "k", Some(&"v".repeat(8192)))
            .unwrap();
    }

    #[tokio::test]
    async fn workshop_subscriptions_round_trip() {
        let steam = steam_with(MockConfig::default());
        steam.subscribe_item("1234").await.unwrap();
        assert_eq!(steam.workshop_items().unwrap()[0].id, "1234");
        assert!(steam.download_item("1234", false).unwrap());
        steam.unsubscribe_item("1234").await.unwrap();
        assert!(steam.workshop_items().unwrap().is_empty());
        assert!(steam.subscribe_item("0").await.is_err());
    }

    #[test]
    fn input_handles_cross_the_boundary_as_strings() {
        let steam = steam_with(MockConfig {
            controllers: vec![InputController {
                handle: "77".into(),
                kind: "steamDeck".into(),
            }],
            ..MockConfig::default()
        });
        assert!(steam.input_controllers().is_err(), "needs input_init first");
        steam.input_init().unwrap();
        assert_eq!(steam.input_controllers().unwrap()[0].handle, "77");

        let set = steam.input_action_set("Gameplay").unwrap();
        steam.input_activate_action_set("77", &set).unwrap();
        let jump = steam.input_action(ActionKind::Digital, "jump").unwrap();
        assert!(!steam.input_digital_state("77", &jump).unwrap().pressed);
        let png = steam
            .input_glyph("77", &set, ActionKind::Digital, &jump, GlyphSize::Large)
            .unwrap();
        assert_eq!(&png[1..4], b"PNG");
        assert!(steam.input_digital_state("0", &jump).is_err());
    }

    #[tokio::test]
    async fn without_steam_the_new_groups_report_unavailable() {
        let emit: Emit = Arc::new(|_| {});
        let options = StartOptions {
            app_id: 480,
            restart_if_necessary: false,
            interval: None,
            preinit: None,
            mock: None,
        };
        let steam = Steamworks::start(options, &Config::default(), emit);
        let expect =
            |error: Error| assert!(error.to_string().starts_with("unavailable: "), "{error}");
        expect(steam.create_lobby(LobbyKind::Public, 4).await.unwrap_err());
        expect(steam.subscribe_item("5").await.unwrap_err());
        expect(steam.workshop_items().unwrap_err());
        expect(steam.input_init().unwrap_err());
    }
}
