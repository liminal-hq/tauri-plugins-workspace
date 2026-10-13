// Defines the serialisable requests, results and capabilities of the share-sheet commands
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

use serde::{Deserialize, Serialize};
use ts_rs::TS;

/// A file to hand to the share sheet.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../guest-js/bindings/")]
pub struct ShareFileOptions {
    /// Absolute path of the file. On Android it must be inside the app's cache or files directory.
    pub path: String,
    /// MIME type of the file, such as `application/zip`.
    pub mime: String,
    /// Optional title, shown or used as a subject where the platform supports one.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub title: Option<String>,
}

/// Text to hand to the share sheet.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../guest-js/bindings/")]
pub struct ShareTextOptions {
    /// The text to share.
    pub text: String,
    /// Optional title, shown or used as a subject where the platform supports one.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub title: Option<String>,
}

/// How a share ended, as far as the platform can tell.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "lowercase")]
#[ts(export, export_to = "../guest-js/bindings/")]
pub enum ShareStatus {
    /// The person picked a share target (and, on iOS, the target reported it finished).
    Shared,
    /// The person dismissed the share sheet without picking a target.
    Cancelled,
    /// The share sheet was shown, but the platform does not say how it ended.
    Unknown,
}

/// The result of a share call.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../guest-js/bindings/")]
pub struct ShareResult {
    pub status: ShareStatus,
}

impl ShareResult {
    pub fn new(status: ShareStatus) -> Self {
        Self { status }
    }

    /// iOS reports whether the activity completed; anything else was dismissed.
    pub fn from_completed(completed: bool) -> Self {
        Self::new(if completed {
            ShareStatus::Shared
        } else {
            ShareStatus::Cancelled
        })
    }

    /// macOS names the chosen sharing service, or none when the picker was dismissed.
    pub fn from_chosen_service(chosen: bool) -> Self {
        Self::from_completed(chosen)
    }
}

/// What the share sheet can do on this system, so a UI can hide its Share button.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../guest-js/bindings/")]
pub struct ShareCapabilities {
    /// Whether any share sheet exists here.
    pub supported: bool,
    /// Whether `share_file` can work.
    pub files: bool,
    /// Whether `share_text` can work.
    pub text: bool,
}

/// The platforms the plugin knows about, used to look up their capabilities.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SharePlatform {
    Android,
    Ios,
    Macos,
    Windows,
    Linux,
    /// Any other target, such as a BSD.
    Other,
}

impl SharePlatform {
    /// The platform this crate was compiled for.
    pub const fn current() -> Self {
        if cfg!(target_os = "android") {
            Self::Android
        } else if cfg!(target_os = "ios") {
            Self::Ios
        } else if cfg!(target_os = "macos") {
            Self::Macos
        } else if cfg!(target_os = "windows") {
            Self::Windows
        } else if cfg!(target_os = "linux") {
            Self::Linux
        } else {
            Self::Other
        }
    }
}

impl ShareCapabilities {
    /// Nothing can be shared.
    pub const NONE: Self = Self {
        supported: false,
        files: false,
        text: false,
    };

    /// Files and text can both be shared.
    pub const ALL: Self = Self {
        supported: true,
        files: true,
        text: true,
    };

    /// The capabilities a platform offers when its share UI is present. Linux has no standard
    /// share UI, so it offers nothing; Windows also checks at run time that the share UI exists.
    pub const fn for_platform(platform: SharePlatform) -> Self {
        match platform {
            SharePlatform::Android
            | SharePlatform::Ios
            | SharePlatform::Macos
            | SharePlatform::Windows => Self::ALL,
            SharePlatform::Linux | SharePlatform::Other => Self::NONE,
        }
    }

