// Runs a periodic tick on a plugin-owned thread and stops it promptly on shutdown
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

use std::{
    sync::mpsc::{self, RecvTimeoutError, Sender},
    thread::JoinHandle,
    time::Duration,
};

/// A named thread that calls `tick` every `interval` until stopped.
pub struct Pump {
    stop: Option<Sender<()>>,
    handle: Option<JoinHandle<()>>,
}

impl Pump {
    #[cfg_attr(not(steam_sdk), allow(dead_code))]
    pub fn start(
        name: &str,
        interval: Duration,
        mut tick: impl FnMut() + Send + 'static,
    ) -> std::io::Result<Self> {
        let (stop, rx) = mpsc::channel::<()>();
        let handle = std::thread::Builder::new()
            .name(name.to_string())
            .spawn(move || {
                // A message or a dropped sender both mean "stop".
                while let Err(RecvTimeoutError::Timeout) = rx.recv_timeout(interval) {
                    tick();
                }
            })?;
        Ok(Self {
            stop: Some(stop),
            handle: Some(handle),
        })
    }

    /// Stops the thread and waits for it to finish. Safe to call more than once.
    pub fn stop(&mut self) {
        if let Some(stop) = self.stop.take() {
            let _ = stop.send(());
        }
        if let Some(handle) = self.handle.take() {
            if handle.join().is_err() {
                log::warn!("steamworks: callback pump thread panicked");
            }
        }
    }
}

impl Drop for Pump {
    fn drop(&mut self) {
        self.stop();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::{
        atomic::{AtomicUsize, Ordering},
        Arc,
    };

    #[test]
    fn ticks_until_stopped_then_stays_quiet() {
        let count = Arc::new(AtomicUsize::new(0));
        let seen = count.clone();
        let mut pump = Pump::start("test-pump", Duration::from_millis(5), move || {
            seen.fetch_add(1, Ordering::SeqCst);
        })
        .unwrap();
        std::thread::sleep(Duration::from_millis(60));
        pump.stop();
        let after_stop = count.load(Ordering::SeqCst);
        assert!(after_stop > 0, "the pump never ticked");
        std::thread::sleep(Duration::from_millis(30));
        assert_eq!(count.load(Ordering::SeqCst), after_stop);
        pump.stop();
    }

    #[test]
    fn stop_returns_promptly_with_a_long_interval() {
        let mut pump = Pump::start("test-pump-long", Duration::from_secs(30), || {}).unwrap();
        let started = std::time::Instant::now();
        pump.stop();
        assert!(started.elapsed() < Duration::from_secs(5));
    }
}
