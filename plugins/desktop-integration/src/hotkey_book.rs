// The bookkeeping of the Windows hotkey thread, kept free of the Windows API so it can be tested
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

use std::{
    collections::HashMap,
    sync::{
        atomic::{AtomicU8, Ordering},
        mpsc, Arc,
    },
    thread::JoinHandle,
    time::Duration,
};

use crate::{error::ServiceError, shortcuts::Accelerator};

/// What the book needs from the system: `RegisterHotKey` and `UnregisterHotKey` on the hotkey
/// window, or a fake in tests.
pub trait HotkeyApi {
    fn register(&mut self, hotkey: i32, accelerator: Accelerator) -> Result<(), ServiceError>;
    fn unregister(&mut self, hotkey: i32) -> Result<(), ServiceError>;
}

/// Which shortcut id owns which hotkey number, and the accelerator behind each, so a failed
/// replacement can put the old binding back.
#[derive(Debug)]
pub struct HotkeyBook {
    next_hotkey: i32,
    by_name: HashMap<String, (i32, Accelerator)>,
    by_hotkey: HashMap<i32, String>,
}

impl Default for HotkeyBook {
    fn default() -> Self {
        Self {
            // Hotkey numbers are small and private to the hotkey window.
            next_hotkey: 1,
            by_name: HashMap::new(),
            by_hotkey: HashMap::new(),
        }
    }
}

impl HotkeyBook {
    /// Registers `accelerator` as `id`, replacing an earlier binding of the same id.
    ///
    /// The earlier binding is released first, because Windows refuses a combination that is
    /// already registered, even to the same window: re-registering the same accelerator would
    /// otherwise always fail. If the new registration fails the earlier binding is restored, so a
    /// failed replacement leaves the shortcut as it was.
    pub fn register(
        &mut self,
        api: &mut impl HotkeyApi,
        id: &str,
        accelerator: Accelerator,
    ) -> Result<(), ServiceError> {
        let previous = self.by_name.get(id).copied();
        if let Some((old, _)) = previous {
            let _ = api.unregister(old);
        }
        let hotkey = self.next_hotkey;
        match api.register(hotkey, accelerator) {
            Ok(()) => {
                self.next_hotkey += 1;
                if let Some((old, _)) = previous {
                    self.by_hotkey.remove(&old);
                }
                self.by_name.insert(id.to_string(), (hotkey, accelerator));
                self.by_hotkey.insert(hotkey, id.to_string());
                Ok(())
            }
            Err(error) => {
                if let Some((old, old_accelerator)) = previous {
                    if api.register(old, old_accelerator).is_err() {
                        // Another process took the combination in between; the id is unbound.
                        self.by_name.remove(id);
                        self.by_hotkey.remove(&old);
                    }
                }
                Err(error)
            }
        }
    }

    /// Releases the hotkey registered as `id`.
    pub fn unregister(&mut self, api: &mut impl HotkeyApi, id: &str) -> Result<(), ServiceError> {
        match self.by_name.remove(id) {
            Some((hotkey, _)) => {
                self.by_hotkey.remove(&hotkey);
                api.unregister(hotkey)
            }
            None => Err(ServiceError::not_found(format!(
                "no shortcut is registered as {id:?}"
            ))),
        }
    }

    /// The shortcut id behind a pressed hotkey number.
    pub fn id_for(&self, hotkey: i32) -> Option<&str> {
        self.by_hotkey.get(&hotkey).map(String::as_str)
    }

    /// Releases every hotkey.
    pub fn unregister_all(&mut self, api: &mut impl HotkeyApi) {
        for (hotkey, _) in self.by_name.drain().map(|(_, binding)| binding) {
            let _ = api.unregister(hotkey);
        }
        self.by_hotkey.clear();
    }
}

const PENDING: u8 = 0;
const RUNNING: u8 = 1;
const CANCELLED: u8 = 2;

/// Shared between a caller waiting on the hotkey thread and the command it queued: whichever of
/// the thread starting the command and the caller giving up claims the ticket first wins, so a
/// command the caller reported as timed out is never run afterwards.
#[derive(Debug, Clone, Default)]
pub struct Ticket(Arc<AtomicU8>);

impl Ticket {
    /// The thread's claim before running the command; `false` means the caller already gave up.
    pub fn start(&self) -> bool {
        self.0
            .compare_exchange(PENDING, RUNNING, Ordering::AcqRel, Ordering::Acquire)
            .is_ok()
    }

    /// The caller's claim when it stops waiting; `false` means the command is already running and
    /// its result is worth waiting for.
    pub fn cancel(&self) -> bool {
        self.0
            .compare_exchange(PENDING, CANCELLED, Ordering::AcqRel, Ordering::Acquire)
            .is_ok()
    }
}

/// Joins a thread that signals its end by dropping or sending on `done`, waiting at most
/// `timeout`. A thread that does not end in time is left running and `false` is returned.
pub fn join_within(join: JoinHandle<()>, done: &mpsc::Receiver<()>, timeout: Duration) -> bool {
    match done.recv_timeout(timeout) {
        Ok(()) | Err(mpsc::RecvTimeoutError::Disconnected) => {
            let _ = join.join();
            true
        }
        Err(mpsc::RecvTimeoutError::Timeout) => false,
    }
}

