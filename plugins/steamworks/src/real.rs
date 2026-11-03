// Initialises Steam and wraps the steamworks client behind the backend trait
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

use std::{sync::Arc, time::Duration};

#[cfg(steam_sdk)]
use crate::{backend::Emit, models::SteamEvent};
use crate::{backend::SteamworksBackend, models::UnavailableReason, pump::Pump};

/// What Steam initialisation produced.
pub(crate) enum InitOutcome {
    // Never built when the SDK is not linked.
    #[cfg_attr(not(steam_sdk), allow(dead_code))]
    Ready(Ready),
    Unavailable(UnavailableReason, Option<String>),
}

#[cfg(steam_sdk)]
pub(crate) struct Ready {
    pub(crate) client: steamworks::Client,
    pub(crate) app_id: u32,
}

/// Without the SDK there is nothing to be ready with.
#[cfg(not(steam_sdk))]
pub(crate) enum Ready {}

#[cfg(steam_sdk)]
pub(crate) fn initialise(app_id: u32, restart_if_necessary: bool) -> InitOutcome {
    use steamworks::{AppId, Client, SteamAPIInitError};

    if restart_if_necessary && steamworks::restart_app_if_necessary(AppId(app_id)) {
        return InitOutcome::Unavailable(UnavailableReason::RestartRequired, None);
    }
    match Client::init_app(AppId(app_id)) {
        Ok(client) => InitOutcome::Ready(Ready { client, app_id }),
        Err(SteamAPIInitError::NoSteamClient(message)) => {
            InitOutcome::Unavailable(UnavailableReason::SteamNotRunning, Some(message))
        }
        Err(other) => {
            InitOutcome::Unavailable(UnavailableReason::InitFailed, Some(other.to_string()))
        }
    }
}

#[cfg(not(steam_sdk))]
pub(crate) fn initialise(_app_id: u32, _restart_if_necessary: bool) -> InitOutcome {
    let reason = if cfg!(any(target_os = "android", target_os = "ios")) {
        UnavailableReason::UnsupportedPlatform
    } else {
        UnavailableReason::NotCompiled
    };
    InitOutcome::Unavailable(reason, None)
}

#[cfg(steam_sdk)]
pub(crate) fn start(
    ready: Ready,
    emit: crate::backend::Emit,
    interval: Duration,
) -> (Arc<dyn SteamworksBackend>, Option<Pump>) {
    let pump_client = ready.client.clone();
    let backend = Arc::new(SteamBackend::new(ready, emit));
    let pump = Pump::start("steamworks-pump", interval, move || {
        pump_client.run_callbacks()
    });
    match pump {
        Ok(pump) => (backend, Some(pump)),
        Err(e) => {
            log::error!("steamworks: could not start the callback pump: {e}");
            (backend, None)
        }
    }
}

#[cfg(not(steam_sdk))]
pub(crate) fn start(
    ready: Ready,
    _emit: crate::backend::Emit,
    _interval: Duration,
) -> (Arc<dyn SteamworksBackend>, Option<Pump>) {
    match ready {}
}

#[cfg(steam_sdk)]
struct SteamBackend {
    client: steamworks::Client,
    app_id: u32,
    /// Dropping a handle unregisters its callback, so they live as long as the backend.
    _callbacks: std::sync::Mutex<Vec<steamworks::CallbackHandle>>,
}

#[cfg(steam_sdk)]
impl SteamBackend {
    fn new(ready: Ready, emit: Emit) -> Self {
        use steamworks::{UserAchievementStored, UserStatsReceived, UserStatsStored};

        let client = ready.client;
        let mut handles = Vec::new();

        let sink = emit.clone();
        handles.push(client.register_callback(move |v: UserStatsReceived| {
            sink(SteamEvent::StatsReceived {
                success: v.result.is_ok(),
                steam_id: v.steam_id.raw().to_string(),
            });
        }));
        let sink = emit.clone();
        handles.push(client.register_callback(move |v: UserStatsStored| {
            sink(SteamEvent::StatsStored {
                success: v.result.is_ok(),
            });
        }));
        let sink = emit;
        handles.push(client.register_callback(move |v: UserAchievementStored| {
            sink(SteamEvent::AchievementStored {
                id: v.achievement_name,
                current: v.current_progress,
                max: v.max_progress,
            });
        }));

        Self {
            client,
            app_id: ready.app_id,
            _callbacks: std::sync::Mutex::new(handles),
        }
    }
}

