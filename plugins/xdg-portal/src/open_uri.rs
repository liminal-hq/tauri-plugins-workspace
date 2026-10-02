// Parses what to open and hands it to the OpenURI portal
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

use std::path::PathBuf;

use crate::error::ServiceError;

/// What an [`OpenUriRequest`](crate::models::OpenUriRequest) points at.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Target {
    /// A local file or folder, from a `file:` URI. The portal takes these as a file descriptor.
    Local(PathBuf),
    /// Any other URI, passed as is.
    Remote(String),
}

/// Reads a request's URI. Anything without a scheme, and a `file:` URI naming another host, is
/// rejected so a stray relative path never reaches the portal.
pub fn parse_target(uri: &str) -> Result<Target, ServiceError> {
    let parsed = url::Url::parse(uri)
        .map_err(|error| ServiceError::invalid(format!("not a URI with a scheme: {error}")))?;
    if parsed.scheme() == "file" {
        if !matches!(parsed.host_str(), None | Some("") | Some("localhost")) {
            return Err(ServiceError::invalid("a file: URI must name this machine"));
        }
        let path = parsed
            .to_file_path()
            .map_err(|()| ServiceError::invalid("the file: URI has no usable path"))?;
        Ok(Target::Local(path))
    } else {
        Ok(Target::Remote(uri.to_string()))
    }
}

#[cfg(target_os = "linux")]
pub use linux::open;

#[cfg(target_os = "linux")]
mod linux {
    use std::collections::HashMap;

    use ashpd::zbus::{
        zvariant::{Fd, OwnedObjectPath, Value},
        Connection, Proxy,
    };

    use std::{fs::File, os::unix::fs::OpenOptionsExt, path::Path};

    use super::Target;
    use crate::{
        error::{ServiceError, ServiceErrorKind},
        linux::{DESKTOP_DESTINATION, DESKTOP_PATH},
        timeout::{with_timeout, CALL_TIMEOUT},
    };

    const INTERFACE: &str = "org.freedesktop.portal.OpenURI";

    /// The extra `open(2)` flags for the descriptor handed to the portal. A file that is only
    /// shown needs just an `O_PATH` descriptor, which names it without reading it, so it cannot
    /// block (a FIFO) and needs no read permission. A writable request keeps an ordinary
    /// read-only descriptor, as the portal checks the descriptor before it allows a write.
    pub(super) fn open_flags(writable: bool) -> i32 {
        if writable {
            libc::O_CLOEXEC
        } else {
            libc::O_PATH | libc::O_CLOEXEC
        }
    }

    /// Opens `path` for the portal and says whether it is a directory. Blocking.
    pub(super) fn open_local(path: &Path, writable: bool) -> Result<(File, bool), ServiceError> {
        let fail = |error: std::io::Error| {
            let kind = if error.kind() == std::io::ErrorKind::NotFound {
                ServiceErrorKind::NotFound
            } else {
                ServiceErrorKind::Failed
            };
            ServiceError::new(kind, format!("{}: {error}", path.display()))
        };
        let file = File::options()
            .read(true)
            .custom_flags(open_flags(writable))
            .open(path)
            .map_err(fail)?;
        let is_dir = file.metadata().map(|m| m.is_dir()).unwrap_or(false);
        Ok((file, is_dir))
    }

