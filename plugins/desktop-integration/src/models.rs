// Defines the serialisable models of the desktop-integration service commands and events
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

use serde::{Deserialize, Serialize};
use ts_rs::TS;

/// A feature the plugin can offer, as reported by `get_status`.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../guest-js/bindings/")]
pub enum Feature {
    /// Desktop notifications outside the portal.
    Notify,
    /// Action buttons on notifications. On Linux the notification server must list `actions` in
    /// its capabilities.
    NotificationActions,
    /// Keeping the machine awake.
    InhibitSleep,
    /// Progress and count on the dock or taskbar icon.
    LauncherProgress,
    /// Owning `org.freedesktop.FileManager1`.
    FileManager,
    /// The Windows global shortcut thread.
    GlobalShortcuts,
}

/// Why a feature is not available.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, TS)]
#[serde(rename_all = "kebab-case")]
#[ts(export, export_to = "../guest-js/bindings/")]
pub enum UnavailableReason {
    /// The feature does not exist on this operating system.
    PlatformUnsupported,
    /// There is no session bus to reach the service on.
    NoSessionBus,
    /// The session bus has no notification server.
    NoNotificationServer,
    /// The system bus has no `systemd-logind`.
    NoLogind,
    /// Windows toasts need an AppUserModelID and the process has none.
    NeedsAppId,
    /// A display server is needed and none is running.
    NoDisplayServer,
    /// The system works, but this feature is not offered by it; `detail` says why.
    ActionsUnsupported,
}

/// Whether one feature works on this system.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../guest-js/bindings/")]
pub struct FeatureStatus {
    pub feature: Feature,
    pub available: bool,
    /// Why the feature is unavailable; absent when it works.
    pub reason: Option<UnavailableReason>,
    /// The error text behind the reason, for logs and the Services panel.
    pub detail: Option<String>,
}

impl FeatureStatus {
    pub fn available(feature: Feature) -> Self {
        Self {
            feature,
            available: true,
            reason: None,
            detail: None,
        }
    }

    pub fn unavailable(
        feature: Feature,
        reason: UnavailableReason,
        detail: Option<String>,
    ) -> Self {
        Self {
            feature,
            available: false,
            reason: Some(reason),
            detail,
        }
    }
}

/// What the plugin can do on the running system.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../guest-js/bindings/")]
pub struct PluginStatus {
    /// True when at least one feature is available.
    pub available: bool,
    /// Why nothing is available; absent otherwise.
    pub reason: Option<UnavailableReason>,
    pub features: Vec<FeatureStatus>,
    /// Whether this process currently owns `org.freedesktop.FileManager1`.
    pub file_manager_owned: bool,
}

impl PluginStatus {
    pub fn new(features: Vec<FeatureStatus>, file_manager_owned: bool) -> Self {
        let available = features.iter().any(|f| f.available);
        let reason = if available {
            None
        } else {
            features.first().and_then(|f| f.reason)
        };
        Self {
            available,
            reason,
            features,
            file_manager_owned,
        }
    }

    pub fn feature(&self, feature: Feature) -> Option<&FeatureStatus> {
        self.features.iter().find(|f| f.feature == feature)
    }
}

/// How urgent a notification is.
#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize, PartialEq, Eq, TS)]
#[serde(rename_all = "kebab-case")]
#[ts(export, export_to = "../guest-js/bindings/")]
pub enum Urgency {
    Low,
    #[default]
    Normal,
    /// Urgent notifications are not removed by the shell on their own; use sparingly.
    Critical,
}

/// A desktop notification to show or replace.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../guest-js/bindings/")]
pub struct NotifyRequest {
    /// The caller's id. Sending the same id again replaces the shown notification.
    pub id: String,
    pub title: String,
    pub body: Option<String>,
    /// The action id reported through the `notification-action` event when the user clicks the
    /// notification itself.
    pub default_action: Option<String>,
    pub urgency: Option<Urgency>,
    /// The application name the shell shows; defaults to the package name.
    pub app_name: Option<String>,
    /// The `.desktop` file id of the app, so the shell can group and style the notification;
    /// defaults to the bundle identifier.
    pub desktop_id: Option<String>,
    /// Buttons to show on the notification, in order. At most 3 are shown; extras are dropped.
    /// A press is reported through the `notification-action` event with the button's `id`
    /// (`default` is reserved for the click on the notification). Servers that do not draw
    /// buttons (see the `notificationActions` feature) show only the default click.
    #[serde(default)]
    #[ts(optional)]
    pub actions: Option<Vec<ActionButton>>,
}

