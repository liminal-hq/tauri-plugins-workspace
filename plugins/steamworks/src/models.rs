// Wire types shared by the commands, events and backends
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

use serde::{Deserialize, Serialize};

use crate::lobby::LobbyMemberChange;

pub const STATS_RECEIVED_EVENT: &str = "steamworks://stats-received";
pub const STATS_STORED_EVENT: &str = "steamworks://stats-stored";
pub const ACHIEVEMENT_STORED_EVENT: &str = "steamworks://achievement-stored";
pub const OVERLAY_TOGGLED_EVENT: &str = "steamworks://overlay-toggled";
pub const SCREENSHOT_REQUESTED_EVENT: &str = "steamworks://screenshot-requested";

/// Which implementation answers the commands.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum BackendKind {
    Steam,
    Mock,
}

/// Why Steam cannot be used.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum UnavailableReason {
    /// The `steam` cargo feature is off.
    NotCompiled,
    /// Android and iOS never link the SDK.
    UnsupportedPlatform,
    /// The Steam client is not running.
    SteamNotRunning,
    /// Steam refused to initialise; the status message has the detail.
    InitFailed,
    /// The app was not launched through Steam and a relaunch was requested.
    RestartRequired,
    /// The plugin has shut down.
    Shutdown,
}

impl UnavailableReason {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::NotCompiled => "notCompiled",
            Self::UnsupportedPlatform => "unsupportedPlatform",
            Self::SteamNotRunning => "steamNotRunning",
            Self::InitFailed => "initFailed",
            Self::RestartRequired => "restartRequired",
            Self::Shutdown => "shutdown",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(
    tag = "state",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
pub enum SteamStatus {
    Available {
        backend: BackendKind,
        app_id: u32,
    },
    Unavailable {
        reason: UnavailableReason,
        message: Option<String>,
    },
}

impl SteamStatus {
    pub fn unavailable(reason: UnavailableReason) -> Self {
        Self::Unavailable {
            reason,
            message: None,
        }
    }

    pub fn is_available(&self) -> bool {
        matches!(self, Self::Available { .. })
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Player {
    /// The 64-bit Steam ID as a decimal string, because JavaScript numbers lose precision.
    pub steam_id: String,
    pub persona_name: String,
    /// The language the Steam client uses for this app, such as `english` or `french`.
    pub app_language: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Achievement {
    pub id: String,
    pub achieved: bool,
    /// Unix time of the unlock, when Steam reports one.
    pub unlock_time: Option<u64>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum StatKind {
    Int,
    Float,
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", content = "value", rename_all = "lowercase")]
pub enum StatValue {
    Int(i32),
    Float(f32),
}

impl StatValue {
    pub fn kind(&self) -> StatKind {
        match self {
            Self::Int(_) => StatKind::Int,
            Self::Float(_) => StatKind::Float,
        }
    }
}

/// Downloadable content the app offers.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DlcInfo {
    pub app_id: u32,
    pub name: String,
    /// Whether the player can get it, as Steam reports availability.
    pub available: bool,
    /// Whether the player owns it and has it installed.
    pub installed: bool,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DeviceInfo {
    /// Whether the app runs on a Steam Deck.
    pub steam_deck: bool,
    /// Whether Steam is in Big Picture mode.
    pub big_picture: bool,
}

/// A file in Steam Cloud.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CloudFile {
    pub name: String,
    pub size: u64,
    /// Unix time the file was last written.
    pub timestamp: i64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CloudQuota {
    pub total_bytes: u64,
    pub available_bytes: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CloudStatus {
    /// Whether the player's account allows Steam Cloud.
    pub account_enabled: bool,
    /// Whether the player has Steam Cloud turned on for this app.
    pub app_enabled: bool,
}

/// A page of the Steam overlay.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum OverlayPage {
    Friends,
    Community,
    Players,
    Settings,
    OfficialGameGroup,
    Stats,
    Achievements,
}

impl OverlayPage {
    /// The dialog name Steam expects.
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Friends => "friends",
            Self::Community => "community",
            Self::Players => "players",
            Self::Settings => "settings",
            Self::OfficialGameGroup => "officialgamegroup",
            Self::Stats => "stats",
            Self::Achievements => "achievements",
        }
    }
}

/// Events the backends raise. The plugin turns them into Tauri events.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SteamEvent {
    StatsReceived {
        success: bool,
        steam_id: String,
    },
    StatsStored {
        success: bool,
    },
    AchievementStored {
        id: String,
        current: u32,
        max: u32,
    },
    OverlayToggled {
        active: bool,
    },
    ScreenshotRequested,
    LobbyChatUpdate {
        lobby_id: String,
        user_changed: String,
        making_change: String,
        change: LobbyMemberChange,
    },
    LobbyDataUpdate {
        lobby_id: String,
        member_id: String,
        success: bool,
    },
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn stat_values_use_kind_and_value() {
        assert_eq!(
            serde_json::to_value(StatValue::Int(3)).unwrap(),
            json!({"kind": "int", "value": 3})
        );
        let parsed: StatValue =
            serde_json::from_value(json!({"kind": "float", "value": 1.5})).unwrap();
        assert_eq!(parsed, StatValue::Float(1.5));
    }

    #[test]
    fn status_serialises_with_a_state_tag() {
        let status = SteamStatus::Available {
            backend: BackendKind::Mock,
            app_id: 480,
        };
        assert_eq!(
            serde_json::to_value(status).unwrap(),
            json!({"state": "available", "backend": "mock", "appId": 480})
        );
        let off = SteamStatus::unavailable(UnavailableReason::SteamNotRunning);
        assert_eq!(
            serde_json::to_value(off).unwrap(),
            json!({"state": "unavailable", "reason": "steamNotRunning", "message": null})
        );
    }

    #[test]
    fn overlay_pages_serialise_as_steam_dialog_names() {
        for page in [
            OverlayPage::Friends,
            OverlayPage::Community,
            OverlayPage::Players,
            OverlayPage::Settings,
            OverlayPage::OfficialGameGroup,
            OverlayPage::Stats,
            OverlayPage::Achievements,
        ] {
            assert_eq!(
                serde_json::to_value(page).unwrap(),
                serde_json::Value::String(page.as_str().to_string())
            );
        }
    }

    #[test]
    fn reason_strings_match_serde_names() {
        for reason in [
            UnavailableReason::NotCompiled,
            UnavailableReason::UnsupportedPlatform,
            UnavailableReason::SteamNotRunning,
            UnavailableReason::InitFailed,
            UnavailableReason::RestartRequired,
            UnavailableReason::Shutdown,
        ] {
            assert_eq!(
                serde_json::to_value(reason).unwrap(),
                serde_json::Value::String(reason.as_str().to_string())
            );
        }
    }
}
