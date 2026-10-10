// Steps a pad through its frames on a worker thread and guarantees it ends silent
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

use std::{
    collections::HashMap,
    sync::{
        mpsc::{channel, RecvTimeoutError, Sender},
        Arc, Mutex,
    },
    thread::{self, JoinHandle},
    time::{Duration, Instant},
};

use crate::{
    backend::{Levels, RumbleBackend},
    models::Frame,
};

/// The shortest time between updates to one pad. Bluetooth pads drop or delay faster updates.
pub const MIN_UPDATE_MS: u64 = 16;

/// Extra time a level is held past its frame. The next update normally replaces it first; if the
/// worker stalls, the device falls silent on its own after this.
const HOLD_SLACK_MS: u64 = 50;

/// Levels that differ by less than this are treated as equal.
const SAME_LEVEL: f64 = 0.02;

fn levels_of(frame: &Frame) -> Levels {
    Levels {
        heavy: frame.heavy,
        light: frame.light,
        left_trigger: frame.left_trigger.unwrap_or(0.0),
        right_trigger: frame.right_trigger.unwrap_or(0.0),
    }
}

fn near(a: Levels, b: Levels) -> bool {
    (a.heavy - b.heavy).abs() < SAME_LEVEL
        && (a.light - b.light).abs() < SAME_LEVEL
        && (a.left_trigger - b.left_trigger).abs() < SAME_LEVEL
        && (a.right_trigger - b.right_trigger).abs() < SAME_LEVEL
}

/// One update to send: hold `levels` for `duration_ms`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Update {
    pub levels: Levels,
    pub duration_ms: u64,
}

/// Merges neighbouring frames with nearly equal levels, then folds frames shorter than
/// `MIN_UPDATE_MS` into their neighbours (by time-weighted average), so no pad is updated faster
/// than it can follow. The total duration is unchanged.
pub fn schedule(frames: &[Frame]) -> Vec<Update> {
    let mut merged: Vec<Update> = Vec::new();
    for frame in frames {
        let levels = levels_of(frame);
        match merged.last_mut() {
            Some(last) if near(last.levels, levels) => last.duration_ms += frame.duration_ms,
            _ => merged.push(Update {
                levels,
                duration_ms: frame.duration_ms,
            }),
        }
    }

    // Fold updates shorter than MIN_UPDATE_MS into what follows (or, for a short tail, what
    // precedes), averaging levels by time so the total duration is unchanged.
    let mut out: Vec<Update> = Vec::new();
    let mut pending: Option<Update> = None;
    for update in merged {
        let combined = match pending.take() {
            Some(p) => blend(p, update),
            None => update,
        };
        if combined.duration_ms >= MIN_UPDATE_MS {
            out.push(combined);
        } else {
            pending = Some(combined);
        }
    }
    if let Some(tail) = pending {
        match out.pop() {
            Some(last) => out.push(blend(last, tail)),
            None => out.push(tail),
        }
    }
    out
}

/// Joins two updates into one that lasts as long as both, with their levels averaged by time.
fn blend(a: Update, b: Update) -> Update {
    let (wa, wb) = (a.duration_ms as f64, b.duration_ms as f64);
    let total = wa + wb;
    let mix = |x: f64, y: f64| (x * wa + y * wb) / total;
    Update {
        levels: Levels {
            heavy: mix(a.levels.heavy, b.levels.heavy),
            light: mix(a.levels.light, b.levels.light),
            left_trigger: mix(a.levels.left_trigger, b.levels.left_trigger),
            right_trigger: mix(a.levels.right_trigger, b.levels.right_trigger),
        },
        duration_ms: a.duration_ms + b.duration_ms,
    }
}

struct Job {
    stop: Sender<()>,
    handle: JoinHandle<()>,
}

/// Runs one worker per pad. Starting a pattern on a pad stops the one already playing there.
pub struct Stepper {
    backend: Arc<dyn RumbleBackend>,
    jobs: Mutex<HashMap<String, Job>>,
}

fn run(
    backend: Arc<dyn RumbleBackend>,
    key: String,
    updates: Vec<Update>,
    stop: std::sync::mpsc::Receiver<()>,
) {
    let start = Instant::now();
    let mut elapsed_ms: u64 = 0;
    for update in updates {
        if let Err(e) = backend.set(&key, update.levels, update.duration_ms + HOLD_SLACK_MS) {
            log::warn!("gamepad-haptics: could not drive {key}: {e}");
            break;
        }
        elapsed_ms += update.duration_ms;
        let deadline = start + Duration::from_millis(elapsed_ms);
        match stop.recv_timeout(deadline.saturating_duration_since(Instant::now())) {
            Err(RecvTimeoutError::Timeout) => {}
            // A stop request, or the owner went away: either way, stop now.
            Ok(()) | Err(RecvTimeoutError::Disconnected) => break,
        }
    }
    if let Err(e) = backend.silence(&key) {
        log::warn!("gamepad-haptics: could not silence {key}: {e}");
    }
}

impl Stepper {
    pub fn new(backend: Arc<dyn RumbleBackend>) -> Self {
        Self {
            backend,
            jobs: Mutex::new(HashMap::new()),
        }
    }

    pub fn play(&self, key: &str, frames: &[Frame]) {
        self.stop(key);
        let updates = schedule(frames);
        let (stop, rx) = channel();
        let backend = Arc::clone(&self.backend);
        let owned = key.to_string();
        let handle = thread::spawn(move || run(backend, owned, updates, rx));
        if let Ok(mut jobs) = self.jobs.lock() {
            jobs.insert(key.to_string(), Job { stop, handle });
        }
    }

