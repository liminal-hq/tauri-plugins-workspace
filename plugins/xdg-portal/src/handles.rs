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

#[cfg(test)]
mod tests {
    use super::*;

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
