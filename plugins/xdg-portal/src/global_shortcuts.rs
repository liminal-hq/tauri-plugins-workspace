// Implements the GlobalShortcuts portal for Wayland shortcut registration
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

use crate::error::PortalError;
use ashpd::WindowIdentifier;
use futures_util::StreamExt;
use log::{error, info, warn};

/// Translates a Tauri-style shortcut string (e.g. `"Alt+Shift+E"`) into the
/// GTK/libxkbcommon accelerator format expected by the portal (`"<Alt><Shift>e"`).
fn to_xdg_trigger(shortcut: &str) -> String {
    let mut result = String::new();
    let mut key = String::new();

    // Collect parts so we can distinguish a non-trailing empty token (which
    // represents the "+" key itself, e.g. "Ctrl++" → ["Ctrl","",""]) from a
    // trailing empty produced by split when the string ends with "+".
    let parts: Vec<&str> = shortcut.split('+').collect();
    let last = parts.len().saturating_sub(1);

    for (i, part) in parts.iter().enumerate() {
        match *part {
            "Alt" => result.push_str("<Alt>"),
            "Shift" => result.push_str("<Shift>"),
            "Ctrl" | "Control" => result.push_str("<Ctrl>"),
            "Super" | "Meta" => result.push_str("<Super>"),
            // XKB keysym names are case-sensitive: single-character keys use lowercase,
            // "space" is lowercase, but named keys (Tab, Return, F1, Left, …) must
            // preserve their original casing.
            "Space" => key = "space".to_string(),
            "" if i < last => {
                // Non-trailing empty token: the "+" character is the key.
                key = "plus".to_string();
            }
            "" => {} // trailing empty after a final '+', skip
            other => {
                key = if other.len() == 1 {
                    other.to_lowercase()
                } else {
                    other.to_string()
                };
            }
        }
    }

    result.push_str(&key);
    result
}

/// Finds the trigger description for `id` among `(id, trigger_description)` pairs.
/// Generic over plain string pairs rather than ashpd's own `Shortcut` type — its
/// fields are private to ashpd, making it unconstructable (and so untestable) from
/// this crate directly; this indirection is what makes selection logic testable.
fn find_trigger_description<'a>(
    shortcuts: impl IntoIterator<Item = (&'a str, &'a str)>,
    id: &str,
) -> Option<&'a str> {
    shortcuts
        .into_iter()
        .find(|(sid, _)| *sid == id)
        .map(|(_, desc)| desc)
}

