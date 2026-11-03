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
    models::{Achievement, BackendKind, Player, StatKind, StatValue, SteamEvent},
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
        }
    }
}

/// A call the fake received, in order.
#[derive(Debug, Clone, PartialEq)]
pub enum Call {
    Unlock(String),
    Clear(String),
    Progress { id: String, current: u32, max: u32 },
    SetStat(String, StatValue),
    StoreStats,
}

#[derive(Default)]
struct State {
    /// Achievement id to unlock time (zero until unlocked in this session).
    unlocked: BTreeMap<String, bool>,
    pending: BTreeSet<String>,
    progress: BTreeMap<String, (u32, u32)>,
    stats: BTreeMap<String, StatValue>,
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
