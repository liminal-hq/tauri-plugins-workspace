// Holds the active backend, the status and the callback pump, and exposes the plugin API
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

use std::{
    collections::BTreeMap,
    sync::{Arc, Mutex, MutexGuard, RwLock},
    time::Duration,
};

#[cfg(any(test, feature = "mock"))]
use crate::mock::{MockBackend, MockConfig};
use crate::{
    backend::{Emit, NullBackend, SteamworksBackend},
    config::{clamp_interval, Config, DEFAULT_CALLBACK_INTERVAL},
    error::{Error, Result},
    models::{
        Achievement, CloudFile, CloudQuota, CloudStatus, OverlayPage, Player, StatKind, StatValue,
        SteamEvent, SteamStatus, UnavailableReason,
    },
    pump::Pump,
    real::{self, InitOutcome},
};

/// Steam initialised ahead of the Tauri builder. See [`preinit`].
pub struct PreInit {
    pub(crate) app_id: u32,
    pub(crate) outcome: InitOutcome,
}

impl PreInit {
    /// The status this initialisation produced.
    pub fn status(&self) -> SteamStatus {
        match &self.outcome {
            InitOutcome::Ready(_) => SteamStatus::Available {
                backend: crate::models::BackendKind::Steam,
                app_id: self.app_id,
            },
            InitOutcome::Unavailable(reason, message) => SteamStatus::Unavailable {
                reason: *reason,
                message: message.clone(),
            },
        }
    }

    /// The Steam client, for handing to other integrations such as an overlay surface.
    #[cfg(steam_sdk)]
    pub fn client(&self) -> Option<&steamworks::Client> {
        match &self.outcome {
            InitOutcome::Ready(ready) => Some(&ready.client),
            InitOutcome::Unavailable(..) => None,
        }
    }
}

/// Initialises Steam before `tauri::Builder` runs. Some integrations, such as the Windows overlay
/// surface, need Steam initialised first. Pass the result to [`crate::Builder::from_preinit`].
/// Never panics: a failure becomes an unavailable status.
pub fn preinit(app_id: u32, restart_if_necessary: bool) -> PreInit {
    PreInit {
        app_id,
        outcome: real::initialise(app_id, restart_if_necessary),
    }
}

pub(crate) struct StartOptions {
    pub app_id: u32,
    pub restart_if_necessary: bool,
    pub interval: Option<Duration>,
    pub preinit: Option<PreInit>,
    #[cfg(any(test, feature = "mock"))]
    pub mock: Option<MockConfig>,
}

type OverlayHook = Arc<dyn Fn(bool) + Send + Sync>;

/// Callbacks registered with [`Steamworks::on_overlay_toggled`].
#[derive(Default)]
struct OverlayHooks(Mutex<Vec<OverlayHook>>);

impl OverlayHooks {
    fn add(&self, hook: OverlayHook) {
        self.0.lock().unwrap_or_else(|e| e.into_inner()).push(hook);
    }

    fn notify(&self, active: bool) {
        // Copy the list first so a hook can register another hook without deadlocking.
        let hooks: Vec<OverlayHook> = self.0.lock().unwrap_or_else(|e| e.into_inner()).clone();
        for hook in hooks {
            hook(active);
        }
    }
}

struct Inner {
    backend: RwLock<Arc<dyn SteamworksBackend>>,
    status: RwLock<SteamStatus>,
    pump: Mutex<Option<Pump>>,
    overlay_hooks: Arc<OverlayHooks>,
}

/// Managed state behind the commands. Rust code reaches it through [`crate::SteamworksExt`].
#[derive(Clone)]
pub struct Steamworks {
    inner: Arc<Inner>,
}

