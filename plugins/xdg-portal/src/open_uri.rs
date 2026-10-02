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

    use super::Target;
    use crate::{
        error::{ServiceError, ServiceErrorKind},
        linux::{DESKTOP_DESTINATION, DESKTOP_PATH},
        timeout::with_timeout,
    };

    const INTERFACE: &str = "org.freedesktop.portal.OpenURI";

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
                let file = std::fs::File::open(path).map_err(|error| {
                    let kind = if error.kind() == std::io::ErrorKind::NotFound {
                        ServiceErrorKind::NotFound
                    } else {
                        ServiceErrorKind::Failed
                    };
                    ServiceError::new(kind, format!("{}: {error}", path.display()))
                })?;
                let is_dir = file.metadata().map(|m| m.is_dir()).unwrap_or(false);
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
