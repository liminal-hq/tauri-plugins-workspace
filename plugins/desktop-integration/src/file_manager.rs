// Turns the URIs of a FileManager1 call into targets the app can open
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

use crate::{
    error::ServiceError,
    models::{FileManagerCall, FileManagerMethod, FileManagerTarget},
};

/// The event emitted for each `FileManager1` call.
pub const CALL_EVENT: &str = "desktop-integration://file-manager";

/// The event emitted when the name is lost or taken.
pub const OWNERSHIP_EVENT: &str = "desktop-integration://file-manager-ownership";

/// The well-known name, object path and interface of the file manager service.
pub const BUS_NAME: &str = "org.freedesktop.FileManager1";
pub const OBJECT_PATH: &str = "/org/freedesktop/FileManager1";

/// The held name, tagged with the generation that took it, so a message about an earlier
/// ownership cannot release a later one.
///
/// The name-lost signal is handled on a task of its own, so by the time it runs the app may have
/// given the name back and taken it again; the task releases only the generation that was lost.
#[derive(Debug)]
pub struct OwnerSlot<T> {
    last_generation: u64,
    current: Option<(u64, T)>,
}

impl<T> Default for OwnerSlot<T> {
    fn default() -> Self {
        Self {
            last_generation: 0,
            current: None,
        }
    }
}

impl<T> OwnerSlot<T> {
    pub fn is_owned(&self) -> bool {
        self.current.is_some()
    }

    /// A fresh generation to tag the next ownership with.
    pub fn next_generation(&mut self) -> u64 {
        self.last_generation += 1;
        self.last_generation
    }

    pub fn set(&mut self, generation: u64, value: T) {
        self.current = Some((generation, value));
    }

    /// Takes whatever is held, whichever generation it is.
    pub fn take(&mut self) -> Option<T> {
        self.current.take().map(|(_, value)| value)
    }

    /// Takes the held value only if it is the one `generation` took.
    pub fn take_generation(&mut self, generation: u64) -> Option<T> {
        match self.current {
            Some((held, _)) if held == generation => self.take(),
            _ => None,
        }
    }
}

/// Reads one URI. A `file:` URI for this machine also gives its decoded local path; any other
/// scheme (`smb:`, `sftp:`, `trash:`) is kept as is for the app's own providers.
pub fn target_from_uri(uri: &str) -> Result<FileManagerTarget, ServiceError> {
    let parsed = url::Url::parse(uri)
        .map_err(|error| ServiceError::invalid(format!("{uri:?} is not a URI: {error}")))?;
    let path = if parsed.scheme() == "file" {
        if !matches!(parsed.host_str(), None | Some("") | Some("localhost")) {
            return Err(ServiceError::invalid(format!(
                "{uri:?} names another machine"
            )));
        }
        let path = parsed
            .to_file_path()
            .map_err(|()| ServiceError::invalid(format!("{uri:?} has no usable path")))?;
        Some(path.to_string_lossy().into_owned())
    } else {
        None
    };
    Ok(FileManagerTarget {
        uri: uri.to_string(),
        path,
    })
}

/// Reads the URIs of a call: empty strings are skipped, and the call fails when any other URI is
/// invalid or none is left, so the caller gets an error and nothing reaches the app.
pub fn targets_from_uris(uris: &[String]) -> Result<Vec<FileManagerTarget>, ServiceError> {
    let targets = uris
        .iter()
        .filter(|uri| !uri.is_empty())
        .map(|uri| target_from_uri(uri))
        .collect::<Result<Vec<_>, _>>()?;
    if targets.is_empty() {
        return Err(ServiceError::invalid("the call names no URI"));
    }
    Ok(targets)
}

/// Builds the event payload for a call.
pub fn call_from(
    method: FileManagerMethod,
    uris: &[String],
    startup_id: &str,
) -> Result<FileManagerCall, ServiceError> {
    Ok(FileManagerCall {
        method,
        targets: targets_from_uris(uris)?,
        startup_id: startup_id.to_string(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_stale_name_lost_message_does_not_release_a_newer_ownership() {
        let mut slot = OwnerSlot::default();
        let first = slot.next_generation();
        slot.set(first, "first");
        // The name is lost, but before the handler runs the app disowns and owns again.
        assert_eq!(slot.take(), Some("first"));
        let second = slot.next_generation();
        slot.set(second, "second");

        assert_eq!(slot.take_generation(first), None);
        assert!(slot.is_owned(), "the healthy ownership stays");
        assert_eq!(slot.take_generation(second), Some("second"));
        assert!(!slot.is_owned());
    }

    #[test]
    fn the_lost_generation_is_released_once() {
        let mut slot = OwnerSlot::default();
        let generation = slot.next_generation();
        slot.set(generation, "held");
        assert_eq!(slot.take_generation(generation), Some("held"));
        assert_eq!(slot.take_generation(generation), None);
    }
    use crate::error::ServiceErrorKind;

    #[test]
    fn a_file_uri_gives_its_decoded_path() {
        let target = target_from_uri("file:///home/me/My%20Folder/a%23b.txt").unwrap();
        assert_eq!(target.uri, "file:///home/me/My%20Folder/a%23b.txt");
        assert_eq!(target.path.as_deref(), Some("/home/me/My Folder/a#b.txt"));
    }

    #[test]
    fn other_schemes_have_no_local_path() {
        let target = target_from_uri("smb://server/share/dir").unwrap();
        assert_eq!(target.path, None);
        assert_eq!(target.uri, "smb://server/share/dir");
    }

    #[test]
    fn rejects_foreign_hosts_and_non_uris() {
        for bad in ["file://server/share", "/plain/path", "not a uri"] {
            assert_eq!(
                target_from_uri(bad).unwrap_err().kind,
                ServiceErrorKind::InvalidArgument,
                "{bad}"
            );
        }
    }

    #[test]
    fn skips_empty_uris_but_needs_at_least_one() {
        let uris = vec![String::new(), "file:///tmp".to_string()];
        assert_eq!(targets_from_uris(&uris).unwrap().len(), 1);
        assert!(targets_from_uris(&[String::new()]).is_err());
        assert!(targets_from_uris(&[]).is_err());
        assert!(targets_from_uris(&["file:///tmp".to_string(), "oops".to_string()]).is_err());
    }

    #[test]
    fn builds_the_event_payload() {
        let call = call_from(
            FileManagerMethod::ShowItems,
            &["file:///tmp/a".to_string(), "file:///tmp/b".to_string()],
            "start-1",
        )
        .unwrap();
        let json = serde_json::to_value(&call).unwrap();
        assert_eq!(json["method"], "show-items");
        assert_eq!(json["startupId"], "start-1");
        assert_eq!(json["targets"][1]["path"], "/tmp/b");
    }
}