impl Steamworks {
    pub(crate) fn start(options: StartOptions, config: &Config, emit: Emit) -> Self {
        let overlay_hooks = Arc::new(OverlayHooks::default());
        let emit: Emit = {
            let hooks = overlay_hooks.clone();
            Arc::new(move |event| {
                if let SteamEvent::OverlayToggled { active } = &event {
                    hooks.notify(*active);
                }
                emit(event);
            })
        };
        let interval = clamp_interval(
            options
                .interval
                .or_else(|| config.callback_interval())
                .unwrap_or(DEFAULT_CALLBACK_INTERVAL),
        );

        #[cfg(any(test, feature = "mock"))]
        let mock = options
            .mock
            .or_else(|| config.wants_mock().then(MockConfig::default));
        #[cfg(not(any(test, feature = "mock")))]
        if config.wants_mock() {
            log::warn!("steamworks: `mock` is set in the config but the `mock` feature is not compiled; ignoring it");
        }

        #[cfg(any(test, feature = "mock"))]
        if let Some(mock) = mock {
            let backend = MockBackend::new(mock, emit);
            let status = SteamStatus::Available {
                backend: crate::models::BackendKind::Mock,
                app_id: backend.app_id(),
            };
            return Self::from_parts(Arc::new(backend), status, None, overlay_hooks);
        }

        let outcome = match options.preinit {
            Some(pre) => pre.outcome,
            None => real::initialise(options.app_id, options.restart_if_necessary),
        };
        match outcome {
            InitOutcome::Ready(ready) => {
                let (backend, pump) = real::start(ready, emit, interval);
                let status = SteamStatus::Available {
                    backend: backend.kind(),
                    app_id: backend.app_id(),
                };
                Self::from_parts(backend, status, pump, overlay_hooks)
            }
            InitOutcome::Unavailable(reason, message) => {
                if let Some(message) = &message {
                    log::info!("steamworks: unavailable ({}): {message}", reason.as_str());
                } else {
                    log::info!("steamworks: unavailable ({})", reason.as_str());
                }
                Self::from_parts(
                    Arc::new(NullBackend::new(reason)),
                    SteamStatus::Unavailable { reason, message },
                    None,
                    overlay_hooks,
                )
            }
        }
    }

    fn from_parts(
        backend: Arc<dyn SteamworksBackend>,
        status: SteamStatus,
        pump: Option<Pump>,
        overlay_hooks: Arc<OverlayHooks>,
    ) -> Self {
        Self {
            inner: Arc::new(Inner {
                backend: RwLock::new(backend),
                status: RwLock::new(status),
                pump: Mutex::new(pump),
                overlay_hooks,
            }),
        }
    }

    fn backend(&self) -> Arc<dyn SteamworksBackend> {
        self.inner
            .backend
            .read()
            .unwrap_or_else(|e| e.into_inner())
            .clone()
    }

