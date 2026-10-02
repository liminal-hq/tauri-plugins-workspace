// Takes sleep and idle inhibitors from systemd-logind
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

use zbus::{zvariant::OwnedFd, Connection, Proxy};

use crate::error::{with_timeout, ServiceError};

/// The well-known name of logind on the system bus.
pub const BUS_NAME: &str = "org.freedesktop.login1";

/// Takes a blocking inhibitor and returns the file descriptor that holds it: logind drops the
/// inhibitor when the descriptor is closed, and also when the process exits.
pub async fn inhibit(
    connection: &Connection,
    what: &str,
    who: &str,
    why: &str,
) -> Result<OwnedFd, ServiceError> {
    let proxy = with_timeout(
        "logind",
        Proxy::new(
            connection,
            BUS_NAME,
            "/org/freedesktop/login1",
            "org.freedesktop.login1.Manager",
        ),
    )
    .await?;
    with_timeout("Inhibit", proxy.call("Inhibit", &(what, who, why, "block"))).await
}
