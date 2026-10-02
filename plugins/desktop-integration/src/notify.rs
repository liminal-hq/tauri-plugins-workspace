// Maps notification requests onto the freedesktop Notifications call and Windows toasts
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

use std::collections::HashMap;

use crate::{
    error::ServiceError,
    models::{NotificationAction, NotifyRequest, Urgency},
};

/// The event emitted when the user clicks a notification.
pub const ACTION_EVENT: &str = "desktop-integration://notification-action";

/// The longest notification id the plugin accepts.
pub const MAX_ID_LEN: usize = 256;

/// The action key a notification server reports when the notification itself is clicked.
pub const DEFAULT_ACTION_KEY: &str = "default";

impl NotifyRequest {
    /// Checks the request before anything is sent.
    pub fn validate(&self) -> Result<(), ServiceError> {
        validate_id(&self.id)?;
        if self.title.trim().is_empty() {
            return Err(ServiceError::invalid("a notification needs a title"));
        }
        if self.default_action.as_deref() == Some("") {
            return Err(ServiceError::invalid(
                "the default action id must not be empty",
            ));
        }
        Ok(())
    }
}

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

/// The arguments of `org.freedesktop.Notifications.Notify`, apart from the hints' D-Bus types.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NotifyCall {
    pub app_name: String,
    /// The server's id of the notification to replace; 0 shows a new one.
    pub replaces_id: u32,
    pub summary: String,
    pub body: String,
    /// Alternating action keys and labels.
    pub actions: Vec<String>,
    /// The `urgency` hint: 0 low, 1 normal, 2 critical.
    pub urgency: u8,
    /// The `desktop-entry` hint, without the `.desktop` suffix.
    pub desktop_entry: String,
    /// The expiry in milliseconds; -1 leaves it to the server.
    pub expire_timeout: i32,
}

/// Builds the `Notify` arguments for a validated request.
pub fn build_call(
    request: &NotifyRequest,
    app_name: &str,
    desktop_id: &str,
    replaces_id: u32,
) -> NotifyCall {
    let actions = match request.default_action {
        Some(_) => vec![DEFAULT_ACTION_KEY.to_string(), "Open".to_string()],
        None => Vec::new(),
    };
    NotifyCall {
        app_name: request
            .app_name
            .clone()
            .unwrap_or_else(|| app_name.to_string()),
        replaces_id,
        summary: request.title.clone(),
        body: request.body.clone().unwrap_or_default(),
        actions,
        urgency: match request.urgency.unwrap_or_default() {
            Urgency::Low => 0,
            Urgency::Normal => 1,
            Urgency::Critical => 2,
        },
        desktop_entry: crate::launcher::desktop_id_of(
            request.desktop_id.as_deref().unwrap_or(desktop_id),
        ),
        expire_timeout: -1,
    }
}

#[derive(Debug, Clone)]
struct Shown {
    id: String,
    default_action: Option<String>,
}

/// Pairs the caller's string ids with the numeric ids a notification server hands out, so a
/// repeated id replaces the shown notification and a server signal finds the caller's id again.
#[derive(Debug, Default)]
pub struct IdBook {
    by_id: HashMap<String, u32>,
    by_server: HashMap<u32, Shown>,
}

impl IdBook {
    /// The server id currently showing `id`, to pass as `replaces_id`.
    pub fn server_id(&self, id: &str) -> Option<u32> {
        self.by_id.get(id).copied()
    }

    /// Records that the server shows `id` as `server_id`.
    pub fn record(&mut self, id: &str, server_id: u32, default_action: Option<&str>) {
        if let Some(previous) = self.by_id.insert(id.to_string(), server_id) {
            if previous != server_id {
                self.by_server.remove(&previous);
            }
        }
        self.by_server.insert(
            server_id,
            Shown {
                id: id.to_string(),
                default_action: default_action.map(str::to_string),
            },
        );
    }

    /// Forgets `id`, returning the server id it was shown as.
    pub fn forget(&mut self, id: &str) -> Option<u32> {
        let server_id = self.by_id.remove(id)?;
        self.by_server.remove(&server_id);
        Some(server_id)
    }

    /// Forgets a notification the server closed.
    pub fn forget_server(&mut self, server_id: u32) {
        if let Some(shown) = self.by_server.remove(&server_id) {
            self.by_id.remove(&shown.id);
        }
    }