/// Creates a GlobalShortcuts portal session.
///
/// `shortcut_id` must be unique among every `GlobalShortcuts` session alive on the
/// session bus at once — not just within this process. Both the activation stream
/// and `on_shortcuts_changed` (below) can only filter events by this id, because
/// `ashpd::desktop::Session` exposes no public accessor for a session's own D-Bus
/// object path to compare against the session handle these signals also carry
/// (`Activated`/`ShortcutsChanged` both include one; there is no way from outside
/// ashpd to check it against "is this actually my session"). A collision with
/// another session's id — this app's or another app's — means this listener will
/// react to that session's activations and trigger changes as if they were its own.
///
/// `window_rx` must deliver the `WindowIdentifier` for the parent window once
/// it becomes available — the portal's `BindShortcuts` call is deferred until
/// then.  Send `None` to bind without a parent window (portal dialog will be
/// unanchored).
///
/// `on_binding_result` is called once binding completes (or fails).
/// `on_activated` is called each time the shortcut fires.
/// `on_shortcuts_changed` is called with the new trigger description (e.g.
/// `"Super+E"`) whenever the compositor's own settings UI reports the shortcut's
/// trigger has changed independently of this app (the `ShortcutsChanged` portal
/// signal) — e.g. GNOME Settings → Apps → <App> → Global Shortcuts. Called inline
/// from the listener loop, in signal-arrival order — not spawned onto a separate
/// thread the way `on_activated` is, since it only does cheap, order-sensitive work
/// (a state write, not window creation) and correctness here depends on later
/// signals never being applied before earlier ones. There is no confirmed
/// behaviour for a shortcut being removed entirely (as opposed to rebound) via that
/// UI — not reproduced, deliberately left unhandled rather than guessed at.
///
/// Returns a `ShortcutHandle`; dropping it cancels the listener and closes the
/// portal session.
pub async fn create_session<F, B, C>(
    shortcut_id: &str,
    description: &str,
    preferred_trigger: Option<&str>,
    on_activated: F,
    on_binding_result: B,
    on_shortcuts_changed: C,
    window_rx: tokio::sync::oneshot::Receiver<Option<WindowIdentifier>>,
) -> Result<ShortcutHandle, PortalError>
where
    // Sync is required so Arc<F> is Send, allowing activation dispatch to an OS
    // thread rather than blocking the Tokio worker during window creation.
    F: Fn() + Send + Sync + 'static,
    B: Fn(Result<(), String>) + Send + 'static,
    // Only Send, not Sync, like B — called inline from the select! loop, never
    // cloned across threads (see the loop body for why that's safe here).
    C: Fn(String) + Send + 'static,
{
    use ashpd::desktop::global_shortcuts::{GlobalShortcuts, NewShortcut};

    let portal = GlobalShortcuts::new().await.map_err(|e| {
        PortalError::Internal(format!("failed to connect to GlobalShortcuts portal: {e}"))
    })?;

    let session = portal.create_session().await.map_err(|e| {
        PortalError::Internal(format!("failed to create GlobalShortcuts session: {e}"))
    })?;

    let trigger_xdg = preferred_trigger.map(to_xdg_trigger);
    let shortcut = {
        let s = NewShortcut::new(shortcut_id, description);
        if let Some(ref t) = trigger_xdg {
            s.preferred_trigger(t.as_str())
        } else {
            s
        }
    };

    let activated_stream = portal
        .receive_activated()
        .await
        .map_err(|e| PortalError::Internal(format!("failed to subscribe to activations: {e}")))?;

    let shortcuts_changed_stream = portal.receive_shortcuts_changed().await.map_err(|e| {
        PortalError::Internal(format!("failed to subscribe to shortcut changes: {e}"))
    })?;

    let (cancel_tx, mut cancel_rx) = tokio::sync::oneshot::channel::<()>();
    let sid = shortcut_id.to_string();
    // Wrap in Arc so we can clone into each per-activation OS thread without
    // moving or blocking the Tokio worker during WebviewWindowBuilder::build().
    // on_shortcuts_changed is NOT wrapped this way — it's called inline from the
    // select! loop (see below), never cloned across threads.
    let on_activated = std::sync::Arc::new(on_activated);

    tokio::spawn(async move {
        // Keep portal and session alive for the lifetime of this task.
        let _portal = portal;
        let _session = session;

        // Wait for the window identifier before binding.
        let window_id = tokio::select! {
            result = window_rx => match result {
                Ok(id) => id.unwrap_or_default(),
                // Sender dropped without a send (e.g. plugin teardown before first window).
                Err(_) => {
                    warn!("shortcut window sender dropped; aborting portal binding");
                    return;
                }
            },
            _ = &mut cancel_rx => return,
        };

        let bind_result = _portal
            .bind_shortcuts(&_session, &[shortcut], &window_id)
            .await;

        match bind_result {
            Ok(request) => match request.response() {
                Ok(resp) => {
                    if resp.shortcuts().is_empty() {
                        warn!(
                            "portal bind succeeded but returned no shortcuts — \
                             the key combination may already be claimed"
                        );
                        on_binding_result(
                            Err("compositor returned no bound shortcuts".to_string()),
                        );
                        return;
                    }
                    info!(
                        "global shortcuts bound: {:?}",
                        resp.shortcuts().iter().map(|s| s.id()).collect::<Vec<_>>()
                    );
                    on_binding_result(Ok(()));
                }
                Err(e) => {
                    error!("bind_shortcuts portal response error: {e}");
                    on_binding_result(Err(e.to_string()));
                    return;
                }
            },
            Err(e) => {
                error!("bind_shortcuts D-Bus call failed: {e}");
                on_binding_result(Err(e.to_string()));
                return;
            }
        }

        tokio::pin!(activated_stream);
        tokio::pin!(shortcuts_changed_stream);
        // Set once the ShortcutsChanged stream ends, to disable that select! branch
        // without tearing down activation delivery — unlike activated_stream ending
        // (which is fatal to the session's whole purpose), this is a secondary,
        // best-effort channel that some compositor backends may not implement at all.
        let mut changed_stream_ended = false;

        loop {
            tokio::select! {
                event = activated_stream.next() => {
                    match event {
                        Some(event) => {
                            if event.shortcut_id() == sid {
                                info!("global shortcut activated: {}", sid);
                                let f = std::sync::Arc::clone(&on_activated);
                                std::thread::spawn(move || f());
                            }
                        }
                        // Stream closed (compositor crash, D-Bus drop) — exit cleanly.
                        None => {
                            warn!("global shortcut activation stream ended for: {}", sid);
                            break;
                        }
                    }
                }
                event = shortcuts_changed_stream.next(), if !changed_stream_ended => {
                    match event {
                        Some(event) => {
                            // Filtered by shortcut id only, not event.session_handle() — see
                            // create_session's doc comment for why: ashpd::desktop::Session
                            // exposes no public accessor for a session's own path to compare
                            // it against. A real fix would mean bypassing ashpd's ergonomic
                            // API for raw zbus calls just to capture that handle ourselves; a
                            // deliberate call not to do that here, since this exact gap already
                            // existed in the activation branch above, unrelated to this PR. If
                            // that fix is ever wanted, it needs a raw-zbus session-path capture
                            // at create_session() and to apply here and to event.shortcut_id()
                            // above alike, not just one of the two.
                            let pairs = event.shortcuts().iter().map(|s| (s.id(), s.trigger_description()));
                            if let Some(desc) = find_trigger_description(pairs, &sid) {
                                info!("global shortcut trigger changed externally: {} -> {}", sid, desc);
                                // Called inline, not via std::thread::spawn like on_activated —
                                // this callback only does a mutex write and an event emit (no
                                // blocking window-creation work to justify the OS-thread hop),
                                // and calling it inline guarantees in-order delivery: this select!
                                // loop processes one branch at a time, so a later signal can never
                                // be applied before an earlier one the way two independently
                                // scheduled threads could race.
                                on_shortcuts_changed(desc.to_string());
                            }
                        }
                        None => {
                            warn!("shortcuts-changed stream ended for: {}", sid);
                            changed_stream_ended = true;
                        }
                    }
                }
                _ = &mut cancel_rx => {
                    info!("global shortcut listener cancelled for: {}", sid);
                    break;
                }
            }
        }
    });

    Ok(ShortcutHandle { _cancel: cancel_tx })
}

