// Holds the active backend, the status and the callback pump, and exposes the plugin API
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

use std::{
    sync::{Arc, Mutex, MutexGuard, RwLock},
    time::Duration,
};

#[cfg(any(test, feature = "mock"))]
use crate::mock::{MockBackend, MockConfig};
use crate::{
    backend::{Emit, NullBackend, SteamworksBackend},
    config::{clamp_interval, Config, DEFAULT_CALLBACK_INTERVAL},
    error::{Error, Result},
    models::{Achievement, Player, StatKind, StatValue, SteamStatus, UnavailableReason},
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

struct Inner {
    backend: RwLock<Arc<dyn SteamworksBackend>>,
    status: RwLock<SteamStatus>,
    pump: Mutex<Option<Pump>>,
}

/// Managed state behind the commands. Rust code reaches it through [`crate::SteamworksExt`].
#[derive(Clone)]
pub struct Steamworks {
    inner: Arc<Inner>,
}

impl Steamworks {
    pub(crate) fn start(options: StartOptions, config: &Config, emit: Emit) -> Self {
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
            return Self::from_parts(Arc::new(backend), status, None);
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
                Self::from_parts(backend, status, pump)
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
                )
            }
        }
    }

    fn from_parts(
        backend: Arc<dyn SteamworksBackend>,
        status: SteamStatus,
        pump: Option<Pump>,
    ) -> Self {
        Self {
            inner: Arc::new(Inner {
                backend: RwLock::new(backend),
                status: RwLock::new(status),
                pump: Mutex::new(pump),
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