    /// The event for an `ActionInvoked(server_id, key)` signal; `None` for an unknown
    /// notification or for the default action of a notification that did not ask for one.
    pub fn action_for(&self, server_id: u32, key: &str) -> Option<NotificationAction> {
        let shown = self.by_server.get(&server_id)?;
        let action = if key == DEFAULT_ACTION_KEY {
            shown.default_action.clone()?
        } else {
            key.to_string()
        };
        Some(NotificationAction {
            id: shown.id.clone(),
            action,
        })
    }
}

/// The toast tag for a notification id: Windows limits tags to 64 characters on some builds, so a
/// longer id is replaced by a stable hash of itself.
pub fn toast_tag(id: &str) -> String {
    if id.chars().count() <= 64 {
        id.to_string()
    } else {
        format!("h{:016x}", fnv1a_64(id.as_bytes()))
    }
}

fn fnv1a_64(bytes: &[u8]) -> u64 {
    bytes.iter().fold(0xcbf2_9ce4_8422_2325, |hash, byte| {
        (hash ^ u64::from(*byte)).wrapping_mul(0x0100_0000_01b3)
    })
}

/// Escapes text for an XML element or attribute value.
fn xml_escape(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    for c in text.chars() {
        match c {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            '\'' => out.push_str("&apos;"),
            c if (c as u32) < 0x20 && !matches!(c, '\n' | '\r' | '\t') => {}
            c => out.push(c),
        }
    }
    out
}

/// Percent-encodes everything but ASCII letters, digits and `-_.~`, so the result is safe in an
/// XML attribute and never contains the `launch` separator, whatever the text holds.
fn launch_encode(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    for byte in text.bytes() {
        if byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_' | b'.' | b'~') {
            out.push(char::from(byte));
        } else {
            out.push_str(&format!("%{byte:02X}"));
        }
    }
    out
}

/// Reverses [`launch_encode`]; `None` for malformed input.
fn launch_decode(text: &str) -> Option<String> {
    let bytes = text.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'%' {
            let hex = text.get(i + 1..i + 3)?;
            out.push(u8::from_str_radix(hex, 16).ok()?);
            i += 3;
        } else {
            out.push(bytes[i]);
            i += 1;
        }
    }
    String::from_utf8(out).ok()
}

/// The toast XML for a request. The `launch` attribute carries the id and default action so the
/// activation handler can report them; both are percent-encoded so that any id, control
/// characters and quotes included, comes back exactly as the app sent it.
pub fn toast_xml(request: &NotifyRequest) -> String {
    let launch = format!(
        "{}\u{1f}{}",
        launch_encode(&request.id),
        launch_encode(request.default_action.as_deref().unwrap_or(""))
    );
    let body = request
        .body
        .as_deref()
        .filter(|b| !b.is_empty())
        .map(|b| format!("<text>{}</text>", xml_escape(b)))
        .unwrap_or_default();
    format!(
        "<toast launch=\"{launch}\" activationType=\"foreground\"><visual><binding template=\"ToastGeneric\"><text>{}</text>{body}</binding></visual></toast>",
        xml_escape(&request.title)
    )
}

