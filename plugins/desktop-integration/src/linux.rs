// Reaches the session and system buses for the Linux implementations of the services
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

pub mod file_manager;
pub mod launcher;
pub mod logind;
pub mod notify;

use zbus::{fdo::DBusProxy, Connection};

use crate::error::{with_timeout, ServiceError, ServiceErrorKind};

/// Connects to the session bus.
pub async fn session() -> Result<Connection, ServiceError> {
    connect("the session bus", Connection::session()).await
}

/// Connects to the system bus.
pub async fn system() -> Result<Connection, ServiceError> {
    connect("the system bus", Connection::system()).await
}

async fn connect(
    what: &str,
    connecting: impl std::future::Future<Output = zbus::Result<Connection>>,
) -> Result<Connection, ServiceError> {
    with_timeout(what, connecting).await.map_err(|error| {
        // Not reaching a bus at all is "unavailable" whatever the transport said.
        ServiceError::new(ServiceErrorKind::Unavailable, error.message)
    })
}

/// Whether some process owns the well-known bus name `name` on `connection`.
pub async fn has_owner(connection: &Connection, name: &'static str) -> Result<bool, ServiceError> {
    let bus = with_timeout("the bus daemon", DBusProxy::new(connection)).await?;
    let name = zbus::names::BusName::try_from(name)
        .map_err(|error| ServiceError::invalid(error.to_string()))?;
    with_timeout("NameHasOwner", bus.name_has_owner(name)).await
}