    /// The static capabilities, narrowed by a run-time check that the share UI is available.
    pub const fn with_runtime_support(self, available: bool) -> Self {
        if available {
            self
        } else {
            Self::NONE
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn file_options_read_camel_case_and_an_optional_title() {
        let options: ShareFileOptions =
            serde_json::from_value(json!({ "path": "/tmp/save.zip", "mime": "application/zip" }))
                .unwrap();
        assert_eq!(options.title, None);

        let titled: ShareFileOptions = serde_json::from_value(
            json!({ "path": "/tmp/save.zip", "mime": "application/zip", "title": "Save" }),
        )
        .unwrap();
        assert_eq!(titled.title.as_deref(), Some("Save"));
    }

    #[test]
    fn file_options_need_a_path_and_a_mime_type() {
        assert!(serde_json::from_value::<ShareFileOptions>(json!({ "path": "/a" })).is_err());
        assert!(serde_json::from_value::<ShareFileOptions>(json!({ "mime": "a/b" })).is_err());
    }

    #[test]
    fn text_options_need_text() {
        assert!(serde_json::from_value::<ShareTextOptions>(json!({ "title": "t" })).is_err());
        let options: ShareTextOptions = serde_json::from_value(json!({ "text": "hi" })).unwrap();
        assert_eq!(options.text, "hi");
    }

    #[test]
    fn statuses_serialise_as_lowercase_words() {
        assert_eq!(
            serde_json::to_value(ShareResult::new(ShareStatus::Shared)).unwrap(),
            json!({ "status": "shared" })
        );
        assert_eq!(
            serde_json::to_value(ShareStatus::Cancelled).unwrap(),
            json!("cancelled")
        );
        assert_eq!(
            serde_json::to_value(ShareStatus::Unknown).unwrap(),
            json!("unknown")
        );
    }

    #[test]
    fn native_results_deserialise_from_the_mobile_plugins() {
        let result: ShareResult = serde_json::from_value(json!({ "status": "unknown" })).unwrap();
        assert_eq!(result.status, ShareStatus::Unknown);
        assert!(serde_json::from_value::<ShareResult>(json!({ "status": "done" })).is_err());
    }

    #[test]
    fn completion_flags_map_to_shared_or_cancelled() {
        assert_eq!(
            ShareResult::from_completed(true).status,
            ShareStatus::Shared
        );
        assert_eq!(
            ShareResult::from_completed(false).status,
            ShareStatus::Cancelled
        );
        assert_eq!(
            ShareResult::from_chosen_service(true).status,
            ShareStatus::Shared
        );
        assert_eq!(
            ShareResult::from_chosen_service(false).status,
            ShareStatus::Cancelled
        );
    }

    #[test]
    fn every_platform_with_a_share_ui_offers_files_and_text() {
        for platform in [
            SharePlatform::Android,
            SharePlatform::Ios,
            SharePlatform::Macos,
            SharePlatform::Windows,
        ] {
            assert_eq!(
                ShareCapabilities::for_platform(platform),
                ShareCapabilities::ALL,
                "{platform:?}"
            );
        }
    }

    #[test]
    fn linux_and_other_targets_offer_nothing() {
        assert_eq!(
            ShareCapabilities::for_platform(SharePlatform::Linux),
            ShareCapabilities::NONE
        );
        assert_eq!(
            ShareCapabilities::for_platform(SharePlatform::Other),
            ShareCapabilities::NONE
        );
    }

    #[test]
    fn a_missing_share_ui_at_run_time_offers_nothing() {
        let windows = ShareCapabilities::for_platform(SharePlatform::Windows);
        assert_eq!(windows.with_runtime_support(true), ShareCapabilities::ALL);
        assert_eq!(windows.with_runtime_support(false), ShareCapabilities::NONE);
    }

    #[test]
    fn capabilities_serialise_as_camel_case() {
        assert_eq!(
            serde_json::to_value(ShareCapabilities::ALL).unwrap(),
            json!({ "supported": true, "files": true, "text": true })
        );
    }

    #[test]
    fn the_current_platform_matches_the_compile_target() {
        #[cfg(target_os = "linux")]
        assert_eq!(SharePlatform::current(), SharePlatform::Linux);
        #[cfg(target_os = "macos")]
        assert_eq!(SharePlatform::current(), SharePlatform::Macos);
        #[cfg(target_os = "windows")]
        assert_eq!(SharePlatform::current(), SharePlatform::Windows);
    }
}
