// Validates notification requests and maps them onto the Notification portal
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

use crate::{
    error::ServiceError,
    models::{ActionButton, NotificationAction, NotificationRequest, Urgency},
};

/// The most action buttons a notification shows; further ones are dropped.
pub const MAX_ACTIONS: usize = 3;

/// The longest action id, in bytes.
pub const MAX_ACTION_ID_LEN: usize = 256;

/// The longest button label, in characters.
pub const MAX_ACTION_LABEL_CHARS: usize = 100;

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
        validate_actions(self.actions())?;
        Ok(())
    }

    /// The buttons that are shown: the first [`MAX_ACTIONS`], in order.
    pub fn actions(&self) -> &[ActionButton] {
        limit_actions(&self.id, self.actions.as_deref().unwrap_or_default())
    }
}

/// Keeps the first [`MAX_ACTIONS`] buttons and logs the ones dropped.
pub fn limit_actions<'a>(id: &str, actions: &'a [ActionButton]) -> &'a [ActionButton] {
    if actions.len() > MAX_ACTIONS {
        log::info!(
            "notification {id:?} has {} actions; showing the first {MAX_ACTIONS}",
            actions.len()
        );
    }
    &actions[..actions.len().min(MAX_ACTIONS)]
}

/// Checks the buttons: ids and labels are non-empty and bounded, and ids are unique so a press
/// is unambiguous.
pub fn validate_actions(actions: &[ActionButton]) -> Result<(), ServiceError> {
    for (index, action) in actions.iter().enumerate() {
        if action.id.is_empty() || action.id.len() > MAX_ACTION_ID_LEN {
            return Err(ServiceError::invalid(format!(
                "an action id is 1 to {MAX_ACTION_ID_LEN} bytes"
            )));
        }
        if action.label.trim().is_empty() || action.label.chars().count() > MAX_ACTION_LABEL_CHARS {
            return Err(ServiceError::invalid(format!(
                "an action label is 1 to {MAX_ACTION_LABEL_CHARS} characters"
            )));
        }
        if actions[..index].iter().any(|other| other.id == action.id) {
            return Err(ServiceError::invalid(format!(
                "the action id {:?} is used twice",
                action.id
            )));
        }
    }
    Ok(())
}

/// The portal's `buttons` as `(label, action)` pairs, in display order.
pub fn button_pairs(request: &NotificationRequest) -> Vec<(&str, &str)> {
    request
        .actions()
        .iter()
        .map(|action| (action.label.as_str(), action.id.as_str()))
        .collect()
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
    use ashpd::desktop::notification::{Button, Notification, NotificationProxy, Priority};
    use futures_util::StreamExt;
    use tauri::{AppHandle, Emitter, Runtime};

    use super::{action_event, button_pairs, ACTION_EVENT};
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
        let notification = Notification::new(&request.title)
            .body(request.body.as_deref())
            .priority(priority)
            .default_action(request.default_action.as_deref());
        button_pairs(request)
            .into_iter()
            .fold(notification, |notification, (label, action)| {
                notification.button(Button::new(label, action))
            })
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
            actions: None,
        }
    }

    fn button(id: &str, label: &str) -> ActionButton {
        ActionButton {
            id: id.into(),
            label: label.into(),
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

    #[test]
    fn keeps_the_first_three_actions() {
        let mut r = request();
        r.actions = Some(
            ["a", "b", "c", "d", "e"]
                .iter()
                .map(|id| button(id, id))
                .collect(),
        );
        assert_eq!(r.validate(), Ok(()));
        let ids: Vec<_> = r.actions().iter().map(|a| a.id.as_str()).collect();
        assert_eq!(ids, ["a", "b", "c"]);
        assert_eq!(
            button_pairs(&r),
            [("a", "a"), ("b", "b"), ("c", "c")],
            "label first, then the action id"
        );
        assert!(request().actions().is_empty());
    }

    #[test]
    fn rejects_bad_actions() {
        let bad = [
            button("", "Label"),
            button("id", ""),
            button("id", "  "),
            button(&"x".repeat(MAX_ACTION_ID_LEN + 1), "Label"),
            button("id", &"x".repeat(MAX_ACTION_LABEL_CHARS + 1)),
        ];
        for action in bad {
            let mut r = request();
            r.actions = Some(vec![action.clone()]);
            assert_eq!(
                r.validate().unwrap_err().kind,
                ServiceErrorKind::InvalidArgument,
                "{action:?}"
            );
        }
        let mut r = request();
        r.actions = Some(vec![button("same", "One"), button("same", "Two")]);
        assert_eq!(
            r.validate().unwrap_err().kind,
            ServiceErrorKind::InvalidArgument
        );

        // The longest label and id are fine, counted in characters for the label.
        let mut r = request();
        r.actions = Some(vec![button(
            &"i".repeat(MAX_ACTION_ID_LEN),
            &"é".repeat(MAX_ACTION_LABEL_CHARS),
        )]);
        assert_eq!(r.validate(), Ok(()));
    }

    #[test]
    fn a_dropped_extra_action_is_not_validated() {
        let mut r = request();
        r.actions = Some(vec![
            button("a", "A"),
            button("b", "B"),
            button("c", "C"),
            button("", ""),
        ]);
        assert_eq!(r.validate(), Ok(()));
    }

    #[test]
    fn a_pressed_button_reports_the_notifications_own_id() {
        let payload = action_event("copy-1", "undo");
        let json = serde_json::to_value(&payload).unwrap();
        assert_eq!(
            json,
            serde_json::json!({ "id": "copy-1", "action": "undo" })
        );
    }

    #[test]
    fn the_request_accepts_actions_from_json() {
        let r: NotificationRequest = serde_json::from_value(serde_json::json!({
            "id": "n", "title": "t", "body": null, "defaultAction": null, "urgency": null,
            "actions": [{ "id": "undo", "label": "Undo" }]
        }))
        .unwrap();
        assert_eq!(r.actions.unwrap(), vec![button("undo", "Undo")]);
        let r: NotificationRequest = serde_json::from_value(serde_json::json!({
            "id": "n", "title": "t"
        }))
        .unwrap();
        assert_eq!(r.actions, None);
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn the_portal_message_carries_the_buttons() {
        use ashpd::zvariant::{serialized::Context, to_bytes, LE};

        let mut r = request();
        r.actions = Some(vec![button("undo", "Undo"), button("show", "Show")]);
        let bytes = to_bytes(Context::new_dbus(LE, 0), &build_notification(&r)).unwrap();
        let text = String::from_utf8_lossy(&bytes);
        for needle in ["buttons", "Undo", "undo", "Show", "show", "label", "action"] {
            assert!(text.contains(needle), "{needle} missing from {text:?}");
        }
        assert!(
            text.find("Undo").unwrap() < text.find("Show").unwrap(),
            "buttons keep their order"
        );

        let without = to_bytes(Context::new_dbus(LE, 0), &build_notification(&request())).unwrap();
        assert!(!String::from_utf8_lossy(&without).contains("buttons"));
    }
}
