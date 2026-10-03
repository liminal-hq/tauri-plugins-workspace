// Shows and closes notifications through org.freedesktop.Notifications
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

use std::{
    collections::HashMap,
    sync::{Arc, Mutex},
};

use futures_util::StreamExt;
use tauri::{AppHandle, Emitter, Runtime};
use zbus::{zvariant::Value, Connection, Proxy};

use crate::{
    error::{with_timeout, ServiceError},
    notify::{IdBook, NotifyCall, ACTION_EVENT},
};

const DESTINATION: &str = "org.freedesktop.Notifications";
const PATH: &str = "/org/freedesktop/Notifications";
const INTERFACE: &str = "org.freedesktop.Notifications";

async fn proxy(connection: &Connection) -> Result<Proxy<'static>, ServiceError> {
    with_timeout(
        "the notification server",
        Proxy::new(connection, DESTINATION, PATH, INTERFACE),
    )
    .await
}

/// The name of the notification server, which also proves one is running.
pub async fn server_name(connection: &Connection) -> Result<String, ServiceError> {
    let proxy = proxy(connection).await?;
    let (name, _vendor, _version, _spec): (String, String, String, String) = with_timeout(
        "GetServerInformation",
        proxy.call("GetServerInformation", &()),
    )
    .await?;
    Ok(name)
}

/// The capabilities the server lists, such as `actions` and `body`.
pub async fn capabilities(connection: &Connection) -> Result<Vec<String>, ServiceError> {
    let proxy = proxy(connection).await?;
    with_timeout("GetCapabilities", proxy.call("GetCapabilities", &())).await
}

/// Shows the notification and returns the server's id for it.
pub async fn notify(connection: &Connection, call: &NotifyCall) -> Result<u32, ServiceError> {
    let proxy = proxy(connection).await?;
    let mut hints: HashMap<&str, Value<'_>> = HashMap::new();
    hints.insert("urgency", Value::U8(call.urgency));
    // An empty entry would only mislead the shell's app matching.
    if !call.desktop_entry.is_empty() {
        hints.insert("desktop-entry", Value::from(call.desktop_entry.as_str()));
    }
    let body = (
        call.app_name.as_str(),
        call.replaces_id,
        "",
        call.summary.as_str(),
        call.body.as_str(),
        &call.actions,
        &hints,
        call.expire_timeout,
    );
    with_timeout("Notify", proxy.call("Notify", &body)).await
}

/// Closes the notification the server knows as `server_id`.
pub async fn close(connection: &Connection, server_id: u32) -> Result<(), ServiceError> {
    let proxy = proxy(connection).await?;
    with_timeout(
        "CloseNotification",
        proxy.call::<_, _, ()>("CloseNotification", &(server_id,)),
    )
    .await
}

/// Forwards `ActionInvoked` to the `notification-action` event and forgets notifications the
/// server closes. Resolves once both subscriptions exist.
pub async fn listen<R: Runtime>(
    app: AppHandle<R>,
    connection: Connection,
    book: Arc<Mutex<IdBook>>,
) -> Result<tauri::async_runtime::JoinHandle<()>, ServiceError> {
    let (ready_tx, ready_rx) = tokio::sync::oneshot::channel();
    let handle = tauri::async_runtime::spawn(async move {
        let setup = async {
            let proxy = proxy(&connection).await?;
            let actions =
                with_timeout("ActionInvoked", proxy.receive_signal("ActionInvoked")).await?;
            let closed = with_timeout(
                "NotificationClosed",
                proxy.receive_signal("NotificationClosed"),
            )
            .await?;
            Ok::<_, ServiceError>((proxy, actions, closed))
        }
        .await;
        let (_proxy, mut actions, mut closed) = match setup {
            Ok(parts) => {
                let _ = ready_tx.send(Ok(()));
                parts
            }
            Err(error) => {
                let _ = ready_tx.send(Err(error));
                return;
            }
        };
        loop {
            tokio::select! {
                Some(message) = actions.next() => {
                    let Ok((server_id, key)) = message.body().deserialize::<(u32, String)>() else {
                        continue;
                    };
                    let action = book.lock().ok().and_then(|book| book.action_for(server_id, &key));
                    if let Some(action) = action {
                        if let Err(error) = app.emit(ACTION_EVENT, action) {
                            log::warn!("could not emit a notification action: {error}");
                        }
                    }
                }
                Some(message) = closed.next() => {
                    if let Ok((server_id, _reason)) = message.body().deserialize::<(u32, u32)>() {
                        if let Ok(mut book) = book.lock() {
                            book.forget_server(server_id);
                        }
                    }
                }
                else => break,
            }
        }
    });
    match ready_rx.await {
        Ok(Ok(())) => Ok(handle),
        Ok(Err(error)) => Err(error),
        Err(_) => Err(ServiceError::from_message("the action listener stopped")),
    }
}
