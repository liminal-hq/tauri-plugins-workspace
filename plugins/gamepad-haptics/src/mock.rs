// A backend that records what it is asked to do, for tests
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

use std::{
    sync::{
        atomic::{AtomicBool, AtomicUsize, Ordering},
        Mutex,
    },
    time::Instant,
};

use crate::{
    backend::{DiscoveredPad, Levels, Notify, RumbleBackend},
    Error, Result,
};

#[derive(Debug, Clone, PartialEq)]
pub enum Call {
    Set {
        key: String,
        levels: Levels,
        hold_ms: u64,
        at_ms: u128,
    },
    Silence {
        key: String,
        at_ms: u128,
    },
    Reset {
        key: String,
    },
}

pub struct MockBackend {
    pads: Mutex<Vec<DiscoveredPad>>,
    calls: Mutex<Vec<Call>>,
    notify: Mutex<Option<Notify>>,
    fail_sets: AtomicBool,
    scans: AtomicUsize,
    started: Instant,
}

impl MockBackend {
    pub fn new(pads: Vec<DiscoveredPad>) -> Self {
        Self {
            pads: Mutex::new(pads),
            calls: Mutex::new(Vec::new()),
            notify: Mutex::new(None),
            fail_sets: AtomicBool::new(false),
            scans: AtomicUsize::new(0),
            started: Instant::now(),
        }
    }

    /// The sets and silences, without the resets a pad gets when it appears.
    pub fn calls(&self) -> Vec<Call> {
        self.calls
            .lock()
            .unwrap()
            .iter()
            .filter(|c| !matches!(c, Call::Reset { .. }))
            .cloned()
            .collect()
    }

    /// The keys of pads that were reset on arrival, in order.
    pub fn resets(&self) -> Vec<String> {
        self.calls
            .lock()
            .unwrap()
            .iter()
            .filter_map(|c| match c {
                Call::Reset { key } => Some(key.clone()),
                _ => None,
            })
            .collect()
    }

    /// How many times the pads were scanned.
    pub fn scans(&self) -> usize {
        self.scans.load(Ordering::SeqCst)
    }

    pub fn fail_sets(&self, fail: bool) {
        self.fail_sets.store(fail, Ordering::SeqCst);
    }

    /// Replaces the pads present and tells the watcher, as a hot-plug would.
    pub fn set_pads(&self, pads: Vec<DiscoveredPad>) {
        *self.pads.lock().unwrap() = pads;
        let notify = self.notify.lock().unwrap().clone();
        if let Some(notify) = notify {
            notify();
        }
    }
}

impl RumbleBackend for MockBackend {
    fn name(&self) -> &'static str {
        "mock"
    }

    fn scan(&self) -> Vec<DiscoveredPad> {
        self.scans.fetch_add(1, Ordering::SeqCst);
        self.pads.lock().unwrap().clone()
    }

    fn set(&self, key: &str, levels: Levels, hold_ms: u64) -> Result<()> {
        if self.fail_sets.load(Ordering::SeqCst) {
            return Err(Error::UnknownPad(key.to_string()));
        }
        self.calls.lock().unwrap().push(Call::Set {
            key: key.to_string(),
            levels,
            hold_ms,
            at_ms: self.started.elapsed().as_millis(),
        });
        Ok(())
    }

    fn silence(&self, key: &str) -> Result<()> {
        self.calls.lock().unwrap().push(Call::Silence {
            key: key.to_string(),
            at_ms: self.started.elapsed().as_millis(),
        });
        Ok(())
    }

    fn reset(&self, key: &str) {
        self.calls.lock().unwrap().push(Call::Reset {
            key: key.to_string(),
        });
    }

    fn watch(&self, notify: Notify) {
        *self.notify.lock().unwrap() = Some(notify);
    }
}