/// Dropping this handle cancels the shortcut listener and closes the portal session.
pub struct ShortcutHandle {
    _cancel: tokio::sync::oneshot::Sender<()>,
}

#[cfg(test)]
mod tests {
    use super::{find_trigger_description, to_xdg_trigger};

    #[test]
    fn find_trigger_description_matches_by_id() {
        let shortcuts = [("other-id", "Ctrl+X"), ("emoji-nook-toggle", "Super+E")];
        assert_eq!(
            find_trigger_description(shortcuts, "emoji-nook-toggle"),
            Some("Super+E")
        );
    }

    #[test]
    fn find_trigger_description_returns_none_when_missing() {
        let shortcuts = [("other-id", "Ctrl+X")];
        assert_eq!(
            find_trigger_description(shortcuts, "emoji-nook-toggle"),
            None
        );
    }

    #[test]
    fn translates_single_char_keys_to_lowercase() {
        assert_eq!(to_xdg_trigger("Alt+Shift+E"), "<Alt><Shift>e");
        assert_eq!(to_xdg_trigger("Ctrl+Space"), "<Ctrl>space");
        assert_eq!(to_xdg_trigger("Super+."), "<Super>.");
    }

    #[test]
    fn handles_plus_key_in_shortcut() {
        assert_eq!(to_xdg_trigger("Ctrl++"), "<Ctrl>plus");
        assert_eq!(to_xdg_trigger("Ctrl+Shift++"), "<Ctrl><Shift>plus");
        assert_eq!(to_xdg_trigger("+"), "plus");
    }

    #[test]
    fn preserves_named_key_case() {
        assert_eq!(to_xdg_trigger("Alt+Tab"), "<Alt>Tab");
        assert_eq!(to_xdg_trigger("Ctrl+Return"), "<Ctrl>Return");
        assert_eq!(to_xdg_trigger("Alt+F1"), "<Alt>F1");
        assert_eq!(to_xdg_trigger("Ctrl+Left"), "<Ctrl>Left");
        assert_eq!(to_xdg_trigger("Shift+BackSpace"), "<Shift>BackSpace");
    }
}
