// The plugin's state: the pads it can address, and what it plays on them
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

use std::sync::{Arc, Mutex, Weak};

use crate::{
    backend::RumbleBackend,
    config::Config,
    models::*,
    normalise::{merge_reasons, plan_play, Plan},
    player::Stepper,
    registry::Registry,
    Error, Result,
};

/// Receives pad events, which the plugin forwards to the webview.
pub type Emit = Arc<dyn Fn(PadEvent) + Send + Sync>;

struct Inner {
    backend: Arc<dyn RumbleBackend>,
    stepper: Stepper,
    registry: Mutex<Registry>,
    /// Held for a whole refresh, so events leave in the order the registry produced them.
    refresh_lock: Mutex<()>,
    config: Config,
    emit: Emit,
}

impl Inner {
    /// Rescans the pads, silences any that left, and emits what changed.
    fn refresh(&self) {
        let Ok(_ordered) = self.refresh_lock.lock() else {
            return;
        };
        // Scanning touches the disk, so it happens before the registry is locked.
        let scan = self.backend.scan();
        let (events, departed, arrived) = match self.registry.lock() {
            Ok(mut registry) => {
                let events = registry.refresh(self.backend.name(), scan);
                (events, registry.take_departed(), registry.take_arrived())
            }
            Err(_) => return,
        };
        for key in departed {
            self.stepper.stop(&key);
        }
        for key in arrived {
            self.backend.reset(&key);
        }
        for event in events {
            (self.emit)(event);
        }
    }

    fn lookup(&self, id: &str) -> Option<(String, PadInfo)> {
        self.registry.lock().ok()?.find(id)
    }
}

pub struct GamepadHaptics {
    inner: Arc<Inner>,
}

impl GamepadHaptics {
    pub fn new(backend: Arc<dyn RumbleBackend>, config: Config, emit: Emit) -> Self {
        let inner = Arc::new(Inner {
            stepper: Stepper::new(Arc::clone(&backend)),
            backend,
            registry: Mutex::new(Registry::default()),
            refresh_lock: Mutex::new(()),
            config,
            emit,
        });
        let weak: Weak<Inner> = Arc::downgrade(&inner);
        inner.backend.watch(Arc::new(move || {
            if let Some(inner) = weak.upgrade() {
                inner.refresh();
            }
        }));
        inner.refresh();
        Self { inner }
    }

    pub fn config(&self) -> &Config {
        &self.inner.config
    }

    pub fn capabilities(&self) -> Result<Capabilities> {
        Ok(Capabilities {
            platform: std::env::consts::OS.to_string(),
            backend: self.inner.backend.name().to_string(),
            limits: self.inner.config.limits(),
            // The hot-plug watcher keeps the registry current, so this does not rescan; `list_pads`
            // does, for backends that cannot watch.
            pads: self.snapshot(),
        })
    }

    fn snapshot(&self) -> Vec<PadInfo> {
        self.inner
            .registry
            .lock()
            .map(|registry| registry.pads())
            .unwrap_or_default()
    }

    /// Rescans, then lists the pads present.
    pub fn list_pads(&self) -> Result<Vec<PadInfo>> {
        self.inner.refresh();
        Ok(self.snapshot())
    }

    pub fn play_frames(&self, args: PlayFramesArgs) -> Result<PlayResult> {
        let (key, pad) = match self.inner.lookup(&args.pad_id) {
            Some(found) => found,
            None => {
                self.inner.refresh();
                self.inner
                    .lookup(&args.pad_id)
                    .ok_or_else(|| Error::UnknownPad(args.pad_id.clone()))?
            }
        };
        let plan = plan_play(
            &args,
            &pad,
            &self.inner.config.limits(),
            self.inner.config.master_scale(),
        )?;
        match plan {
            Plan::Silent(reason) => Ok(PlayResult::silent(pad.id, reason)),
            Plan::Play { play, reasons } => {
                let play = play.into_inner();
                self.inner.stepper.play(&key, &play.frames);
                let result = PlayResult {
                    ok: true,
                    tier: play.tier,
                    target: pad.id,
                    downgraded: false,
                    reason: None,
                };
                Ok(merge_reasons(result, &reasons))
            }
        }
    }

    /// Buzzes one pad in a pattern that tells it from the others: heavy, light, heavy.
    pub fn identify(&self, pad_id: &str) -> Result<PlayResult> {
        let pulse = |heavy: f64, light: f64| Frame {
            duration_ms: 120,
            heavy,
            light,
            left_trigger: None,
            right_trigger: None,
        };
        let gap = Frame {
            duration_ms: 80,
            heavy: 0.0,
            light: 0.0,
            left_trigger: None,
            right_trigger: None,
        };
        self.play_frames(PlayFramesArgs {
            pad_id: pad_id.to_string(),
            frames: vec![
                pulse(0.8, 0.0),
                gap.clone(),
                pulse(0.0, 0.8),
                gap,
                pulse(0.8, 0.0),
            ],
            scale: Some(1.0),
        })
    }

    /// Stops one pad, or every pad when `pad_id` is absent.
    pub fn stop(&self, pad_id: Option<&str>) -> Result<()> {
        match pad_id {
            None => self.inner.stepper.stop_all(),
            Some(id) => {
                let (key, _) = self
                    .inner
                    .lookup(id)
                    .ok_or_else(|| Error::UnknownPad(id.to_string()))?;
                self.inner.stepper.stop(&key);
            }
        }
        Ok(())
    }

    pub fn stop_all(&self) {
        self.inner.stepper.stop_all();
    }
}

impl Drop for GamepadHaptics {
    fn drop(&mut self) {
        self.inner.stepper.stop_all();
    }
}

