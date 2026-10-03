// Defines serialisable models for XDG portal IPC payloads
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

use serde::{Deserialize, Serialize};
use ts_rs::TS;

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../guest-js/bindings/")]
pub struct AvailabilityInfo {
    pub is_linux: bool,
    pub sandboxed: bool,
    pub portal_available: bool,
}

/// Colour scheme preference from the desktop portal.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, TS)]
#[serde(rename_all = "kebab-case")]
#[ts(export, export_to = "../guest-js/bindings/")]
pub enum ColourScheme {
    NoPreference,
    PreferDark,
    PreferLight,
}

/// Desktop environment family, used to select widget style maps.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, TS)]
#[serde(rename_all = "kebab-case")]
#[ts(export, export_to = "../guest-js/bindings/")]
pub enum DesktopEnvironment {
    Gnome,
    Kde,
    Cinnamon,
    Mate,
    Xfce,
    Unknown,
}

/// Accent colour as sRGB values in 0.0–1.0 range.
/// Absent if the desktop does not report an accent colour.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../guest-js/bindings/")]
pub struct AccentColour {
    pub r: f64,
    pub g: f64,
    pub b: f64,
}

/// Combined theme information from the desktop portal and environment.
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../guest-js/bindings/")]
pub struct ThemeInfo {
    pub colour_scheme: ColourScheme,
    pub accent_colour: Option<AccentColour>,
    pub high_contrast: bool,
    pub desktop_environment: DesktopEnvironment,
}

/// A feature the plugin can offer, as reported by `get_status`.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../guest-js/bindings/")]
pub enum PortalFeature {
    /// `org.freedesktop.portal.Notification`.
    Notification,
    /// Action buttons on notifications: the Notification portal's `buttons`, from interface
    /// version 1.
    NotificationActions,
    /// `org.freedesktop.portal.Inhibit`.
    Inhibit,
    /// `org.freedesktop.portal.OpenURI`.
    OpenUri,
}

/// Why a feature is not available.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, TS)]
#[serde(rename_all = "kebab-case")]
#[ts(export, export_to = "../guest-js/bindings/")]
pub enum UnavailableReason {
    /// The operating system has no xdg-desktop-portal.
    PlatformUnsupported,
    /// The session bus is unreachable or no portal is running on it.
    NoPortal,
    /// A portal runs, but its backend does not offer this interface.
    InterfaceMissing,
    /// The portal did not answer within the time allowed.
    NoResponse,
    /// The portal answers only callers it can identify, and this process is not in a sandbox.
    NotSandboxed,
    /// The portal works, but this feature is not offered by it; `detail` says why.
    ActionsUnsupported,
}

/// Whether one portal interface works on this session.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../guest-js/bindings/")]
pub struct FeatureStatus {
    pub feature: PortalFeature,
    pub available: bool,
    /// Why the feature is unavailable; absent when it works.
    pub reason: Option<UnavailableReason>,
    /// The error text behind the reason, for logs and the Services panel.
    pub detail: Option<String>,
    /// The version of the portal interface, when it was read.
    pub version: Option<u32>,
}

impl FeatureStatus {
    pub fn available(feature: PortalFeature, version: Option<u32>) -> Self {
        Self {
            feature,
            available: true,
            reason: None,
            detail: None,
            version,
        }
    }

    pub fn unavailable(
        feature: PortalFeature,
        reason: UnavailableReason,
        detail: Option<String>,
    ) -> Self {
        Self {
            feature,
            available: false,
            reason: Some(reason),
            detail,
            version: None,
        }
    }
}

/// What the plugin's portal features can do on the running session.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../guest-js/bindings/")]
pub struct PortalStatus {
    /// True when at least one feature is available.
    pub available: bool,
    /// Why nothing is available; absent otherwise.
    pub reason: Option<UnavailableReason>,
    /// Whether this process runs inside a Flatpak or Snap sandbox.
    pub sandboxed: bool,
    pub features: Vec<FeatureStatus>,
}

impl PortalStatus {
    pub fn new(sandboxed: bool, features: Vec<FeatureStatus>) -> Self {
        let available = features.iter().any(|f| f.available);
        // When nothing works, the first feature's reason is the one to show: a missing portal
        // takes every interface with it.
        let reason = if available {
            None
        } else {
            features.first().and_then(|f| f.reason)
        };
        Self {
            available,
            reason,
            sandboxed,
            features,
        }
    }

    pub fn feature(&self, feature: PortalFeature) -> Option<&FeatureStatus> {
        self.features.iter().find(|f| f.feature == feature)
    }
}

/// How urgent a notification is; the portal maps it to the shell's own priority levels.
#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize, PartialEq, Eq, TS)]
#[serde(rename_all = "kebab-case")]
#[ts(export, export_to = "../guest-js/bindings/")]
pub enum Urgency {
    Low,
    #[default]
    Normal,
    High,
    Urgent,
}

/// A desktop notification to show or replace.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../guest-js/bindings/")]
pub struct NotificationRequest {
    /// The caller's id for this notification. Sending the same id again replaces the shown one;
    /// withdrawing takes it off screen.
    pub id: String,
    pub title: String,
    pub body: Option<String>,
    /// The action id to report through the `action` event when the user clicks the notification
    /// itself. An id that starts with `app.` is instead activated through the
    /// `org.freedesktop.Application` interface and never reaches the event.
    pub default_action: Option<String>,
    pub urgency: Option<Urgency>,
    /// Buttons to show on the notification, in order. At most 3 are shown; extras are dropped.
    /// A press is reported through the `action` event with the button's `id`. Desktops that do
    /// not draw buttons (see the `notificationActions` feature) show only the default click. A
    /// button id must differ from `defaultAction`, so the two can be told apart in the event.
    #[serde(default)]
    #[ts(optional)]
    pub actions: Option<Vec<ActionButton>>,
}

/// One action button on a notification.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../guest-js/bindings/")]
pub struct ActionButton {
    /// Reported as the event's `action` when the button is pressed. An id that starts with
    /// `app.` is instead activated through `org.freedesktop.Application` and never reaches the
    /// event.
    pub id: String,
    /// The text on the button.
    pub label: String,
}

/// Payload of the `xdg-portal://notification-action` event.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../guest-js/bindings/")]
pub struct NotificationAction {
    /// The id of the notification the user acted on.
    pub id: String,
    /// The action id: the request's `defaultAction` for a click on the notification, or the `id`
    /// of the button that was pressed.
    pub action: String,
}

/// What to keep the session from doing.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, TS)]
#[serde(rename_all = "kebab-case")]
#[ts(export, export_to = "../guest-js/bindings/")]
pub enum InhibitKind {
    /// Idling: screen blanking and locking.
    Idle,
    /// Suspending the machine.
    Suspend,
}

/// A request to keep the session from idling or suspending.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../guest-js/bindings/")]
pub struct InhibitRequest {
    /// What the user is told the app is doing, such as "Copying 3 files".
    pub reason: String,
    /// What to inhibit; empty means both.
    #[serde(default)]
    pub kinds: Vec<InhibitKind>,
}

/// The handle of a live inhibitor, to give back to `release_inhibit`.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../guest-js/bindings/")]
pub struct InhibitHandle {
    pub handle: u32,
}

/// A request to open a URI or a local file or folder with the user's chosen application.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../guest-js/bindings/")]
pub struct OpenUriRequest {
    /// A URI with a scheme. `file:` URIs open a file or, when the target is a folder, show it.
    pub uri: String,
    /// Always ask which application to use.
    pub ask: Option<bool>,
    /// Open the file for writing. Applies to `file:` URIs.
    pub writable: Option<bool>,
}
