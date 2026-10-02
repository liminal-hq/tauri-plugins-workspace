// Keeps one background task running, starting it again once it has ended
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

use std::future::Future;

use tauri::async_runtime::JoinHandle;
use tokio::sync::Mutex;

/// Makes sure the task in `slot` is running, calling `start` when the slot is empty or its task
/// has ended (a signal stream closed, the bus connection dropped), so a listener that stopped is
/// started again by the next call and not lost for the rest of the process. A running task is
/// left alone; if `start` fails the slot is left empty.
pub async fn ensure_running<E, Fut>(
    slot: &Mutex<Option<JoinHandle<()>>>,
    start: impl FnOnce() -> Fut,
) -> Result<(), E>
where
    Fut: Future<Output = Result<JoinHandle<()>, E>>,
{
    let mut slot = slot.lock().await;
    if slot
        .as_ref()
        .is_some_and(|task| !task.inner().is_finished())
    {
        return Ok(());
    }
    *slot = None;
    *slot = Some(start().await?);
    Ok(())
}

#[cfg(test)]
mod tests {
    use std::{
        sync::atomic::{AtomicU32, Ordering},
        time::Duration,
    };

    use super::*;

    async fn ended(slot: &Mutex<Option<JoinHandle<()>>>) {
        while !slot.lock().await.as_ref().unwrap().inner().is_finished() {
            tokio::time::sleep(Duration::from_millis(5)).await;
        }
    }

    #[tokio::test]
    async fn a_listener_whose_stream_ended_is_started_again() {
        let slot = Mutex::new(None);
        let starts = AtomicU32::new(0);
        let start = || async {
            starts.fetch_add(1, Ordering::SeqCst);
            // A signal stream that delivers one item and then closes.
            let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel();
            tx.send(1).unwrap();
            drop(tx);
            Ok::<_, ()>(tauri::async_runtime::spawn(async move {
                while rx.recv().await.is_some() {}
            }))
        };
        ensure_running(&slot, start).await.unwrap();
        ended(&slot).await;
        ensure_running(&slot, start).await.unwrap();
        assert_eq!(starts.load(Ordering::SeqCst), 2);
    }

    #[tokio::test]
    async fn a_running_listener_is_not_started_twice() {
        let slot = Mutex::new(None);
        let starts = AtomicU32::new(0);
        let start = || async {
            starts.fetch_add(1, Ordering::SeqCst);
            Ok::<_, ()>(tauri::async_runtime::spawn(std::future::pending()))
        };
        ensure_running(&slot, start).await.unwrap();
        ensure_running(&slot, start).await.unwrap();
        assert_eq!(starts.load(Ordering::SeqCst), 1);
        slot.lock().await.take().unwrap().abort();
    }

    #[tokio::test]
    async fn a_failed_start_leaves_the_slot_empty_for_the_next_call() {
        let slot = Mutex::new(None);
        assert_eq!(
            ensure_running(&slot, || async { Err::<JoinHandle<()>, _>("no bus") }).await,
            Err("no bus")
        );
        assert!(slot.lock().await.is_none());
    }
}
