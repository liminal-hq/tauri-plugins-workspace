// Provides an in-memory fake backend so apps and tests run without Steam
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

use std::{
    collections::{BTreeMap, BTreeSet},
    sync::{Mutex, MutexGuard},
};

use crate::{
    backend::{Emit, SteamworksBackend},
    error::{Error, Result},
    models::{
        Achievement, BackendKind, CloudFile, CloudQuota, CloudStatus, DeviceInfo, DlcInfo,
        OverlayPage, Player, StatKind, StatValue, SteamEvent,
    },
};

/// What the fake starts with. Achievements and stats must be declared, as Steam requires.
#[derive(Debug, Clone)]
pub struct MockConfig {
    pub app_id: u32,
    pub player: Player,
    pub achievements: Vec<String>,
    pub stats: Vec<(String, StatValue)>,
    /// Makes `store_stats` report failure, to exercise error paths.
    pub fail_store: bool,
    /// Steam Cloud quota in bytes.
    pub cloud_quota_bytes: u64,
    /// Reports Steam Cloud as turned off for the app.
    pub cloud_disabled: bool,
    pub dlc: Vec<DlcInfo>,
    pub device: DeviceInfo,
}

impl Default for MockConfig {
    fn default() -> Self {
        Self {
            app_id: 480,
            player: Player {
                steam_id: "76561198000000000".to_string(),
                persona_name: "Mock Player".to_string(),
                app_language: "english".to_string(),
            },
            achievements: Vec::new(),
            stats: Vec::new(),
            fail_store: false,
            cloud_quota_bytes: 100 * 1024 * 1024,
            cloud_disabled: false,
            dlc: Vec::new(),
            device: DeviceInfo::default(),
        }
    }
}

/// A call the fake received, in order.
#[derive(Debug, Clone, PartialEq)]
pub enum Call {
    Unlock(String),
    Clear(String),
    Progress {
        id: String,
        current: u32,
        max: u32,
    },
    SetStat(String, StatValue),
    StoreStats,
    SetRichPresence(BTreeMap<String, Option<String>>),
    ClearRichPresence,
    Overlay(OverlayPage),
    OverlayWebPage(String),
    CloudWrite {
        name: String,
        len: usize,
    },
    CloudRead(String),
    CloudDelete(String),
    HookScreenshots(bool),
    TriggerScreenshot,
    AddScreenshot {
        path: std::path::PathBuf,
        width: u32,
        height: u32,
    },
}

#[derive(Default)]
struct State {
    /// Achievement id to unlock time (zero until unlocked in this session).
    unlocked: BTreeMap<String, bool>,
    pending: BTreeSet<String>,
    progress: BTreeMap<String, (u32, u32)>,
    stats: BTreeMap<String, StatValue>,
    presence: BTreeMap<String, String>,
    /// Cloud file name to contents and write time.
    cloud: BTreeMap<String, (Vec<u8>, i64)>,
    screenshots_hooked: bool,
    calls: Vec<Call>,
}

pub struct MockBackend {
    config: MockConfig,
    emit: Emit,
    state: Mutex<State>,
}

impl MockBackend {
    pub fn new(config: MockConfig, emit: Emit) -> Self {
        let state = State {
            unlocked: config
                .achievements
                .iter()
                .map(|id| (id.clone(), false))
                .collect(),
            stats: config.stats.iter().cloned().collect(),
            ..State::default()
        };
        Self {
            config,
            emit,
            state: Mutex::new(state),
        }
    }

    fn state(&self) -> MutexGuard<'_, State> {
        self.state.lock().unwrap_or_else(|e| e.into_inner())
    }

    /// The rich presence keys currently set.
    pub fn rich_presence(&self) -> BTreeMap<String, String> {
        self.state().presence.clone()
    }

    /// Every call received so far, oldest first.
    pub fn calls(&self) -> Vec<Call> {
        self.state().calls.clone()
    }
}

impl SteamworksBackend for MockBackend {
    fn kind(&self) -> BackendKind {
        BackendKind::Mock
    }

    fn app_id(&self) -> u32 {
        self.config.app_id
    }