/// One action button on a notification.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../guest-js/bindings/")]
pub struct ActionButton {
    /// Reported as the event's `action` when the button is pressed.
    pub id: String,
    /// The text on the button.
    pub label: String,
}

/// Payload of the `desktop-integration://notification-action` event.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../guest-js/bindings/")]
pub struct NotificationAction {
    /// The id of the notification the user acted on.
    pub id: String,
    /// The request's `defaultAction` for a click on the notification, or the `id` of the button
    /// that was pressed.
    pub action: String,
}

/// What to keep the machine from doing.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, TS)]
#[serde(rename_all = "kebab-case")]
#[ts(export, export_to = "../guest-js/bindings/")]
pub enum SleepKind {
    /// Suspending the machine.
    Sleep,
    /// Idling: screen blanking and locking. Linux only; Windows keeps the display on.
    Idle,
}

/// A request to keep the machine awake.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../guest-js/bindings/")]
pub struct SleepInhibitRequest {
    /// What the user is told the app is doing, such as "Copying 3 files".
    pub reason: String,
    /// What to inhibit; empty means sleep only.
    #[serde(default)]
    pub kinds: Vec<SleepKind>,
}

/// The handle of a live sleep inhibitor, to give back to `release_sleep_inhibit`.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../guest-js/bindings/")]
pub struct SleepInhibitHandle {
    pub handle: u32,
}

/// What the launcher or taskbar icon shows.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, TS)]
#[serde(tag = "state", rename_all = "kebab-case")]
#[ts(export, export_to = "../guest-js/bindings/")]
pub enum LauncherProgress {
    /// A fraction from 0 to 1.
    Value { value: f64 },
    /// Busy with no known end. Linux launchers have no such state and show an empty bar.
    Indeterminate,
    /// No progress shown.
    Cleared,
}

/// Progress and count for the app's dock or taskbar entry.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../guest-js/bindings/")]
pub struct LauncherRequest {
    pub progress: LauncherProgress,
    /// A badge count, such as the number of running jobs; zero hides it. Absent leaves it alone.
    /// Linux only.
    #[ts(type = "number | null")]
    pub count: Option<i64>,
    /// The `.desktop` file id the dock matches the signal to; defaults to the bundle identifier.
    /// Linux only.
    pub desktop_id: Option<String>,
    /// The window whose taskbar button shows the progress. Windows only; defaults to the focused
    /// window, else the first window whose label starts with `main`, else the first label in alphabetical order.
    pub window_label: Option<String>,
}

/// The `org.freedesktop.FileManager1` method another application called.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, TS)]
#[serde(rename_all = "kebab-case")]
#[ts(export, export_to = "../guest-js/bindings/")]
pub enum FileManagerMethod {
    /// Show these folders.
    ShowFolders,
    /// Show the folders containing these items and select them.
    ShowItems,
    /// Show the properties of these items.
    ShowItemProperties,
}

/// One URI from a `FileManager1` call.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../guest-js/bindings/")]
pub struct FileManagerTarget {
    /// The URI as the caller sent it.
    pub uri: String,
    /// The local path, percent-decoded, when the URI is a `file:` URI for this machine.
    pub path: Option<String>,
}

/// Payload of the `desktop-integration://file-manager` event.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../guest-js/bindings/")]
pub struct FileManagerCall {
    pub method: FileManagerMethod,
    pub targets: Vec<FileManagerTarget>,
    /// The caller's startup id, for focus-stealing prevention; often empty.
    pub startup_id: String,
}

/// Whether this process owns `org.freedesktop.FileManager1`.
///
/// This is the return value of `own_file_manager` and `disown_file_manager` and the payload of the
/// `desktop-integration://file-manager-ownership` event, which also reports a name taken away.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../guest-js/bindings/")]
pub struct FileManagerOwnership {
    pub owned: bool,
    /// Why the name is not owned, when it was lost or could not be taken.
    pub reason: Option<String>,
}

/// A global shortcut to register on Windows.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../guest-js/bindings/")]
pub struct GlobalShortcutRequest {
    /// The caller's id for the shortcut, reported by the `pressed` event.
    pub id: String,
    /// A Tauri-style accelerator such as `Ctrl+Alt+K`.
    pub accelerator: String,
}

/// Payload of the `desktop-integration://shortcut-pressed` event.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../guest-js/bindings/")]
pub struct GlobalShortcutPressed {
    pub id: String,
}
