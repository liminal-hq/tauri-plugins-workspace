// Maps sleep-inhibit requests onto logind's `what` string and checks them
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

use crate::{
    error::ServiceError,
    models::{SleepInhibitRequest, SleepKind},
};

impl SleepInhibitRequest {
    pub fn validate(&self) -> Result<(), ServiceError> {
        if self.reason.trim().is_empty() {
            return Err(ServiceError::invalid("a sleep inhibitor needs a reason"));
        }
        Ok(())
    }
}

/// logind's `what` argument for the kinds asked for; no kinds means sleep only.
pub fn logind_what(kinds: &[SleepKind]) -> &'static str {
    let sleep = kinds.is_empty() || kinds.contains(&SleepKind::Sleep);
    let idle = kinds.contains(&SleepKind::Idle);
    match (sleep, idle) {
        (true, true) => "sleep:idle",
        (true, false) => "sleep",
        (false, _) => "idle",
    }
}

/// What a live sleep inhibitor holds: logind's file descriptor, or a Windows power request.
#[cfg(target_os = "linux")]
pub type Inhibitor = zbus::zvariant::OwnedFd;
#[cfg(target_os = "windows")]
pub type Inhibitor = crate::win::power::PowerRequest;
#[cfg(not(any(target_os = "linux", target_os = "windows")))]
pub type Inhibitor = std::convert::Infallible;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn maps_kinds_to_logind() {
        assert_eq!(logind_what(&[]), "sleep");
        assert_eq!(logind_what(&[SleepKind::Sleep]), "sleep");
        assert_eq!(logind_what(&[SleepKind::Idle]), "idle");
        assert_eq!(
            logind_what(&[SleepKind::Idle, SleepKind::Sleep]),
            "sleep:idle"
        );
    }

    #[test]
    fn a_reason_is_required() {
        let request = SleepInhibitRequest {
            reason: String::new(),
            kinds: vec![],
        };
        assert!(request.validate().is_err());
    }
}