    fn player(&self) -> Result<Player> {
        Ok(self.config.player.clone())
    }

    fn achievement(&self, id: &str) -> Result<Achievement> {
        let state = self.state();
        let achieved = *state
            .unlocked
            .get(id)
            .ok_or_else(|| Error::UnknownAchievement(id.to_string()))?;
        Ok(Achievement {
            id: id.to_string(),
            achieved,
            unlock_time: None,
        })
    }

    fn unlock_achievement(&self, id: &str) -> Result<()> {
        let mut state = self.state();
        let slot = state
            .unlocked
            .get_mut(id)
            .ok_or_else(|| Error::UnknownAchievement(id.to_string()))?;
        let newly = !*slot;
        *slot = true;
        if newly {
            state.pending.insert(id.to_string());
        }
        state.calls.push(Call::Unlock(id.to_string()));
        Ok(())
    }

    fn clear_achievement(&self, id: &str) -> Result<()> {
        let mut state = self.state();
        let slot = state
            .unlocked
            .get_mut(id)
            .ok_or_else(|| Error::UnknownAchievement(id.to_string()))?;
        *slot = false;
        state.pending.remove(id);
        state.progress.remove(id);
        state.calls.push(Call::Clear(id.to_string()));
        Ok(())
    }

    fn indicate_progress(&self, id: &str, current: u32, max: u32) -> Result<()> {
        let mut state = self.state();
        if !state.unlocked.contains_key(id) {
            return Err(Error::UnknownAchievement(id.to_string()));
        }
        state.progress.insert(id.to_string(), (current, max));
        state.calls.push(Call::Progress {
            id: id.to_string(),
            current,
            max,
        });
        drop(state);
        (self.emit)(SteamEvent::AchievementStored {
            id: id.to_string(),
            current,
            max,
        });
        Ok(())
    }

    fn get_stat(&self, name: &str, kind: StatKind) -> Result<StatValue> {
        let state = self.state();
        match state.stats.get(name) {
            Some(value) if value.kind() == kind => Ok(*value),
            _ => Err(Error::UnknownStat(name.to_string())),
        }
    }

    fn set_stat(&self, name: &str, value: StatValue) -> Result<()> {
        let mut state = self.state();
        match state.stats.get(name) {
            Some(existing) if existing.kind() == value.kind() => {}
            _ => return Err(Error::UnknownStat(name.to_string())),
        }
        state.stats.insert(name.to_string(), value);
        state.calls.push(Call::SetStat(name.to_string(), value));
        Ok(())
    }

    fn store_stats(&self) -> Result<()> {
        let (stored, success) = {
            let mut state = self.state();
            state.calls.push(Call::StoreStats);
            let success = !self.config.fail_store;
            let stored = if success {
                std::mem::take(&mut state.pending)
            } else {
                BTreeSet::new()
            };
            (stored, success)
        };
        for id in stored {
            (self.emit)(SteamEvent::AchievementStored {
                id,
                current: 0,
                max: 0,
            });
        }
        (self.emit)(SteamEvent::StatsStored { success });
        if success {
            Ok(())
        } else {
            Err(Error::Steam("the mock was told to fail store_stats".into()))
        }
    }

    fn set_rich_presence(&self, entries: &BTreeMap<String, Option<String>>) -> Result<()> {
        let mut state = self.state();
        for (key, value) in entries {
            match value {
                Some(value) => {
                    state.presence.insert(key.clone(), value.clone());
                }
                None => {
                    state.presence.remove(key);
                }
            }
        }
        state.calls.push(Call::SetRichPresence(entries.clone()));
        Ok(())
    }

    fn clear_rich_presence(&self) -> Result<()> {
        let mut state = self.state();
        state.presence.clear();
        state.calls.push(Call::ClearRichPresence);
        Ok(())
    }

    /// Like Steam, opening the overlay raises a toggle event.
    fn activate_overlay(&self, page: OverlayPage) -> Result<()> {
        self.state().calls.push(Call::Overlay(page));
        (self.emit)(SteamEvent::OverlayToggled { active: true });
        Ok(())
    }

