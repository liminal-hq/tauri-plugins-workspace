// Hands out numeric handles for long-lived portal objects, such as inhibitors
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

use std::collections::BTreeMap;

/// Owns values that a caller later gives back by number.
///
/// Handles count up from 1 and are never reused within one book, so a stale handle from a released
/// value cannot release a newer one.
#[derive(Debug)]
pub struct HandleBook<T> {
    next: u32,
    entries: BTreeMap<u32, T>,
}

impl<T> Default for HandleBook<T> {
    fn default() -> Self {
        Self {
            next: 1,
            entries: BTreeMap::new(),
        }
    }
}

impl<T> HandleBook<T> {
    pub fn insert(&mut self, value: T) -> u32 {
        let handle = self.next;
        self.next = self.next.wrapping_add(1).max(1);
        self.entries.insert(handle, value);
        handle
    }

    pub fn remove(&mut self, handle: u32) -> Option<T> {
        self.entries.remove(&handle)
    }

    /// Puts a value back under the handle it was taken from, so a release that failed can be
    /// tried again with the same handle.
    pub fn restore(&mut self, handle: u32, value: T) {
        self.entries.insert(handle, value);
    }

    /// Takes every value out, oldest first.
    pub fn drain(&mut self) -> Vec<T> {
        std::mem::take(&mut self.entries).into_values().collect()
    }

    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }
}

/// Releases the value behind `handle` with `close`, which gets the value and, if it fails, hands
/// it back with the error. A failed release puts the value back in the book under the same
/// handle, so the holder is not lost: a retry (or the release at exit) still finds it.
///
/// Returns `None` when the handle is unknown or already released.
pub async fn release_with<T, E, Fut>(
    book: &tokio::sync::Mutex<HandleBook<T>>,
    handle: u32,
    close: impl FnOnce(T) -> Fut,
) -> Option<Result<(), E>>
where
    Fut: std::future::Future<Output = Result<(), (T, E)>>,
{
    let value = book.lock().await.remove(handle)?;
    match close(value).await {
        Ok(()) => Some(Ok(())),
        Err((value, error)) => {
            book.lock().await.restore(handle, value);
            Some(Err(error))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn a_failed_release_keeps_the_handle_for_a_retry() {
        let book = tokio::sync::Mutex::new(HandleBook::default());
        let handle = book.lock().await.insert("inhibitor");
        let failed = release_with(&book, handle, |value| async move {
            Err::<(), _>((value, "Close timed out"))
        })
        .await;
        assert_eq!(failed, Some(Err("Close timed out")));
        assert_eq!(book.lock().await.len(), 1);

        let released = release_with(&book, handle, |_| async { Ok::<(), (&str, &str)>(()) }).await;
        assert_eq!(released, Some(Ok(())));
        assert!(book.lock().await.is_empty());

        let again = release_with(&book, handle, |_| async { Ok::<(), (&str, &str)>(()) }).await;
        assert_eq!(again, None, "released handles are gone");
        assert_eq!(book.lock().await.insert("next"), handle + 1);
    }

    #[test]
    fn hands_out_increasing_handles_and_never_reuses_one() {
        let mut book = HandleBook::default();
        let a = book.insert("a");
        let b = book.insert("b");
        assert_eq!((a, b), (1, 2));
        assert_eq!(book.remove(a), Some("a"));
        assert_eq!(book.remove(a), None);
        assert_eq!(book.insert("c"), 3);
        assert_eq!(book.len(), 2);
    }

    #[test]
    fn drain_empties_the_book_oldest_first() {
        let mut book = HandleBook::default();
        book.insert(10);
        book.insert(20);
        assert_eq!(book.drain(), vec![10, 20]);
        assert!(book.is_empty());
    }
}
