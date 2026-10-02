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

    use crate::{
        error::ServiceError,
        linux::{DESKTOP_DESTINATION, DESKTOP_PATH},
        timeout::with_timeout,
    };

    /// A live inhibitor: the portal's request object, which holds it until closed.
    pub type Inhibitor = OwnedObjectPath;

    /// Takes an inhibitor for as long as the returned request object stays open.
    ///
    /// The portal keeps the inhibitor on the request that `Inhibit` returns, and emits that
    /// request's `Response` only when it ends, so the call is made directly and not through a
    /// wrapper that waits for the response.
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
        with_timeout("Inhibit", proxy.call("Inhibit", &("", flags, &options))).await
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
pub type Inhibitor = ();

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
    fn a_reason_is_required() {
        let request = InhibitRequest {
            reason: " ".into(),
            kinds: vec![],
        };
        assert!(request.validate().is_err());
    }
}