#[cfg(test)]
mod tests {
    use std::collections::HashSet;

    use super::*;
    use crate::error::ServiceErrorKind;

    /// Behaves like Windows: one registration per combination, even for the same window.
    #[derive(Default)]
    struct Fake {
        held: HashMap<i32, Accelerator>,
        /// Combinations another process holds.
        foreign: HashSet<u32>,
    }

    impl HotkeyApi for Fake {
        fn register(&mut self, hotkey: i32, accelerator: Accelerator) -> Result<(), ServiceError> {
            let taken = self.foreign.contains(&accelerator.key)
                || self.held.values().any(|held| *held == accelerator);
            if taken {
                return Err(ServiceError::new(ServiceErrorKind::Conflict, "taken"));
            }
            self.held.insert(hotkey, accelerator);
            Ok(())
        }

        fn unregister(&mut self, hotkey: i32) -> Result<(), ServiceError> {
            self.held.remove(&hotkey);
            Ok(())
        }
    }

    fn accelerator(key: u32) -> Accelerator {
        Accelerator { modifiers: 2, key }
    }

    #[test]
    fn registering_an_id_again_with_the_same_accelerator_replaces_it() {
        let (mut book, mut api) = (HotkeyBook::default(), Fake::default());
        book.register(&mut api, "toggle", accelerator(75)).unwrap();
        book.register(&mut api, "toggle", accelerator(75)).unwrap();
        assert_eq!(api.held.len(), 1);
        let hotkey = *api.held.keys().next().unwrap();
        assert_eq!(book.id_for(hotkey), Some("toggle"));
    }

    #[test]
    fn a_new_accelerator_replaces_the_old_one() {
        let (mut book, mut api) = (HotkeyBook::default(), Fake::default());
        book.register(&mut api, "toggle", accelerator(75)).unwrap();
        book.register(&mut api, "toggle", accelerator(76)).unwrap();
        assert_eq!(
            api.held.values().copied().collect::<Vec<_>>(),
            [accelerator(76)]
        );
        assert_eq!(api.held.len(), 1);
    }

    #[test]
    fn a_failed_replacement_keeps_the_old_binding() {
        let (mut book, mut api) = (HotkeyBook::default(), Fake::default());
        book.register(&mut api, "toggle", accelerator(75)).unwrap();
        api.foreign.insert(76);
        let error = book
            .register(&mut api, "toggle", accelerator(76))
            .unwrap_err();
        assert_eq!(error.kind, ServiceErrorKind::Conflict);
        assert_eq!(
            api.held.values().copied().collect::<Vec<_>>(),
            [accelerator(75)]
        );
        let hotkey = *api.held.keys().next().unwrap();
        assert_eq!(book.id_for(hotkey), Some("toggle"));
        // The restored binding is still the one to replace or remove.
        book.unregister(&mut api, "toggle").unwrap();
        assert!(api.held.is_empty());
    }

    #[test]
    fn a_failed_first_registration_leaves_nothing_behind() {
        let (mut book, mut api) = (HotkeyBook::default(), Fake::default());
        api.foreign.insert(75);
        assert!(book.register(&mut api, "toggle", accelerator(75)).is_err());
        assert_eq!(
            book.unregister(&mut api, "toggle").unwrap_err().kind,
            ServiceErrorKind::NotFound
        );
    }

    #[test]
    fn unregistering_all_releases_every_hotkey() {
        let (mut book, mut api) = (HotkeyBook::default(), Fake::default());
        book.register(&mut api, "a", accelerator(65)).unwrap();
        book.register(&mut api, "b", accelerator(66)).unwrap();
        book.unregister_all(&mut api);
        assert!(api.held.is_empty());
        assert_eq!(book.id_for(1), None);
    }

    #[test]
    fn a_caller_that_gave_up_stops_the_command_from_running() {
        let ticket = Ticket::default();
        assert!(ticket.cancel());
        assert!(!ticket.start());
    }

    #[test]
    fn a_command_already_running_cannot_be_cancelled() {
        let ticket = Ticket::default();
        assert!(ticket.start());
        assert!(!ticket.cancel());
    }

    #[test]
    fn join_within_waits_for_a_thread_that_ends() {
        let (done_tx, done_rx) = mpsc::channel();
        let join = std::thread::spawn(move || drop(done_tx));
        assert!(join_within(join, &done_rx, Duration::from_secs(5)));
    }

    #[test]
    fn join_within_gives_up_on_a_thread_that_hangs() {
        let (done_tx, done_rx) = mpsc::channel::<()>();
        let (release_tx, release_rx) = mpsc::channel::<()>();
        let join = std::thread::spawn(move || {
            let _done = done_tx;
            let _ = release_rx.recv();
        });
        assert!(!join_within(join, &done_rx, Duration::from_millis(50)));
        drop(release_tx);
    }
}
