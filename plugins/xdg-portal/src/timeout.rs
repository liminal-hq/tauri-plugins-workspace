// Bounds every portal call so a stuck portal cannot stall a command
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

use std::{fmt::Display, future::Future, time::Duration};

use crate::error::ServiceError;

/// How long a portal call may take before the command fails with a timeout.
pub const CALL_TIMEOUT: Duration = Duration::from_secs(5);

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
    use crate::error::ServiceErrorKind;

    #[tokio::test]
    async fn passes_a_value_through() {
        let result = with_timeout("a call", async { Ok::<_, String>(7) }).await;
        assert_eq!(result, Ok(7));
    }

    #[tokio::test]
    async fn classifies_an_error() {
        let result = with_timeout("a call", async {
            Err::<(), _>("org.freedesktop.DBus.Error.ServiceUnknown".to_string())
        })
        .await;
        assert_eq!(
            result.unwrap_err().kind,
            ServiceErrorKind::PortalUnavailable
        );
    }

    #[tokio::test(start_paused = true)]
    async fn gives_up_on_a_call_that_never_answers() {
        let result = with_timeout("a call", std::future::pending::<Result<(), String>>()).await;
        assert_eq!(result.unwrap_err().kind, ServiceErrorKind::Timeout);
    }
}