#[cfg(test)]
mod tests {
    use std::{sync::Mutex as StdMutex, thread, time::Duration};

    use super::*;
    use crate::{
        mock::{Call, MockBackend},
        registry::tests::pad,
    };

    fn frame(duration_ms: u64, heavy: f64, light: f64) -> Frame {
        Frame {
            duration_ms,
            heavy,
            light,
            left_trigger: None,
            right_trigger: None,
        }
    }

    fn setup(
        pads: Vec<crate::backend::DiscoveredPad>,
    ) -> (
        GamepadHaptics,
        Arc<MockBackend>,
        Arc<StdMutex<Vec<PadEvent>>>,
    ) {
        let backend = Arc::new(MockBackend::new(pads));
        let events = Arc::new(StdMutex::new(Vec::new()));
        let sink = Arc::clone(&events);
        let emit: Emit = Arc::new(move |e| sink.lock().unwrap().push(e));
        let haptics = GamepadHaptics::new(backend.clone(), Config::default(), emit);
        (haptics, backend, events)
    }

    #[test]
    fn pads_present_at_start_are_announced() {
        let (haptics, _, events) = setup(vec![pad("a")]);
        assert_eq!(haptics.list_pads().unwrap().len(), 1);
        assert!(matches!(events.lock().unwrap()[0], PadEvent::Connected(_)));
    }

    #[test]
    fn hot_plug_is_announced_through_the_backend_watch() {
        let (haptics, backend, events) = setup(vec![]);
        backend.set_pads(vec![pad("a")]);
        assert_eq!(events.lock().unwrap().len(), 1);
        backend.set_pads(vec![]);
        assert!(matches!(
            events.lock().unwrap()[1],
            PadEvent::Disconnected { .. }
        ));
        assert!(haptics.list_pads().unwrap().is_empty());
    }

    #[test]
    fn capabilities_read_the_registry_without_rescanning() {
        let (haptics, backend, _) = setup(vec![pad("a")]);
        let scans = backend.scans();
        assert_eq!(haptics.capabilities().unwrap().pads.len(), 1);
        assert_eq!(backend.scans(), scans);
        haptics.list_pads().unwrap();
        assert_eq!(backend.scans(), scans + 1);
    }

    #[test]
    fn a_pad_is_reset_once_when_it_appears_and_not_when_it_stays() {
        let (haptics, backend, _) = setup(vec![pad("a")]);
        assert_eq!(backend.resets(), ["a"]);
        haptics.list_pads().unwrap();
        assert_eq!(backend.resets(), ["a"]);
        backend.set_pads(vec![pad("a"), pad("b")]);
        assert_eq!(backend.resets(), ["a", "b"]);
        backend.set_pads(vec![pad("b")]);
        backend.set_pads(vec![pad("a"), pad("b")]);
        assert_eq!(backend.resets(), ["a", "b", "a"]);
    }

    #[test]
    fn a_valid_request_plays_and_reports_its_tier() {
        let (haptics, backend, _) = setup(vec![pad("a")]);
        let res = haptics
            .play_frames(PlayFramesArgs {
                pad_id: "gamepad:0".into(),
                frames: vec![frame(40, 0.5, 1.0)],
                scale: None,
            })
            .unwrap();
        assert_eq!((res.tier, res.downgraded), (2, false));
        thread::sleep(Duration::from_millis(120));
        haptics.stop(None).unwrap();
        assert!(matches!(backend.calls()[0], Call::Set { .. }));
    }

    #[test]
    fn silent_and_invalid_requests_never_reach_the_backend() {
        let (haptics, backend, _) = setup(vec![pad("a")]);
        let silent = haptics
            .play_frames(PlayFramesArgs {
                pad_id: "gamepad:0".into(),
                frames: vec![frame(40, 0.5, 1.0)],
                scale: Some(0.0),
            })
            .unwrap();
        assert_eq!(silent.tier, 0);
        let invalid = haptics.play_frames(PlayFramesArgs {
            pad_id: "gamepad:0".into(),
            frames: vec![frame(0, 0.5, 1.0)],
            scale: None,
        });
        assert!(matches!(invalid, Err(Error::InvalidRequest(_))));
        assert!(backend.calls().is_empty());
    }

    #[test]
    fn an_unknown_pad_is_an_error() {
        let (haptics, _, _) = setup(vec![]);
        let res = haptics.play_frames(PlayFramesArgs {
            pad_id: "gamepad:3".into(),
            frames: vec![frame(40, 0.5, 1.0)],
            scale: None,
        });
        assert!(matches!(res, Err(Error::UnknownPad(_))));
        assert!(matches!(
            haptics.stop(Some("gamepad:3")),
            Err(Error::UnknownPad(_))
        ));
    }

    #[test]
    fn unplugging_a_pad_silences_it() {
        let (haptics, backend, _) = setup(vec![pad("a")]);
        haptics
            .play_frames(PlayFramesArgs {
                pad_id: "gamepad:0".into(),
                frames: vec![frame(2_000, 1.0, 1.0)],
                scale: None,
            })
            .unwrap();
        thread::sleep(Duration::from_millis(30));
        backend.set_pads(vec![]);
        assert!(matches!(backend.calls().last(), Some(Call::Silence { key, .. }) if key == "a"));
    }

    #[test]
    fn identify_plays_a_distinct_pattern() {
        let (haptics, backend, _) = setup(vec![pad("a")]);
        let res = haptics.identify("gamepad:0").unwrap();
        assert_eq!(res.tier, 2);
        thread::sleep(Duration::from_millis(60));
        haptics.stop_all();
        assert!(!backend.calls().is_empty());
    }
}