    fn pump(&self) -> MutexGuard<'_, Option<Pump>> {
        self.inner.pump.lock().unwrap_or_else(|e| e.into_inner())
    }

    pub fn status(&self) -> SteamStatus {
        self.inner
            .status
            .read()
            .unwrap_or_else(|e| e.into_inner())
            .clone()
    }

    pub fn is_available(&self) -> bool {
        self.status().is_available()
    }

    pub fn player(&self) -> Result<Player> {
        self.backend().player()
    }

    pub fn achievement(&self, id: &str) -> Result<Achievement> {
        check_id(id)?;
        self.backend().achievement(id)
    }

    /// Unlocks an achievement and stores it so Steam shows the notification.
    pub fn unlock_achievement(&self, id: &str) -> Result<()> {
        check_id(id)?;
        let backend = self.backend();
        backend.unlock_achievement(id)?;
        backend.store_stats()
    }

    pub fn clear_achievement(&self, id: &str) -> Result<()> {
        check_id(id)?;
        let backend = self.backend();
        backend.clear_achievement(id)?;
        backend.store_stats()
    }

    pub fn indicate_progress(&self, id: &str, current: u32, max: u32) -> Result<()> {
        check_id(id)?;
        if max == 0 || current > max {
            return Err(Error::InvalidRequest(
                "progress needs 0 <= current <= max and max > 0".into(),
            ));
        }
        self.backend().indicate_progress(id, current, max)
    }

    pub fn get_stat(&self, name: &str, kind: StatKind) -> Result<StatValue> {
        check_id(name)?;
        self.backend().get_stat(name, kind)
    }

    /// Sets a stat without storing it, so several writes can be stored together.
    pub fn set_stat(&self, name: &str, value: StatValue) -> Result<()> {
        check_id(name)?;
        self.backend().set_stat(name, value)
    }

    pub fn store_stats(&self) -> Result<()> {
        self.backend().store_stats()
    }

    /// Sets or, with `None`, removes rich presence keys. Steam allows 20 keys of up to 64 bytes
    /// with values of up to 256 bytes, so larger requests are rejected up front.
    pub fn set_rich_presence(&self, entries: &BTreeMap<String, Option<String>>) -> Result<()> {
        check_presence(entries)?;
        self.backend().set_rich_presence(entries)
    }

    pub fn clear_rich_presence(&self) -> Result<()> {
        self.backend().clear_rich_presence()
    }

    pub fn activate_overlay(&self, page: OverlayPage) -> Result<()> {
        self.backend().activate_overlay(page)
    }

    /// Opens a web page in the overlay. Only `http` and `https` addresses are accepted.
    pub fn activate_overlay_to_web_page(&self, url: &str) -> Result<()> {
        let parsed = url::Url::parse(url)
            .map_err(|e| Error::InvalidRequest(format!("not a valid URL: {e}")))?;
        if !matches!(parsed.scheme(), "http" | "https") {
            return Err(Error::InvalidRequest(
                "the overlay opens only http and https addresses".into(),
            ));
        }
        self.backend().activate_overlay_to_web_page(parsed.as_str())
    }

    /// Writes a whole file to Steam Cloud. Names use ASCII letters, digits and ` ._-`, with `/`
    /// separating folders; files are at most 100 MiB. Prefer Auto-Cloud for save games.
    pub fn cloud_write(&self, name: &str, data: &[u8]) -> Result<()> {
        check_cloud_name(name)?;
        if data.len() > MAX_CLOUD_FILE_BYTES {
            return Err(Error::InvalidRequest(format!(
                "a cloud file is at most {MAX_CLOUD_FILE_BYTES} bytes"
            )));
        }
        self.backend().cloud_write(name, data)
    }

    pub fn cloud_read(&self, name: &str) -> Result<Vec<u8>> {
        check_cloud_name(name)?;
        self.backend().cloud_read(name)
    }

    pub fn cloud_delete(&self, name: &str) -> Result<()> {
        check_cloud_name(name)?;
        self.backend().cloud_delete(name)
    }

    pub fn cloud_list(&self) -> Result<Vec<CloudFile>> {
        self.backend().cloud_list()
    }

    pub fn cloud_quota(&self) -> Result<CloudQuota> {
        self.backend().cloud_quota()
    }

    pub fn cloud_status(&self) -> Result<CloudStatus> {
        self.backend().cloud_status()
    }

    /// Calls `hook` with `true` or `false` whenever the Steam overlay opens or closes. Use it to
    /// pass the change to other integrations, such as an overlay-surface plugin, from Rust.
    pub fn on_overlay_toggled(&self, hook: impl Fn(bool) + Send + Sync + 'static) {
        self.inner.overlay_hooks.add(Arc::new(hook));
    }

    /// Stops the pump and drops the Steam client. Later calls report `shutdown`.
    pub fn shutdown(&self) {
        if let Some(mut pump) = self.pump().take() {
            pump.stop();
        }
        *self
            .inner
            .backend
            .write()
            .unwrap_or_else(|e| e.into_inner()) =
            Arc::new(NullBackend::new(UnavailableReason::Shutdown));
        *self.inner.status.write().unwrap_or_else(|e| e.into_inner()) =
            SteamStatus::unavailable(UnavailableReason::Shutdown);
    }
}

/// Steam's per-file limit for Cloud files.
const MAX_CLOUD_FILE_BYTES: usize = 100 * 1024 * 1024;
const MAX_CLOUD_NAME_BYTES: usize = 260;

/// Accepts a conservative set of names so they are safe as headers and as paths: ASCII letters,
/// digits and ` ._-`, with `/` between folder segments and no `.`, `..` or empty segments.
fn check_cloud_name(name: &str) -> Result<()> {
    let bad = |why: &str| Err(Error::InvalidRequest(format!("cloud file name {why}")));
    if name.is_empty() || name.len() > MAX_CLOUD_NAME_BYTES {
        return bad("must be 1-260 bytes");
    }
    if !name
        .chars()
        .all(|c| c.is_ascii_alphanumeric() || matches!(c, ' ' | '.' | '_' | '-' | '/'))
    {
        return bad("may use only ASCII letters, digits and space . _ - /");
    }
    for segment in name.split('/') {
        if segment.is_empty() || segment == "." || segment == ".." || segment.trim() != segment {
            return bad("has an empty, dot or padded folder segment");
        }
    }
    Ok(())
}

const MAX_PRESENCE_KEYS: usize = 20;
const MAX_PRESENCE_KEY_BYTES: usize = 64;
const MAX_PRESENCE_VALUE_BYTES: usize = 256;