    /// Stops `key` and waits until its worker has silenced the pad.
    pub fn stop(&self, key: &str) {
        let job = self.jobs.lock().ok().and_then(|mut jobs| jobs.remove(key));
        if let Some(job) = job {
            let _ = job.stop.send(());
            let _ = job.handle.join();
        }
    }

    pub fn stop_all(&self) {
        let jobs: Vec<Job> = match self.jobs.lock() {
            Ok(mut jobs) => jobs.drain().map(|(_, job)| job).collect(),
            Err(_) => return,
        };
        for job in &jobs {
            let _ = job.stop.send(());
        }
        for job in jobs {
            let _ = job.handle.join();
        }
    }
}

impl Drop for Stepper {
    fn drop(&mut self) {
        self.stop_all();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mock::{Call, MockBackend};

    fn frame(duration_ms: u64, heavy: f64, light: f64) -> Frame {
        Frame {
            duration_ms,
            heavy,
            light,
            left_trigger: None,
            right_trigger: None,
        }
    }

    #[test]
    fn schedule_merges_equal_neighbours_and_keeps_the_total() {
        let updates = schedule(&[
            frame(20, 0.5, 0.5),
            frame(30, 0.51, 0.5),
            frame(40, 1.0, 0.0),
        ]);
        assert_eq!(updates.len(), 2);
        assert_eq!(updates[0].duration_ms, 50);
        assert_eq!(updates[1].duration_ms, 40);
    }

    #[test]
    fn schedule_folds_short_frames_into_their_neighbours() {
        let updates = schedule(&[frame(8, 1.0, 0.0), frame(8, 0.0, 1.0), frame(40, 0.5, 0.5)]);
        let total: u64 = updates.iter().map(|u| u.duration_ms).sum();
        assert_eq!(total, 56);
        assert!(updates.iter().all(|u| u.duration_ms >= MIN_UPDATE_MS));
        assert_eq!(updates[0].levels.heavy, 0.5);
    }

    #[test]
    fn schedule_joins_a_short_tail_to_the_update_before_it() {
        let updates = schedule(&[frame(40, 1.0, 1.0), frame(4, 0.0, 0.0)]);
        assert_eq!(updates.len(), 1);
        assert_eq!(updates[0].duration_ms, 44);
    }

    #[test]
    fn a_pattern_plays_in_order_and_ends_silent() {
        let backend = Arc::new(MockBackend::new(vec![]));
        let stepper = Stepper::new(backend.clone());
        stepper.play("a", &[frame(30, 1.0, 0.0), frame(30, 0.0, 1.0)]);
        // Wait for the worker to finish by stopping after the pattern's length.
        thread::sleep(Duration::from_millis(120));
        stepper.stop("a");

        let calls = backend.calls();
        assert!(matches!(calls[0], Call::Set { levels, .. } if levels.heavy == 1.0));
        assert!(matches!(calls[1], Call::Set { levels, .. } if levels.light == 1.0));
        assert!(matches!(calls.last(), Some(Call::Silence { .. })));
        assert_eq!(calls.len(), 3);
    }

    #[test]
    fn stop_interrupts_a_long_pattern_and_silences() {
        let backend = Arc::new(MockBackend::new(vec![]));
        let stepper = Stepper::new(backend.clone());
        stepper.play("a", &[frame(2_000, 1.0, 1.0)]);
        thread::sleep(Duration::from_millis(30));
        let before = Instant::now();
        stepper.stop("a");
        assert!(before.elapsed() < Duration::from_millis(500));
        assert!(matches!(backend.calls().last(), Some(Call::Silence { .. })));
    }

    #[test]
    fn a_new_pattern_replaces_the_one_playing() {
        let backend = Arc::new(MockBackend::new(vec![]));
        let stepper = Stepper::new(backend.clone());
        stepper.play("a", &[frame(2_000, 1.0, 0.0)]);
        thread::sleep(Duration::from_millis(20));
        stepper.play("a", &[frame(30, 0.0, 1.0)]);
        thread::sleep(Duration::from_millis(100));
        stepper.stop_all();
        let calls = backend.calls();
        let silences = calls
            .iter()
            .filter(|c| matches!(c, Call::Silence { .. }))
            .count();
        assert_eq!(silences, 2);
    }

    #[test]
    fn dropping_the_stepper_silences_every_pad() {
        let backend = Arc::new(MockBackend::new(vec![]));
        {
            let stepper = Stepper::new(backend.clone());
            stepper.play("a", &[frame(2_000, 1.0, 1.0)]);
            stepper.play("b", &[frame(2_000, 1.0, 1.0)]);
            thread::sleep(Duration::from_millis(20));
        }
        let silences = backend
            .calls()
            .iter()
            .filter(|c| matches!(c, Call::Silence { .. }))
            .count();
        assert_eq!(silences, 2);
    }

    #[test]
    fn a_failing_backend_ends_the_pattern_silent() {
        let backend = Arc::new(MockBackend::new(vec![]));
        backend.fail_sets(true);
        let stepper = Stepper::new(backend.clone());
        stepper.play("a", &[frame(500, 1.0, 1.0)]);
        thread::sleep(Duration::from_millis(40));
        stepper.stop("a");
        assert!(matches!(backend.calls().last(), Some(Call::Silence { .. })));
    }
}
