// Validates notification requests and maps them onto the Notification portal
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

use crate::{
    error::ServiceError,
    models::{NotificationAction, NotificationRequest, Urgency},
};

/// The longest notification id the plugin accepts.
pub const MAX_ID_LEN: usize = 256;

/// The event emitted when the user acts on a notification.
pub const ACTION_EVENT: &str = "xdg-portal://notification-action";

impl NotificationRequest {
    /// Checks the request before anything is sent to the portal.
    pub fn validate(&self) -> Result<(), ServiceError> {
        validate_id(&self.id)?;
        if self.title.trim().is_empty() {
            return Err(ServiceError::invalid("a notification needs a title"));
        }
        if let Some(action) = &self.default_action {
            if action.is_empty() {
                return Err(ServiceError::invalid(
                    "the default action id must not be empty",
                ));
            }
        }
        Ok(())
    }
}

/// Checks a notification id: the portal accepts any string, but an empty or huge one is a bug.
pub fn validate_id(id: &str) -> Result<(), ServiceError> {
    if id.is_empty() {
        return Err(ServiceError::invalid("a notification needs an id"));
    }
    if id.len() > MAX_ID_LEN {
        return Err(ServiceError::invalid(format!(
            "a notification id is at most {MAX_ID_LEN} bytes"
        )));
    }
    Ok(())
}

/// The portal's name for an urgency: the value of the `priority` option.
pub fn priority_name(urgency: Urgency) -> &'static str {
    match urgency {
        Urgency::Low => "low",
        Urgency::Normal => "normal",
        Urgency::High => "high",
        Urgency::Urgent => "urgent",
    }
}

/// The event payload for an `ActionInvoked` signal.
pub fn action_event(id: &str, action: &str) -> NotificationAction {
    NotificationAction {
        id: id.to_string(),
        action: action.to_string(),
    }
}

#[cfg(target_os = "linux")]
pub use linux::{build_notification, listen_for_actions};

#[cfg(target_os = "linux")]
mod linux {
    use ashpd::desktop::notification::{Notification, NotificationProxy, Priority};
    use futures_util::StreamExt;
    use tauri::{AppHandle, Emitter, Runtime};

    use super::{action_event, ACTION_EVENT};
    use crate::{
        error::ServiceError,
        models::{NotificationRequest, Urgency},
        timeout::with_timeout,
    };

    /// Builds the portal's notification for a validated request.
    pub fn build_notification(request: &NotificationRequest) -> Notification {
        let priority = match request.urgency.unwrap_or_default() {
            Urgency::Low => Priority::Low,
            Urgency::Normal => Priority::Normal,
            Urgency::High => Priority::High,
            Urgency::Urgent => Priority::Urgent,
        };
        Notification::new(&request.title)
            .body(request.body.as_deref())
            .priority(priority)
            .default_action(request.default_action.as_deref())
    }

    /// Starts forwarding `ActionInvoked` signals to the `action` event; resolves once the signal
    /// subscription exists, so a notification sent afterwards cannot be missed.
    pub async fn listen_for_actions<R: Runtime>(
        app: AppHandle<R>,
        connection: ashpd::zbus::Connection,
    ) -> Result<tauri::async_runtime::JoinHandle<()>, ServiceError> {
        let (ready_tx, ready_rx) = tokio::sync::oneshot::channel();
        let handle = tauri::async_runtime::spawn(async move {
            // The stream borrows the proxy, so both live in this task.
            let proxy = match with_timeout(
                "the notification portal",
                NotificationProxy::with_connection(connection),
            )
            .await
            {
                Ok(proxy) => proxy,
                Err(error) => {
                    let _ = ready_tx.send(Err(error));
                    return;
                }
            };
            let stream =
                match with_timeout("the action subscription", proxy.receive_action_invoked()).await
                {
                    Ok(stream) => stream,
                    Err(error) => {
                        let _ = ready_tx.send(Err(error));
                        return;
                    }
                };
            let _ = ready_tx.send(Ok(()));
            let mut stream = std::pin::pin!(stream);
            while let Some(action) = stream.next().await {
                let payload = action_event(action.id(), action.name());
                if let Err(error) = app.emit(ACTION_EVENT, payload) {
                    log::warn!("could not emit a notification action: {error}");
                }
            }
        });
        match ready_rx.await {
            Ok(Ok(())) => Ok(handle),
            Ok(Err(error)) => Err(error),
            Err(_) => Err(ServiceError::from_message("the action listener stopped")),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::error::ServiceErrorKind;

    fn request() -> NotificationRequest {
        NotificationRequest {
            id: "copy-1".into(),
            title: "Copy finished".into(),
            body: Some("3 files".into()),
            default_action: Some("show-job".into()),
            urgency: None,
        }
    }

    #[test]
    fn a_complete_request_is_valid() {
        assert_eq!(request().validate(), Ok(()));
    }

    #[test]
    fn rejects_a_missing_id_title_or_action() {
        let mut r = request();
        r.id.clear();
        assert_eq!(
            r.validate().unwrap_err().kind,
            ServiceErrorKind::InvalidArgument
        );

        let mut r = request();
        r.title = "  ".into();
        assert_eq!(
            r.validate().unwrap_err().kind,
            ServiceErrorKind::InvalidArgument
        );

        let mut r = request();
        r.default_action = Some(String::new());
        assert_eq!(
            r.validate().unwrap_err().kind,
            ServiceErrorKind::InvalidArgument
        );

        let mut r = request();
        r.id = "x".repeat(MAX_ID_LEN + 1);
        assert_eq!(
            r.validate().unwrap_err().kind,
            ServiceErrorKind::InvalidArgument
        );
    }

    #[test]
    fn urgency_names_match_the_portal() {
        assert_eq!(priority_name(Urgency::Low), "low");
        assert_eq!(priority_name(Urgency::Normal), "normal");
        assert_eq!(priority_name(Urgency::High), "high");
        assert_eq!(priority_name(Urgency::Urgent), "urgent");
    }

    #[test]
    fn the_action_event_carries_both_ids() {
        let payload = action_event("copy-1", "show-job");
        let json = serde_json::to_value(&payload).unwrap();
        assert_eq!(json["id"], "copy-1");
        assert_eq!(json["action"], "show-job");
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn the_portal_message_carries_the_request_fields() {
        use ashpd::zvariant::{serialized::Context, to_bytes, LE};

        let mut r = request();
        r.urgency = Some(Urgency::High);
        let bytes = to_bytes(Context::new_dbus(LE, 0), &build_notification(&r)).unwrap();
        let text = String::from_utf8_lossy(&bytes);
        for needle in ["Copy finished", "3 files", "show-job", "high"] {
            assert!(text.contains(needle), "{needle} missing from {text:?}");
        }
    }
}