fn check_presence(entries: &BTreeMap<String, Option<String>>) -> Result<()> {
    if entries.len() > MAX_PRESENCE_KEYS {
        return Err(Error::InvalidRequest(format!(
            "rich presence takes at most {MAX_PRESENCE_KEYS} keys"
        )));
    }
    for (key, value) in entries {
        check_id(key)?;
        if key.len() > MAX_PRESENCE_KEY_BYTES {
            return Err(Error::InvalidRequest(format!(
                "a rich presence key is at most {MAX_PRESENCE_KEY_BYTES} bytes"
            )));
        }
        if let Some(value) = value {
            if value.contains('\0') || value.len() > MAX_PRESENCE_VALUE_BYTES {
                return Err(Error::InvalidRequest(format!(
                    "a rich presence value is at most {MAX_PRESENCE_VALUE_BYTES} bytes and free of NUL"
                )));
            }
        }
    }
    Ok(())
}

fn check_id(id: &str) -> Result<()> {
    if id.is_empty() || id.contains('\0') {
        return Err(Error::InvalidRequest(
            "an id or name must be non-empty and free of NUL".into(),
        ));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::SteamEvent;

    fn emit_sink() -> (Emit, Arc<Mutex<Vec<SteamEvent>>>) {
        let events = Arc::new(Mutex::new(Vec::new()));
        let sink = events.clone();
        (Arc::new(move |e| sink.lock().unwrap().push(e)), events)
    }

    fn options() -> StartOptions {
        StartOptions {
            app_id: 480,
            restart_if_necessary: false,
            interval: None,
            preinit: None,
            mock: None,
        }
    }

    #[test]
    fn without_steam_the_plugin_is_unavailable_not_broken() {
        let (emit, _) = emit_sink();
        let steam = Steamworks::start(options(), &Config::default(), emit);
        assert!(!steam.is_available());
        let SteamStatus::Unavailable { reason, .. } = steam.status() else {
            panic!("expected unavailable");
        };
        if cfg!(steam_sdk) {
            // The SDK is linked but no Steam client runs here, so it cannot initialise.
            assert!(matches!(
                reason,
                UnavailableReason::SteamNotRunning | UnavailableReason::InitFailed
            ));
        } else {
            assert_eq!(reason, UnavailableReason::NotCompiled);
        }
        let err = steam.unlock_achievement("ACH").unwrap_err();
        assert!(err.to_string().starts_with("unavailable: "));
        assert!(steam.player().is_err());
        assert!(steam.store_stats().is_err());
    }

    #[test]
    fn config_can_select_the_mock() {
        let (emit, _) = emit_sink();
        let config = Config {
            mock: Some(true),
            ..Config::default()
        };
        let steam = Steamworks::start(options(), &config, emit);
        assert!(steam.is_available());
        assert_eq!(steam.player().unwrap().persona_name, "Mock Player");
    }

    #[test]
    fn explicit_mock_beats_everything() {
        let (emit, events) = emit_sink();
        let mut opts = options();
        opts.mock = Some(MockConfig {
            achievements: vec!["ACH".into()],
            ..MockConfig::default()
        });
        let steam = Steamworks::start(opts, &Config::default(), emit);
        steam.unlock_achievement("ACH").unwrap();
        assert!(steam.achievement("ACH").unwrap().achieved);
        let events = events.lock().unwrap();
        assert!(events.contains(&SteamEvent::StatsStored { success: true }));
    }

    #[test]
    fn progress_is_validated_before_the_backend_sees_it() {
        let (emit, _) = emit_sink();
        let mut opts = options();
        opts.mock = Some(MockConfig {
            achievements: vec!["ACH".into()],
            ..MockConfig::default()
        });
        let steam = Steamworks::start(opts, &Config::default(), emit);
        assert!(matches!(
            steam.indicate_progress("ACH", 5, 3),
            Err(Error::InvalidRequest(_))
        ));
        assert!(matches!(
            steam.indicate_progress("ACH", 0, 0),
            Err(Error::InvalidRequest(_))
        ));
        steam.indicate_progress("ACH", 1, 3).unwrap();
        assert!(matches!(
            steam.achievement(""),
            Err(Error::InvalidRequest(_))
        ));
    }

    fn mock_steam() -> (Steamworks, Arc<Mutex<Vec<SteamEvent>>>) {
        let (emit, events) = emit_sink();
        let mut opts = options();
        opts.mock = Some(MockConfig::default());
        (Steamworks::start(opts, &Config::default(), emit), events)
    }

    fn entry(key: &str, value: &str) -> BTreeMap<String, Option<String>> {
        BTreeMap::from([(key.to_string(), Some(value.to_string()))])
    }

    #[test]
    fn presence_limits_are_enforced_before_the_backend() {
        let (steam, _) = mock_steam();
        steam.set_rich_presence(&entry("status", "Mayor")).unwrap();

        let long_key = "k".repeat(65);
        assert!(steam.set_rich_presence(&entry(&long_key, "v")).is_err());
        let long_value = "v".repeat(257);
        assert!(steam
            .set_rich_presence(&entry("status", &long_value))
            .is_err());
        assert!(steam.set_rich_presence(&entry("", "v")).is_err());
        assert!(steam.set_rich_presence(&entry("status", "a\0b")).is_err());

        let many: BTreeMap<_, _> = (0..21)
            .map(|i| (format!("k{i}"), Some("v".to_string())))
            .collect();
        assert!(steam.set_rich_presence(&many).is_err());
        let exactly_max = "v".repeat(256);
        steam
            .set_rich_presence(&entry("status", &exactly_max))
            .unwrap();
    }

    #[test]
    fn cloud_names_are_confined_to_safe_relative_paths() {
        let (steam, _) = mock_steam();
        for good in ["save.dat", "saves/city 1.sav", "a-b_c.d/e"] {
            steam
                .cloud_write(good, b"x")
                .unwrap_or_else(|e| panic!("{good}: {e}"));
        }
        let too_long = "a".repeat(261);
        for bad in [
            "",
            "/etc/passwd",
            "../escape",
            "a/../b",
            "a//b",
            "a/",
            "./a",
            "back\\slash",
            "unicode-é",
            "nul\0byte",
            " padded /x",
            &too_long,
        ] {
            assert!(
                matches!(steam.cloud_write(bad, b"x"), Err(Error::InvalidRequest(_))),
                "{bad:?} should be refused"
            );
            assert!(matches!(
                steam.cloud_read(bad),
                Err(Error::InvalidRequest(_))
            ));
            assert!(matches!(
                steam.cloud_delete(bad),
                Err(Error::InvalidRequest(_))
            ));
        }
    }

    #[test]
    fn cloud_calls_without_steam_report_unavailable() {
        let (emit, _) = emit_sink();
        let mut opts = options();
        opts.mock = None;
        let steam = Steamworks::start(opts, &Config::default(), emit);
        assert!(steam
            .cloud_list()
            .unwrap_err()
            .to_string()
            .starts_with("unavailable: "));
        assert!(steam.cloud_quota().is_err());
    }

    #[test]
    fn only_web_addresses_open_in_the_overlay() {
        let (steam, _) = mock_steam();
        steam
            .activate_overlay_to_web_page("https://store.steampowered.com/app/480")
            .unwrap();
        for bad in [
            "file:///etc/passwd",
            "javascript:alert(1)",
            "steam://run/480",
            "not a url",
        ] {
            assert!(
                matches!(
                    steam.activate_overlay_to_web_page(bad),
                    Err(Error::InvalidRequest(_))
                ),
                "{bad} should be refused"
            );
        }
    }

    #[test]
    fn overlay_hooks_hear_toggles_and_may_register_more_hooks() {
        let (steam, events) = mock_steam();
        let heard = Arc::new(Mutex::new(Vec::new()));
        let sink = heard.clone();
        let again = steam.clone();
        steam.on_overlay_toggled(move |active| {
            sink.lock().unwrap().push(active);
            again.on_overlay_toggled(|_| {});
        });
        steam.activate_overlay(OverlayPage::Friends).unwrap();
        assert_eq!(heard.lock().unwrap().clone(), vec![true]);
        assert!(events
            .lock()
            .unwrap()
            .contains(&SteamEvent::OverlayToggled { active: true }));
    }

    #[test]
    fn shutdown_makes_later_calls_report_shutdown() {
        let (emit, _) = emit_sink();
        let mut opts = options();
        opts.mock = Some(MockConfig::default());
        let steam = Steamworks::start(opts, &Config::default(), emit);
        assert!(steam.is_available());
        steam.shutdown();
        assert!(!steam.is_available());
        assert_eq!(
            steam.player().unwrap_err().to_string(),
            "unavailable: shutdown"
        );
        steam.shutdown();
    }
}