#[cfg(steam_sdk)]
impl SteamworksBackend for SteamBackend {
    fn kind(&self) -> crate::models::BackendKind {
        crate::models::BackendKind::Steam
    }

    fn app_id(&self) -> u32 {
        self.app_id
    }

    fn player(&self) -> crate::Result<crate::models::Player> {
        Ok(crate::models::Player {
            steam_id: self.client.user().steam_id().raw().to_string(),
            persona_name: self.client.friends().name(),
            app_language: self.client.apps().current_game_language(),
        })
    }

    fn achievement(&self, id: &str) -> crate::Result<crate::models::Achievement> {
        let stats = self.client.user_stats();
        let (achieved, unlock_time) = stats
            .achievement(id)
            .get_achievement_and_unlock_time()
            .map_err(|()| crate::Error::UnknownAchievement(id.to_string()))?;
        Ok(crate::models::Achievement {
            id: id.to_string(),
            achieved,
            unlock_time: (achieved && unlock_time > 0).then_some(u64::from(unlock_time)),
        })
    }

    fn unlock_achievement(&self, id: &str) -> crate::Result<()> {
        self.client
            .user_stats()
            .achievement(id)
            .set()
            .map_err(|()| crate::Error::UnknownAchievement(id.to_string()))
    }

    fn clear_achievement(&self, id: &str) -> crate::Result<()> {
        self.client
            .user_stats()
            .achievement(id)
            .clear()
            .map_err(|()| crate::Error::UnknownAchievement(id.to_string()))
    }

    fn indicate_progress(&self, id: &str, current: u32, max: u32) -> crate::Result<()> {
        let name = std::ffi::CString::new(id)
            .map_err(|_| crate::Error::InvalidRequest("achievement id contains a NUL".into()))?;
        // steamworks 0.13 has no safe wrapper for this call, so it goes through the raw binding.
        // The interface pointer comes from Steam and stays valid while the client is alive, which
        // `self.client` guarantees.
        let accepted = unsafe {
            steamworks_sys::SteamAPI_ISteamUserStats_IndicateAchievementProgress(
                steamworks_sys::SteamAPI_SteamUserStats_v013(),
                name.as_ptr(),
                current,
                max,
            )
        };
        if accepted {
            Ok(())
        } else {
            Err(crate::Error::UnknownAchievement(id.to_string()))
        }
    }

    fn get_stat(
        &self,
        name: &str,
        kind: crate::models::StatKind,
    ) -> crate::Result<crate::models::StatValue> {
        use crate::models::{StatKind, StatValue};
        let stats = self.client.user_stats();
        match kind {
            StatKind::Int => stats.get_stat_i32(name).map(StatValue::Int),
            StatKind::Float => stats.get_stat_f32(name).map(StatValue::Float),
        }
        .map_err(|()| crate::Error::UnknownStat(name.to_string()))
    }

    fn set_stat(&self, name: &str, value: crate::models::StatValue) -> crate::Result<()> {
        use crate::models::StatValue;
        let stats = self.client.user_stats();
        match value {
            StatValue::Int(v) => stats.set_stat_i32(name, v),
            StatValue::Float(v) => stats.set_stat_f32(name, v),
        }
        .map_err(|()| crate::Error::UnknownStat(name.to_string()))
    }

    fn store_stats(&self) -> crate::Result<()> {
        self.client
            .user_stats()
            .store_stats()
            .map_err(|()| crate::Error::Steam("Steam refused to store stats".into()))
    }
}