    /// Asks the portal to open `target`. Resolves when the portal has accepted the call, not when
    /// the user has chosen an application, so an `ask` dialog does not hold the command open.
    pub async fn open(
        connection: &Connection,
        target: &Target,
        ask: Option<bool>,
        writable: Option<bool>,
    ) -> Result<(), ServiceError> {
        let proxy = with_timeout(
            "the OpenURI portal",
            Proxy::new(connection, DESKTOP_DESTINATION, DESKTOP_PATH, INTERFACE),
        )
        .await?;
        let mut options: HashMap<&str, Value<'_>> = HashMap::new();
        if let Some(ask) = ask {
            options.insert("ask", Value::from(ask));
        }
        match target {
            Target::Remote(uri) => {
                let reply: Result<OwnedObjectPath, _> = with_timeout(
                    "OpenURI",
                    proxy.call("OpenURI", &("", uri.as_str(), &options)),
                )
                .await;
                reply.map(drop)
            }
            Target::Local(path) => {
                // Opening can block (a FIFO, a stalled mount), so it runs off the async workers
                // and inside the same time limit as the portal calls.
                let (file, is_dir) = {
                    let (path, writable) = (path.clone(), writable.unwrap_or(false));
                    let opening = tokio::task::spawn_blocking(move || open_local(&path, writable));
                    tokio::time::timeout(CALL_TIMEOUT, opening)
                        .await
                        .map_err(|_| ServiceError::timeout("opening the file"))?
                        .map_err(|error| ServiceError::from_message(error.to_string()))??
                };
                let fd = Fd::from(&file);
                if is_dir {
                    let reply: Result<OwnedObjectPath, _> = with_timeout(
                        "OpenDirectory",
                        proxy.call("OpenDirectory", &("", fd, &options)),
                    )
                    .await;
                    reply.map(drop)
                } else {
                    if let Some(writable) = writable {
                        options.insert("writable", Value::from(writable));
                    }
                    let reply: Result<OwnedObjectPath, _> =
                        with_timeout("OpenFile", proxy.call("OpenFile", &("", fd, &options))).await;
                    reply.map(drop)
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::error::ServiceErrorKind;

    #[cfg(target_os = "linux")]
    #[test]
    fn a_shown_file_is_opened_by_path_and_a_writable_one_is_not() {
        use std::os::unix::fs::MetadataExt;

        let dir = std::env::temp_dir().join(format!("xdg-portal-open-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let file = dir.join("a.txt");
        std::fs::write(&file, "x").unwrap();

        let (opened, is_dir) = linux::open_local(&file, false).unwrap();
        assert!(!is_dir);
        assert_eq!(opened.metadata().unwrap().size(), 1);
        let (_, is_dir) = linux::open_local(&dir, false).unwrap();
        assert!(is_dir);
        assert_ne!(linux::open_flags(false) & libc::O_PATH, 0);
        assert_eq!(linux::open_flags(true) & libc::O_PATH, 0);
        assert!(linux::open_local(&file, true).is_ok());
        assert_eq!(
            linux::open_local(&dir.join("missing"), false)
                .unwrap_err()
                .kind,
            ServiceErrorKind::NotFound
        );

        // A FIFO with no writer would block an ordinary open for good.
        let fifo = dir.join("pipe");
        let c_path = std::ffi::CString::new(fifo.to_str().unwrap()).unwrap();
        // SAFETY: a valid NUL-terminated path.
        assert_eq!(unsafe { libc::mkfifo(c_path.as_ptr(), 0o600) }, 0);
        assert!(linux::open_local(&fifo, false).is_ok());
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn a_file_uri_becomes_a_decoded_path() {
        assert_eq!(
            parse_target("file:///home/me/My%20Docs/a.txt").unwrap(),
            Target::Local(PathBuf::from("/home/me/My Docs/a.txt"))
        );
        assert_eq!(
            parse_target("file://localhost/tmp").unwrap(),
            Target::Local(PathBuf::from("/tmp"))
        );
    }

    #[test]
    fn other_schemes_pass_through_unchanged() {
        assert_eq!(
            parse_target("https://example.com/a?b=c").unwrap(),
            Target::Remote("https://example.com/a?b=c".into())
        );
        assert_eq!(
            parse_target("mailto:me@example.com").unwrap(),
            Target::Remote("mailto:me@example.com".into())
        );
    }

    #[test]
    fn rejects_paths_and_foreign_hosts() {
        for bad in [
            "",
            "/home/me/a.txt",
            "relative/path",
            "file://server/share/a",
        ] {
            assert_eq!(
                parse_target(bad).unwrap_err().kind,
                ServiceErrorKind::InvalidArgument,
                "{bad:?}"
            );
        }
    }
}
