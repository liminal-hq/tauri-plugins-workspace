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
        use steamworks::{
            screenshots::ScreenshotRequested, GameOverlayActivated, UserAchievementStored,
            UserStatsReceived, UserStatsStored,
        };

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
        let sink = emit.clone();
        handles.push(client.register_callback(move |v: UserAchievementStored| {
            sink(SteamEvent::AchievementStored {
                id: v.achievement_name,
                current: v.current_progress,
                max: v.max_progress,
            });
        }));
        let sink = emit.clone();
        handles.push(client.register_callback(move |v: GameOverlayActivated| {
            sink(SteamEvent::OverlayToggled { active: v.active });
        }));
        let sink = emit;
        handles.push(client.register_callback(move |_: ScreenshotRequested| {
            sink(SteamEvent::ScreenshotRequested);
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

    fn set_rich_presence(
        &self,
        entries: &std::collections::BTreeMap<String, Option<String>>,
    ) -> crate::Result<()> {
        let friends = self.client.friends();
        for (key, value) in entries {
            if !friends.set_rich_presence(key, value.as_deref()) {
                return Err(crate::Error::Steam(format!(
                    "Steam refused the rich presence key `{key}`"
                )));
            }
        }
        Ok(())
    }

    fn clear_rich_presence(&self) -> crate::Result<()> {
        self.client.friends().clear_rich_presence();
        Ok(())
    }

    fn activate_overlay(&self, page: crate::models::OverlayPage) -> crate::Result<()> {
        self.client.friends().activate_game_overlay(page.as_str());
        Ok(())
    }

    fn activate_overlay_to_web_page(&self, url: &str) -> crate::Result<()> {
        self.client.friends().activate_game_overlay_to_web_page(url);
        Ok(())
    }

    fn cloud_write(&self, name: &str, data: &[u8]) -> crate::Result<()> {
        let file = cloud_name(name)?;
        let len = i32::try_from(data.len())
            .map_err(|_| crate::Error::InvalidRequest("the file is too large".into()))?;
        // `FileWrite` is Steam's synchronous whole-file write. The interface pointer stays valid
        // while the client is alive, which `self.client` guarantees, and `data` outlives the call.
        let written = unsafe {
            steamworks_sys::SteamAPI_ISteamRemoteStorage_FileWrite(
                steamworks_sys::SteamAPI_SteamRemoteStorage_v016(),
                file.as_ptr(),
                data.as_ptr().cast(),
                len,
            )
        };
        if written {
            Ok(())
        } else {
            Err(crate::Error::Steam(format!(
                "Steam could not write the cloud file `{name}` (quota, size or Cloud settings)"
            )))
        }
    }

    fn cloud_read(&self, name: &str) -> crate::Result<Vec<u8>> {
        let file = cloud_name(name)?;
        // SAFETY: as in `cloud_write`; the buffer is exactly `size` bytes long.
        unsafe {
            let storage = steamworks_sys::SteamAPI_SteamRemoteStorage_v016();
            if !steamworks_sys::SteamAPI_ISteamRemoteStorage_FileExists(storage, file.as_ptr()) {
                return Err(crate::Error::UnknownCloudFile(name.to_string()));
            }
            let size =
                steamworks_sys::SteamAPI_ISteamRemoteStorage_GetFileSize(storage, file.as_ptr());
            let Ok(len) = usize::try_from(size) else {
                return Err(crate::Error::UnknownCloudFile(name.to_string()));
            };
            let mut buffer = vec![0u8; len];
            let read = steamworks_sys::SteamAPI_ISteamRemoteStorage_FileRead(
                storage,
                file.as_ptr(),
                buffer.as_mut_ptr().cast(),
                size,
            );
            if read < 0 || read as usize != len {
                return Err(crate::Error::Steam(format!(
                    "Steam could not read the cloud file `{name}`"
                )));
            }
            Ok(buffer)
        }
    }

    fn cloud_delete(&self, name: &str) -> crate::Result<()> {
        let file = cloud_name(name)?;
        // SAFETY: as in `cloud_write`.
        let deleted = unsafe {
            steamworks_sys::SteamAPI_ISteamRemoteStorage_FileDelete(
                steamworks_sys::SteamAPI_SteamRemoteStorage_v016(),
                file.as_ptr(),
            )
        };
        if deleted {
            Ok(())
        } else {
            Err(crate::Error::UnknownCloudFile(name.to_string()))
        }
    }

    fn cloud_list(&self) -> crate::Result<Vec<crate::models::CloudFile>> {
        let storage = self.client.remote_storage();
        Ok(storage
            .files()
            .into_iter()
            .map(|info| {
                let timestamp = storage.file(&info.name).timestamp();
                crate::models::CloudFile {
                    name: info.name,
                    size: info.size,
                    timestamp,
                }
            })
            .collect())
    }

    fn cloud_quota(&self) -> crate::Result<crate::models::CloudQuota> {
        let (mut total, mut available) = (0u64, 0u64);
        // SAFETY: as in `cloud_write`; both out-pointers refer to live locals.
        let ok = unsafe {
            steamworks_sys::SteamAPI_ISteamRemoteStorage_GetQuota(
                steamworks_sys::SteamAPI_SteamRemoteStorage_v016(),
                &mut total,
                &mut available,
            )
        };
        if ok {
            Ok(crate::models::CloudQuota {
                total_bytes: total,
                available_bytes: available,
            })
        } else {
            Err(crate::Error::Steam(
                "Steam could not report the Cloud quota".into(),
            ))
        }
    }

    fn cloud_status(&self) -> crate::Result<crate::models::CloudStatus> {
        let storage = self.client.remote_storage();
        Ok(crate::models::CloudStatus {
            account_enabled: storage.is_cloud_enabled_for_account(),
            app_enabled: storage.is_cloud_enabled_for_app(),
        })
    }

    fn dlc_list(&self) -> crate::Result<Vec<crate::models::DlcInfo>> {
        let apps = self.client.apps();
        // SAFETY: the interface pointer stays valid while `self.client` is alive. `name` is a
        // writable buffer of the length passed, and the out-pointers refer to live locals.
        let count = unsafe {
            steamworks_sys::SteamAPI_ISteamApps_GetDLCCount(
                steamworks_sys::SteamAPI_SteamApps_v009(),
            )
        };
        let mut list = Vec::new();
        for index in 0..count.max(0) {
            let mut app_id = 0u32;
            let mut available = false;
            let mut name = [0 as std::os::raw::c_char; 256];
            let found = unsafe {
                steamworks_sys::SteamAPI_ISteamApps_BGetDLCDataByIndex(
                    steamworks_sys::SteamAPI_SteamApps_v009(),
                    index,
                    &mut app_id,
                    &mut available,
                    name.as_mut_ptr(),
                    name.len() as i32,
                )
            };
            if !found {
                continue;
            }
            // SAFETY: Steam NUL-terminates the name within the buffer.
            let name = unsafe { std::ffi::CStr::from_ptr(name.as_ptr()) }
                .to_string_lossy()
                .into_owned();
            list.push(crate::models::DlcInfo {
                app_id,
                name,
                available,
                installed: apps.is_dlc_installed(steamworks::AppId(app_id)),
            });
        }
        Ok(list)
    }

    fn is_dlc_installed(&self, app_id: u32) -> crate::Result<bool> {
        Ok(self
            .client
            .apps()
            .is_dlc_installed(steamworks::AppId(app_id)))
    }

    fn device_info(&self) -> crate::Result<crate::models::DeviceInfo> {
        let utils = self.client.utils();
        Ok(crate::models::DeviceInfo {
            steam_deck: utils.is_steam_running_on_steam_deck(),
            big_picture: utils.is_steam_in_big_picture_mode(),
        })
    }

    fn hook_screenshots(&self, hook: bool) -> crate::Result<()> {
        self.client.screenshots().hook_screenshots(hook);
        Ok(())
    }

    fn trigger_screenshot(&self) -> crate::Result<()> {
        self.client.screenshots().trigger_screenshot();
        Ok(())
    }

    fn add_screenshot(&self, path: &std::path::Path, width: u32, height: u32) -> crate::Result<()> {
        let too_large = || crate::Error::InvalidRequest("the screenshot is too large".into());
        self.client
            .screenshots()
            .add_screenshot_to_library(
                path,
                None,
                i32::try_from(width).map_err(|_| too_large())?,
                i32::try_from(height).map_err(|_| too_large())?,
            )
            .map(|_| ())
            .map_err(|e| crate::Error::Steam(e.to_string()))
    }
}

#[cfg(steam_sdk)]
fn cloud_name(name: &str) -> crate::Result<std::ffi::CString> {
    std::ffi::CString::new(name)
        .map_err(|_| crate::Error::InvalidRequest("a cloud file name cannot contain NUL".into()))
}
