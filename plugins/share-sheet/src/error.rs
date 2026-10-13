// Defines the typed share-sheet error and maps native plugin rejections onto it
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

use serde::{ser::SerializeStruct, Deserialize, Serialize, Serializer};
use ts_rs::TS;

/// Why a share call was rejected. The guest bindings expose it as `ShareSheetError.code`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "kebab-case")]
#[ts(export, export_to = "../guest-js/bindings/")]
pub enum ErrorCode {
    /// There is no share UI here (Linux, or a Windows build without one).
    Unsupported,
    /// A required argument is empty or malformed, such as an empty text or a bad MIME type.
    InvalidArgument,
    /// The path is not absolute, is not a regular file, or is outside the directories the
    /// platform allows (the app's cache and files directories on Android).
    InvalidPath,
    /// No file exists at the path.
    NotFound,
    /// The platform failed to show the share UI.
    Failed,
}

impl ErrorCode {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Unsupported => "unsupported",
            Self::InvalidArgument => "invalid-argument",
            Self::InvalidPath => "invalid-path",
            Self::NotFound => "not-found",
            Self::Failed => "failed",
        }
    }

    /// Maps the code a Kotlin or Swift rejection carries; anything unrecognised is a failure.
    pub fn from_native(code: Option<&str>) -> Self {
        match code {
            Some("unsupported") => Self::Unsupported,
            Some("invalid-argument") => Self::InvalidArgument,
            Some("invalid-path") => Self::InvalidPath,
            Some("not-found") => Self::NotFound,
            _ => Self::Failed,
        }
    }
}

/// A rejected share call, serialised to the webview as `{ code, message }`.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[error("{message}")]
pub struct Error {
    pub code: ErrorCode,
    pub message: String,
}

impl Error {
    pub fn new(code: ErrorCode, message: impl Into<String>) -> Self {
        Self {
            code,
            message: message.into(),
        }
    }

    pub fn unsupported(message: impl Into<String>) -> Self {
        Self::new(ErrorCode::Unsupported, message)
    }

    pub fn invalid_argument(message: impl Into<String>) -> Self {
        Self::new(ErrorCode::InvalidArgument, message)
    }

    pub fn invalid_path(message: impl Into<String>) -> Self {
        Self::new(ErrorCode::InvalidPath, message)
    }

    pub fn not_found(message: impl Into<String>) -> Self {
        Self::new(ErrorCode::NotFound, message)
    }

    pub fn failed(message: impl Into<String>) -> Self {
        Self::new(ErrorCode::Failed, message)
    }

    /// Maps a rejection from the Kotlin or Swift plugin, keeping its code and message.
    pub fn from_native(code: Option<&str>, message: Option<&str>) -> Self {
        let code = ErrorCode::from_native(code);
        let message = match message {
            Some(message) if !message.is_empty() => message.to_string(),
            _ => format!(
                "the native share sheet rejected the request ({})",
                code.as_str()
            ),
        };
        Self::new(code, message)
    }
}

#[cfg(mobile)]
impl From<tauri::plugin::mobile::PluginInvokeError> for Error {
    fn from(error: tauri::plugin::mobile::PluginInvokeError) -> Self {
        match error {
            tauri::plugin::mobile::PluginInvokeError::InvokeRejected(rejected) => {
                Self::from_native(rejected.code.as_deref(), rejected.message.as_deref())
            }
            other => Self::failed(other.to_string()),
        }
    }
}

impl Serialize for Error {
    fn serialize<S>(&self, serializer: S) -> std::result::Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let mut state = serializer.serialize_struct("Error", 2)?;
        state.serialize_field("code", &self.code)?;
        state.serialize_field("message", &self.message)?;
        state.end()
    }
}

pub type Result<T> = std::result::Result<T, Error>;

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn errors_serialise_as_a_code_and_a_message() {
        let error = Error::not_found("no file at /a");
        assert_eq!(
            serde_json::to_value(&error).unwrap(),
            json!({ "code": "not-found", "message": "no file at /a" })
        );
        assert_eq!(error.to_string(), "no file at /a");
    }

    #[test]
    fn every_code_serialises_as_its_kebab_case_name() {
        for code in [
            ErrorCode::Unsupported,
            ErrorCode::InvalidArgument,
            ErrorCode::InvalidPath,
            ErrorCode::NotFound,
            ErrorCode::Failed,
        ] {
            assert_eq!(serde_json::to_value(code).unwrap(), json!(code.as_str()));
            assert_eq!(ErrorCode::from_native(Some(code.as_str())), code);
        }
    }

    #[test]
    fn unrecognised_native_codes_are_failures() {
        assert_eq!(ErrorCode::from_native(None), ErrorCode::Failed);
        assert_eq!(ErrorCode::from_native(Some("")), ErrorCode::Failed);
        assert_eq!(ErrorCode::from_native(Some("NOT_FOUND")), ErrorCode::Failed);
    }

    #[test]
    fn native_rejections_keep_their_message_or_get_one() {
        let kept = Error::from_native(Some("invalid-path"), Some("outside the cache"));
        assert_eq!(kept, Error::invalid_path("outside the cache"));

        let filled = Error::from_native(None, None);
        assert_eq!(filled.code, ErrorCode::Failed);
        assert!(filled.message.contains("failed"), "{}", filled.message);

        let empty = Error::from_native(Some("not-found"), Some(""));
        assert_eq!(empty.code, ErrorCode::NotFound);
        assert!(!empty.message.is_empty());
    }
}