    fn activate_overlay_to_web_page(&self, url: &str) -> Result<()> {
        self.state()
            .calls
            .push(Call::OverlayWebPage(url.to_string()));
        (self.emit)(SteamEvent::OverlayToggled { active: true });
        Ok(())
    }

    fn cloud_write(&self, name: &str, data: &[u8]) -> Result<()> {
        let mut state = self.state();
        let used: u64 = state
            .cloud
            .iter()
            .filter(|(existing, _)| existing.as_str() != name)
            .map(|(_, (bytes, _))| bytes.len() as u64)
            .sum();
        if used + data.len() as u64 > self.config.cloud_quota_bytes {
            return Err(Error::Steam("not enough Steam Cloud quota".into()));
        }
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_secs() as i64)
            .unwrap_or(0);
        state.cloud.insert(name.to_string(), (data.to_vec(), now));
        state.calls.push(Call::CloudWrite {
            name: name.to_string(),
            len: data.len(),
        });
        Ok(())
    }

    fn cloud_read(&self, name: &str) -> Result<Vec<u8>> {
        let mut state = self.state();
        state.calls.push(Call::CloudRead(name.to_string()));
        state
            .cloud
            .get(name)
            .map(|(bytes, _)| bytes.clone())
            .ok_or_else(|| Error::UnknownCloudFile(name.to_string()))
    }

    fn cloud_delete(&self, name: &str) -> Result<()> {
        let mut state = self.state();
        state.calls.push(Call::CloudDelete(name.to_string()));
        state
            .cloud
            .remove(name)
            .map(|_| ())
            .ok_or_else(|| Error::UnknownCloudFile(name.to_string()))
    }

    fn cloud_list(&self) -> Result<Vec<CloudFile>> {
        Ok(self
            .state()
            .cloud
            .iter()
            .map(|(name, (bytes, timestamp))| CloudFile {
                name: name.clone(),
                size: bytes.len() as u64,
                timestamp: *timestamp,
            })
            .collect())
    }

    fn cloud_quota(&self) -> Result<CloudQuota> {
        let used: u64 = self
            .state()
            .cloud
            .values()
            .map(|(bytes, _)| bytes.len() as u64)
            .sum();
        Ok(CloudQuota {
            total_bytes: self.config.cloud_quota_bytes,
            available_bytes: self.config.cloud_quota_bytes.saturating_sub(used),
        })
    }

    fn cloud_status(&self) -> Result<CloudStatus> {
        Ok(CloudStatus {
            account_enabled: true,
            app_enabled: !self.config.cloud_disabled,
        })
    }

    fn dlc_list(&self) -> Result<Vec<DlcInfo>> {
        Ok(self.config.dlc.clone())
    }

    fn is_dlc_installed(&self, app_id: u32) -> Result<bool> {
        Ok(self
            .config
            .dlc
            .iter()
            .any(|dlc| dlc.app_id == app_id && dlc.installed))
    }

    fn device_info(&self) -> Result<DeviceInfo> {
        Ok(self.config.device)
    }

    fn hook_screenshots(&self, hook: bool) -> Result<()> {
        let mut state = self.state();
        state.screenshots_hooked = hook;
        state.calls.push(Call::HookScreenshots(hook));
        Ok(())
    }

    /// Like Steam, a hooked app is asked to take the screenshot itself.
    fn trigger_screenshot(&self) -> Result<()> {
        let hooked = {
            let mut state = self.state();
            state.calls.push(Call::TriggerScreenshot);
            state.screenshots_hooked
        };
        if hooked {
            (self.emit)(SteamEvent::ScreenshotRequested);
        }
        Ok(())
    }

    fn add_screenshot(&self, path: &std::path::Path, width: u32, height: u32) -> Result<()> {
        self.state().calls.push(Call::AddScreenshot {
            path: path.to_path_buf(),
            width,
            height,
        });
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;

    fn fixture(fail_store: bool) -> (MockBackend, Arc<Mutex<Vec<SteamEvent>>>) {
        let events = Arc::new(Mutex::new(Vec::new()));
        let sink = events.clone();
        let emit: Emit = Arc::new(move |event| sink.lock().unwrap().push(event));
        let config = MockConfig {
            achievements: vec!["ACH_ONE".into(), "ACH_TWO".into()],
            stats: vec![
                ("games".into(), StatValue::Int(0)),
                ("distance".into(), StatValue::Float(0.0)),
            ],
            fail_store,
            ..MockConfig::default()
        };
        (MockBackend::new(config, emit), events)
    }

    #[test]
    fn unlock_is_recorded_and_visible() {
        let (mock, _) = fixture(false);
        assert!(!mock.achievement("ACH_ONE").unwrap().achieved);
        mock.unlock_achievement("ACH_ONE").unwrap();
        assert!(mock.achievement("ACH_ONE").unwrap().achieved);
        assert_eq!(mock.calls(), vec![Call::Unlock("ACH_ONE".into())]);
    }

    #[test]
    fn clear_resets_an_unlock() {
        let (mock, _) = fixture(false);
        mock.unlock_achievement("ACH_ONE").unwrap();
        mock.clear_achievement("ACH_ONE").unwrap();
        assert!(!mock.achievement("ACH_ONE").unwrap().achieved);
    }

    #[test]
    fn unknown_ids_are_rejected() {
        let (mock, _) = fixture(false);
        assert!(matches!(
            mock.unlock_achievement("NOPE"),
            Err(Error::UnknownAchievement(_))
        ));
        assert!(matches!(
            mock.get_stat("nope", StatKind::Int),
            Err(Error::UnknownStat(_))
        ));
        assert!(matches!(
            mock.set_stat("nope", StatValue::Int(1)),
            Err(Error::UnknownStat(_))
        ));
    }

    #[test]
    fn stats_round_trip_by_kind() {
        let (mock, _) = fixture(false);
        mock.set_stat("games", StatValue::Int(7)).unwrap();
        mock.set_stat("distance", StatValue::Float(2.5)).unwrap();
        assert_eq!(
            mock.get_stat("games", StatKind::Int).unwrap(),
            StatValue::Int(7)
        );
        assert_eq!(
            mock.get_stat("distance", StatKind::Float).unwrap(),
            StatValue::Float(2.5)
        );
        // A stat declared as an int cannot be read or written as a float.
        assert!(mock.get_stat("games", StatKind::Float).is_err());
        assert!(mock.set_stat("games", StatValue::Float(1.0)).is_err());
    }

    #[test]
    fn store_emits_stored_events_once() {
        let (mock, events) = fixture(false);
        mock.unlock_achievement("ACH_TWO").unwrap();
        mock.store_stats().unwrap();
        mock.store_stats().unwrap();
        let events = events.lock().unwrap().clone();
        assert_eq!(
            events,
            vec![
                SteamEvent::AchievementStored {
                    id: "ACH_TWO".into(),
                    current: 0,
                    max: 0
                },
                SteamEvent::StatsStored { success: true },
                SteamEvent::StatsStored { success: true },
            ]
        );
    }

    #[test]
    fn failed_store_reports_failure() {
        let (mock, events) = fixture(true);
        mock.unlock_achievement("ACH_ONE").unwrap();
        assert!(mock.store_stats().is_err());
        assert_eq!(
            events.lock().unwrap().clone(),
            vec![SteamEvent::StatsStored { success: false }]
        );
    }

    #[test]
    fn presence_sets_updates_and_removes_keys() {
        let (mock, _) = fixture(false);
        let mut entries = BTreeMap::new();
        entries.insert("status".to_string(), Some("Mayor of Fizzton".to_string()));
        entries.insert("steam_display".to_string(), Some("#Playing".to_string()));
        mock.set_rich_presence(&entries).unwrap();
        assert_eq!(mock.rich_presence().len(), 2);
        let mut removal = BTreeMap::new();
        removal.insert("status".to_string(), None);
        mock.set_rich_presence(&removal).unwrap();
        assert_eq!(
            mock.rich_presence().keys().collect::<Vec<_>>(),
            vec!["steam_display"]
        );
        mock.clear_rich_presence().unwrap();
        assert!(mock.rich_presence().is_empty());
    }

    #[test]
    fn opening_the_overlay_raises_a_toggle_event() {
        let (mock, events) = fixture(false);
        mock.activate_overlay(OverlayPage::Achievements).unwrap();
        mock.activate_overlay_to_web_page("https://example.com/")
            .unwrap();
        assert_eq!(
            events.lock().unwrap().clone(),
            vec![
                SteamEvent::OverlayToggled { active: true },
                SteamEvent::OverlayToggled { active: true }
            ]
        );
        assert_eq!(
            mock.calls(),
            vec![
                Call::Overlay(OverlayPage::Achievements),
                Call::OverlayWebPage("https://example.com/".into())
            ]
        );
    }

    #[test]
    fn cloud_files_round_trip_list_and_delete() {
        let (mock, _) = fixture(false);
        mock.cloud_write("saves/city.sav", b"hello").unwrap();
        mock.cloud_write("settings.json", b"{}").unwrap();
        assert_eq!(mock.cloud_read("saves/city.sav").unwrap(), b"hello");
        let list = mock.cloud_list().unwrap();
        assert_eq!(
            list.iter()
                .map(|f| (f.name.as_str(), f.size))
                .collect::<Vec<_>>(),
            vec![("saves/city.sav", 5), ("settings.json", 2)]
        );
        mock.cloud_delete("settings.json").unwrap();
        assert!(matches!(
            mock.cloud_read("settings.json"),
            Err(Error::UnknownCloudFile(_))
        ));
        assert!(matches!(
            mock.cloud_delete("settings.json"),
            Err(Error::UnknownCloudFile(_))
        ));
    }

    #[test]
    fn cloud_quota_counts_overwrites_once_and_rejects_overflow() {
        let emit: Emit = Arc::new(|_| {});
        let mock = MockBackend::new(
            MockConfig {
                cloud_quota_bytes: 10,
                ..MockConfig::default()
            },
            emit,
        );
        mock.cloud_write("a", &[0; 8]).unwrap();
        mock.cloud_write("a", &[0; 9]).unwrap();
        assert_eq!(mock.cloud_quota().unwrap().available_bytes, 1);
        assert!(mock.cloud_write("b", &[0; 2]).is_err());
        assert_eq!(mock.cloud_quota().unwrap().total_bytes, 10);
    }

    #[test]
    fn dlc_and_device_come_from_the_config() {
        let emit: Emit = Arc::new(|_| {});
        let mock = MockBackend::new(
            MockConfig {
                dlc: vec![
                    DlcInfo {
                        app_id: 1,
                        name: "Owned".into(),
                        available: true,
                        installed: true,
                    },
                    DlcInfo {
                        app_id: 2,
                        name: "Missing".into(),
                        available: true,
                        installed: false,
                    },
                ],
                device: DeviceInfo {
                    steam_deck: true,
                    big_picture: false,
                },
                ..MockConfig::default()
            },
            emit,
        );
        assert_eq!(mock.dlc_list().unwrap().len(), 2);
        assert!(mock.is_dlc_installed(1).unwrap());
        assert!(!mock.is_dlc_installed(2).unwrap());
        assert!(!mock.is_dlc_installed(3).unwrap());
        assert!(mock.device_info().unwrap().steam_deck);
    }

    #[test]
    fn a_screenshot_request_is_raised_only_while_hooked() {
        let (mock, events) = fixture(false);
        mock.trigger_screenshot().unwrap();
        assert!(events.lock().unwrap().is_empty());
        mock.hook_screenshots(true).unwrap();
        mock.trigger_screenshot().unwrap();
        assert_eq!(
            events.lock().unwrap().clone(),
            vec![SteamEvent::ScreenshotRequested]
        );
    }

    #[test]
    fn progress_emits_an_event_with_the_counts() {
        let (mock, events) = fixture(false);
        mock.indicate_progress("ACH_ONE", 3, 10).unwrap();
        assert_eq!(
            events.lock().unwrap().clone(),
            vec![SteamEvent::AchievementStored {
                id: "ACH_ONE".into(),
                current: 3,
                max: 10
            }]
        );
    }
}
