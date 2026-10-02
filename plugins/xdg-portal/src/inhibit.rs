// Maps inhibit requests onto the Inhibit portal's flags and request objects
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

use crate::{
    error::ServiceError,
    models::{InhibitKind, InhibitRequest},
};

/// The portal's flag for keeping the machine from suspending.
pub const FLAG_SUSPEND: u32 = 4;
/// The portal's flag for keeping the session from going idle.
pub const FLAG_IDLE: u32 = 8;

/// The reason shown when a caller gives none.
pub const DEFAULT_REASON: &str = "Waypoint is busy";

/// The portal flags for a request; no kinds means idle and suspend both.
pub fn flags_for(kinds: &[InhibitKind]) -> u32 {
    if kinds.is_empty() {
        return FLAG_IDLE | FLAG_SUSPEND;
    }
    kinds.iter().fold(0, |flags, kind| {
        flags
            | match kind {
                InhibitKind::Idle => FLAG_IDLE,
                InhibitKind::Suspend => FLAG_SUSPEND,
            }
    })
}

/// The `handle_token` for the `n`th request of this process: a valid object path element that no
/// other request of the process shares.
pub fn handle_token(n: u64) -> String {
    format!("tauri_plugin_xdg_portal_{}_{n}", std::process::id())
}

/// The object path the portal gives the request made with `handle_token` by the bus connection
/// `unique_name` (`:1.42` becomes `1_42`), known before the portal answers.
pub fn request_path(unique_name: &str, token: &str) -> String {
    let sender = unique_name.trim_start_matches(':').replace('.', "_");
    format!("/org/freedesktop/portal/desktop/request/{sender}/{token}")
}

impl InhibitRequest {
    pub fn validate(&self) -> Result<(), ServiceError> {
        if self.reason.trim().is_empty() {
            return Err(ServiceError::invalid("an inhibitor needs a reason"));
        }
        Ok(())
    }
}

#[cfg(target_os = "linux")]
pub use linux::{acquire, release, Inhibitor};

#[cfg(target_os = "linux")]
mod linux {
    use std::collections::HashMap;

    use ashpd::zbus::{
        zvariant::{OwnedObjectPath, Value},
        Connection, Proxy,
    };

    use std::sync::atomic::{AtomicU64, Ordering};

    use super::{handle_token, request_path};
    use crate::{
        error::{ServiceError, ServiceErrorKind},
        linux::{DESKTOP_DESTINATION, DESKTOP_PATH},
        timeout::with_timeout,
    };

    static REQUESTS: AtomicU64 = AtomicU64::new(0);

    /// A live inhibitor: the portal's request object, which holds it until closed.
    pub type Inhibitor = OwnedObjectPath;

    /// Takes an inhibitor for as long as the returned request object stays open.
    ///
    /// The portal keeps the inhibitor on the request that `Inhibit` returns, and emits that
    /// request's `Response` only when it ends, so the call is made directly and not through a
    /// wrapper that waits for the response.
    ///
    /// The request is made with a `handle_token`, which fixes its object path in advance. If the
    /// call times out on this side the portal may still have created the request, so that path is
    /// closed on a best-effort basis; the portal can, rarely, create the request after that
    /// `Close` has already failed, and the inhibitor then lasts until the app exits.
    pub async fn acquire(
        connection: &Connection,
        flags: u32,
        reason: &str,
    ) -> Result<Inhibitor, ServiceError> {
        let proxy = with_timeout(
            "the Inhibit portal",
            Proxy::new(
                connection,
                DESKTOP_DESTINATION,
                DESKTOP_PATH,
                "org.freedesktop.portal.Inhibit",
            ),
        )
        .await?;
        let mut options: HashMap<&str, Value<'_>> = HashMap::new();
        options.insert("reason", Value::from(reason));
        let token = handle_token(REQUESTS.fetch_add(1, Ordering::Relaxed));
        options.insert("handle_token", Value::from(token.as_str()));
        let acquired = with_timeout("Inhibit", proxy.call("Inhibit", &("", flags, &options))).await;
        if matches!(&acquired, Err(error) if error.kind == ServiceErrorKind::Timeout) {
            if let Some(name) = connection.unique_name() {
                let path = request_path(name.as_str(), &token);
                if let Ok(path) = OwnedObjectPath::try_from(path) {
                    // Nothing to release if the portal never created the request.
                    let _ = release(connection, &path).await;
                }
            }
        }
        acquired
    }

    /// Ends an inhibitor by closing its request object.
    pub async fn release(
        connection: &Connection,
        inhibitor: &Inhibitor,
    ) -> Result<(), ServiceError> {
        let request = with_timeout(
            "the request object",
            Proxy::new(
                connection,
                DESKTOP_DESTINATION,
                inhibitor.as_ref(),
                "org.freedesktop.portal.Request",
            ),
        )
        .await?;
        with_timeout("Close", request.call::<_, _, ()>("Close", &())).await
    }
}

/// Non-Linux builds never hold an inhibitor; the type only keeps the shared code compiling.
#[cfg(not(target_os = "linux"))]
pub type Inhibitor = std::convert::Infallible;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn no_kinds_means_both() {
        assert_eq!(flags_for(&[]), 12);
    }

    #[test]
    fn kinds_combine_into_flags() {
        assert_eq!(flags_for(&[InhibitKind::Idle]), FLAG_IDLE);
        assert_eq!(flags_for(&[InhibitKind::Suspend]), FLAG_SUSPEND);
        assert_eq!(
            flags_for(&[InhibitKind::Suspend, InhibitKind::Idle, InhibitKind::Idle]),
            12
        );
    }

    #[test]
    fn the_request_path_follows_the_portal_naming() {
        assert_eq!(
            request_path(":1.42", "token_1"),
            "/org/freedesktop/portal/desktop/request/1_42/token_1"
        );
    }

    #[test]
    fn handle_tokens_are_unique_and_valid_path_elements() {
        let (a, b) = (handle_token(0), handle_token(1));
        assert_ne!(a, b);
        assert!(a.chars().all(|c| c.is_ascii_alphanumeric() || c == '_'));
    }

    #[test]
    fn a_reason_is_required() {
        let request = InhibitRequest {
            reason: " ".into(),
            kinds: vec![],
        };
        assert!(request.validate().is_err());
    }
}