/// Splits the `launch` string of [`toast_xml`] back into the id and default action.
pub fn parse_toast_launch(launch: &str) -> Option<NotificationAction> {
    let (id, action) = launch.split_once('\u{1f}')?;
    if id.is_empty() || action.is_empty() {
        return None;
    }
    Some(NotificationAction {
        id: launch_decode(id)?,
        action: launch_decode(action)?,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::error::ServiceErrorKind;

    fn request() -> NotifyRequest {
        NotifyRequest {
            id: "copy-1".into(),
            title: "Copy finished".into(),
            body: Some("3 files".into()),
            default_action: Some("show-job".into()),
            urgency: None,
            app_name: None,
            desktop_id: None,
        }
    }

    #[test]
    fn validates_the_request() {
        assert_eq!(request().validate(), Ok(()));
        for mutate in [
            (|r: &mut NotifyRequest| r.id.clear()) as fn(&mut NotifyRequest),
            |r| r.title = " ".into(),
            |r| r.default_action = Some(String::new()),
            |r| r.id = "x".repeat(MAX_ID_LEN + 1),
        ] {
            let mut r = request();
            mutate(&mut r);
            assert_eq!(
                r.validate().unwrap_err().kind,
                ServiceErrorKind::InvalidArgument
            );
        }
    }

    #[test]
    fn builds_the_notify_arguments() {
        let call = build_call(&request(), "Waypoint", "ca.liminalhq.waypoint.desktop", 7);
        assert_eq!(call.app_name, "Waypoint");
        assert_eq!(call.replaces_id, 7);
        assert_eq!(call.summary, "Copy finished");
        assert_eq!(call.body, "3 files");
        assert_eq!(call.actions, vec!["default", "Open"]);
        assert_eq!(call.urgency, 1);
        assert_eq!(call.desktop_entry, "ca.liminalhq.waypoint");

        let mut r = request();
        r.default_action = None;
        r.urgency = Some(Urgency::Critical);
        r.app_name = Some("Other".into());
        let call = build_call(&r, "Waypoint", "x", 0);
        assert!(call.actions.is_empty());
        assert_eq!(call.urgency, 2);
        assert_eq!(call.app_name, "Other");
    }

    #[test]
    fn a_repeated_id_replaces_the_server_notification() {
        let mut book = IdBook::default();
        assert_eq!(book.server_id("a"), None);
        book.record("a", 10, Some("open"));
        assert_eq!(book.server_id("a"), Some(10));
        book.record("a", 11, Some("open"));
        assert_eq!(book.action_for(10, "default"), None);
        assert_eq!(
            book.action_for(11, "default"),
            Some(NotificationAction {
                id: "a".into(),
                action: "open".into()
            })
        );
    }

    #[test]
    fn routes_server_actions_back_to_the_callers_id() {
        let mut book = IdBook::default();
        book.record("a", 10, None);
        book.record("b", 11, Some("show"));
        assert_eq!(
            book.action_for(10, "default"),
            None,
            "no default action asked for"
        );
        assert_eq!(book.action_for(10, "snooze").unwrap().action, "snooze");
        assert_eq!(book.action_for(99, "default"), None);

        book.forget_server(11);
        assert_eq!(book.server_id("b"), None);
        assert_eq!(book.forget("a"), Some(10));
        assert_eq!(book.forget("a"), None);
    }

    #[test]
    fn long_ids_become_a_stable_tag() {
        assert_eq!(toast_tag("short"), "short");
        let long = "x".repeat(100);
        let tag = toast_tag(&long);
        assert_eq!(tag.len(), 17);
        assert_eq!(tag, toast_tag(&long));
        assert_ne!(tag, toast_tag(&"y".repeat(100)));
    }

    #[test]
    fn toast_xml_escapes_and_round_trips_the_launch_string() {
        let mut r = request();
        r.title = "Fish & <chips>".into();
        r.body = Some("\"quoted\" 'text'".into());
        let xml = toast_xml(&r);
        assert!(
            xml.contains("<text>Fish &amp; &lt;chips&gt;</text>"),
            "{xml}"
        );
        assert!(
            xml.contains("<text>&quot;quoted&quot; &apos;text&apos;</text>"),
            "{xml}"
        );
        let launch = xml
            .split("launch=\"")
            .nth(1)
            .unwrap()
            .split('"')
            .next()
            .unwrap();
        assert_eq!(
            parse_toast_launch(launch),
            Some(NotificationAction {
                id: "copy-1".into(),
                action: "show-job".into()
            })
        );
    }

    #[test]
    fn the_launch_string_round_trips_any_id_and_action() {
        for (id, action) in [
            ("line\nbreak\ttab\r", "ok"),
            ("bell\u{7}\u{1f}unit-separator", "\u{1b}esc"),
            ("quote\" apostrophe' <&> %41", "100% \"done\""),
            ("café — 通知 🔔", "ouvrir/открыть"),
        ] {
            let mut r = request();
            r.id = id.into();
            r.default_action = Some(action.into());
            let xml = toast_xml(&r);
            let launch = xml
                .split("launch=\"")
                .nth(1)
                .unwrap()
                .split('"')
                .next()
                .unwrap();
            assert_eq!(
                parse_toast_launch(launch),
                Some(NotificationAction {
                    id: id.into(),
                    action: action.into()
                }),
                "{id:?}"
            );
        }
    }

    #[test]
    fn a_malformed_launch_string_is_ignored() {
        assert_eq!(parse_toast_launch("a%ZZ\u{1f}b"), None);
        assert_eq!(parse_toast_launch("a%4\u{1f}b"), None);
        assert_eq!(parse_toast_launch("a%FF\u{1f}b"), None);
    }

    #[test]
    fn toast_without_a_body_has_one_text_element() {
        let mut r = request();
        r.body = None;
        r.default_action = None;
        let xml = toast_xml(&r);
        assert_eq!(xml.matches("<text>").count(), 1);
        assert_eq!(parse_toast_launch("copy-1\u{1f}"), None);
    }
}
