// Emits the Unity LauncherEntry Update signal that docks read progress and counts from
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

use std::collections::HashMap;

use zbus::{zvariant::Value, Connection};

use crate::{
    error::{with_timeout, ServiceError},
    launcher::{app_uri, object_path, properties, Property, LAUNCHER_INTERFACE},
    models::LauncherRequest,
};

/// Emits the `Update` signal for the app whose `.desktop` id is `desktop_id`.
///
/// The signal is a broadcast: it succeeds whether or not a dock is listening.
pub async fn update(
    connection: &Connection,
    desktop_id: &str,
    request: &LauncherRequest,
) -> Result<(), ServiceError> {
    let properties = properties(request)?;
    let mut dict: HashMap<&str, Value<'_>> = HashMap::new();
    for (key, value) in properties {
        dict.insert(
            key,
            match value {
                Property::Double(v) => Value::F64(v),
                Property::Bool(v) => Value::Bool(v),
                Property::Int(v) => Value::I64(v),
            },
        );
    }
    let path = object_path(desktop_id);
    let uri = app_uri(desktop_id);
    with_timeout(
        "the launcher entry signal",
        connection.emit_signal(
            None::<&str>,
            path.as_str(),
            LAUNCHER_INTERFACE,
            "Update",
            &(uri.as_str(), &dict),
        ),
    )
    .await
}
