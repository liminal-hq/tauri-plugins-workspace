// Defines the typed error every desktop-integration service command rejects with
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

use std::{fmt::Display, future::Future, time::Duration};

use serde::{Deserialize, Serialize};
use thiserror::Error;
use ts_rs::TS;

/// How long a call to a desktop service may take before the command fails with a timeout.
pub const CALL_TIMEOUT: Duration = Duration::from_secs(5);

/// Why a service command failed, as a value a caller can branch on.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, TS)]
#[serde(rename_all = "kebab-case")]
#[ts(export, export_to = "../guest-js/bindings/")]
pub enum ServiceErrorKind {
    /// The feature does not exist on this operating system.
    UnsupportedPlatform,
    /// The service the feature talks to is not running or not reachable.
    Unavailable,
    /// Windows toasts need an AppUserModelID and the process has none.
    NeedsAppId,
    /// The request itself is malformed; nothing was sent.
    InvalidArgument,
    /// The handle, id or shortcut does not name anything alive.
    NotFound,
    /// Another process already holds the name or shortcut.
    Conflict,
    /// The service did not answer in time.
    Timeout,
    /// The service answered with an error.
    Failed,
}

/// The error of every service command: a `kind` to branch on and a `message` for logs.
#[derive(Debug, Clone, Error, Serialize, Deserialize, PartialEq, Eq, TS)]
#[error("{message}")]
#[ts(export, export_to = "../guest-js/bindings/")]
pub struct ServiceError {
    pub kind: ServiceErrorKind,
    pub message: String,
}

impl ServiceError {
    pub fn new(kind: ServiceErrorKind, message: impl Into<String>) -> Self {
        Self {
            kind,
            message: message.into(),
        }
    }

    pub fn invalid(message: impl Into<String>) -> Self {
        Self::new(ServiceErrorKind::InvalidArgument, message)
    }

    pub fn not_found(message: impl Into<String>) -> Self {
        Self::new(ServiceErrorKind::NotFound, message)
    }

    pub fn unsupported(what: &str) -> Self {
        Self::new(
            ServiceErrorKind::UnsupportedPlatform,
            format!("{what} is not available on this operating system"),
        )
    }

    pub fn timeout(what: &str) -> Self {
        Self::new(
            ServiceErrorKind::Timeout,
            format!("{what} did not answer in time"),
        )
    }

    /// Sorts a D-Bus or system error message into the kind a caller can act on.
    pub fn from_message(message: impl Into<String>) -> Self {
        let message = message.into();
        let kind = classify_message(&message);
        Self { kind, message }
    }
}

/// Maps the text of a D-Bus error onto a [`ServiceErrorKind`].
pub fn classify_message(message: &str) -> ServiceErrorKind {
    let lower = message.to_ascii_lowercase();
    let has = |needle: &str| lower.contains(needle);
    if has("serviceunknown")
        || has("namehasnoowner")
        || has("unknownobject")
        || has("unknowninterface")
        || has("unknownmethod")
        || has("no such file")
        || has("failed to connect")
        || has("connection refused")
    {
        ServiceErrorKind::Unavailable
    } else if has("timeout") || has("timed out") || has("noreply") {
        ServiceErrorKind::Timeout
    } else if has("already owned")
        || has("already taken")
        || has("name is taken")
        || has("alreadyexists")
    {
        ServiceErrorKind::Conflict
    } else {
        ServiceErrorKind::Failed
    }
}

/// Awaits `future` for at most [`CALL_TIMEOUT`], mapping its error onto a [`ServiceError`].
pub async fn with_timeout<T, E: Display>(
    what: &str,
    future: impl Future<Output = Result<T, E>>,
) -> Result<T, ServiceError> {
    match tokio::time::timeout(CALL_TIMEOUT, future).await {
        Ok(Ok(value)) => Ok(value),
        Ok(Err(error)) => Err(ServiceError::from_message(format!("{what}: {error}"))),
        Err(_) => Err(ServiceError::timeout(what)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn classifies_dbus_errors() {
        let cases = [
            (
                "org.freedesktop.DBus.Error.ServiceUnknown",
                ServiceErrorKind::Unavailable,
            ),
            (
                "org.freedesktop.DBus.Error.NoReply: timed out",
                ServiceErrorKind::Timeout,
            ),
            ("name is taken", ServiceErrorKind::Conflict),
            (
                "RequestName: name already taken on the bus",
                ServiceErrorKind::Conflict,
            ),
            ("boom", ServiceErrorKind::Failed),
        ];
        for (message, kind) in cases {
            assert_eq!(classify_message(message), kind, "{message}");
        }
    }

    #[test]
    fn serialises_as_an_object() {
        let json = serde_json::to_value(ServiceError::not_found("no such handle")).unwrap();
        assert_eq!(json["kind"], "not-found");
        assert_eq!(json["message"], "no such handle");
    }

    #[tokio::test]
    async fn with_timeout_passes_values_and_classifies_errors() {
        assert_eq!(
            with_timeout("a call", async { Ok::<_, String>(1) }).await,
            Ok(1)
        );
        let error = with_timeout("a call", async {
            Err::<(), _>("org.freedesktop.DBus.Error.ServiceUnknown".to_string())
        })
        .await
        .unwrap_err();
        assert_eq!(error.kind, ServiceErrorKind::Unavailable);
    }

    #[tokio::test(start_paused = true)]
    async fn with_timeout_gives_up_on_a_call_that_never_answers() {
        let error = with_timeout("a call", std::future::pending::<Result<(), String>>())
            .await
            .unwrap_err();
        assert_eq!(error.kind, ServiceErrorKind::Timeout);
    }
}
