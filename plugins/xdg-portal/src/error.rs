// Defines plugin error types for portal command failures
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

use thiserror::Error;

#[derive(Debug, Error)]
pub enum PortalError {
    #[error("unsupported platform: only Linux is supported")]
    UnsupportedPlatform,
    #[error("internal error: {0}")]
    Internal(String),
}

impl serde::Serialize for PortalError {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        serializer.serialize_str(&self.to_string())
    }
}

/// Why a portal service command failed, as a value a caller can branch on.
#[derive(Debug, Clone, Copy, serde::Serialize, serde::Deserialize, PartialEq, Eq, ts_rs::TS)]
#[serde(rename_all = "kebab-case")]
#[ts(export, export_to = "../guest-js/bindings/")]
pub enum ServiceErrorKind {
    /// The feature does not exist on this operating system.
    UnsupportedPlatform,
    /// The portal, or the interface the call needs, is not offered on this session.
    PortalUnavailable,
    /// The portal refused the call, for example because the caller is not allowed to make it.
    NotAllowed,
    /// The request itself is malformed; nothing was sent to the portal.
    InvalidArgument,
    /// The handle or notification id does not name anything alive.
    NotFound,
    /// The portal did not answer in time.
    Timeout,
    /// The portal answered with an error.
    Failed,
}

/// The error of every notification, inhibit and open-URI command: a `kind` to branch on and a
/// `message` for logs.
#[derive(
    Debug, Clone, thiserror::Error, serde::Serialize, serde::Deserialize, PartialEq, Eq, ts_rs::TS,
)]
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

    pub fn unsupported() -> Self {
        Self::new(
            ServiceErrorKind::UnsupportedPlatform,
            "xdg-desktop-portal is only available on Linux",
        )
    }

    pub fn timeout(what: &str) -> Self {
        Self::new(
            ServiceErrorKind::Timeout,
            format!("the portal did not answer {what} in time"),
        )
    }

    /// Sorts a D-Bus or portal error message into the kind a caller can act on.
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
    if has("notallowed") || has("not allowed") || has("accessdenied") || has("access denied") {
        ServiceErrorKind::NotAllowed
    } else if has("serviceunknown")
        || has("namehasnoowner")
        || has("unknowninterface")
        || has("unknownobject")
        || has("unknownmethod")
        || has("no such file")
        || has("failed to connect")
        || has("connection refused")
    {
        ServiceErrorKind::PortalUnavailable
    } else if has("timeout") || has("timed out") || has("noreply") {
        ServiceErrorKind::Timeout
    } else if has("notfound") || has("not found") {
        ServiceErrorKind::NotFound
    } else {
        ServiceErrorKind::Failed
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn classifies_dbus_errors() {
        let cases = [
            (
                "org.freedesktop.DBus.Error.ServiceUnknown: not provided",
                ServiceErrorKind::PortalUnavailable,
            ),
            (
                "org.freedesktop.DBus.Error.UnknownInterface",
                ServiceErrorKind::PortalUnavailable,
            ),
            (
                "org.freedesktop.portal.Error.NotAllowed: no",
                ServiceErrorKind::NotAllowed,
            ),
            (
                "org.freedesktop.DBus.Error.NoReply: timed out",
                ServiceErrorKind::Timeout,
            ),
            ("something broke", ServiceErrorKind::Failed),
        ];
        for (message, kind) in cases {
            assert_eq!(classify_message(message), kind, "{message}");
        }
    }

    #[test]
    fn serialises_as_an_object() {
        let json = serde_json::to_value(ServiceError::invalid("bad id")).unwrap();
        assert_eq!(json["kind"], "invalid-argument");
        assert_eq!(json["message"], "bad id");
    }
}
