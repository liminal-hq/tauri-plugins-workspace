// Owns org.freedesktop.FileManager1 and forwards its calls to the app
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

use std::sync::Arc;

use futures_util::StreamExt;
use zbus::{
    self,
    fdo::{DBusProxy, RequestNameFlags, RequestNameReply},
    Connection,
};

use crate::{
    error::{with_timeout, ServiceError, ServiceErrorKind},
    file_manager::{call_from, BUS_NAME, OBJECT_PATH},
    models::{FileManagerCall, FileManagerMethod},
};

/// Receives each call another application makes to the file manager service.
pub type CallSink = Arc<dyn Fn(FileManagerCall) + Send + Sync>;

/// The `org.freedesktop.FileManager1` object. Each method turns its URIs into targets and hands
/// the call to the sink; a call with no usable URI is answered with an `InvalidArgs` error.
pub struct FileManager1 {
    sink: CallSink,
}

impl FileManager1 {
    pub fn new(sink: CallSink) -> Self {
        Self { sink }
    }

    fn forward(
        &self,
        method: FileManagerMethod,
        uris: &[String],
        startup_id: &str,
    ) -> zbus::fdo::Result<()> {
        let call = call_from(method, uris, startup_id)
            .map_err(|error| zbus::fdo::Error::InvalidArgs(error.message))?;
        (self.sink)(call);
        Ok(())
    }
}

#[zbus::interface(name = "org.freedesktop.FileManager1")]
impl FileManager1 {
    async fn show_folders(&self, uris: Vec<String>, startup_id: String) -> zbus::fdo::Result<()> {
        self.forward(FileManagerMethod::ShowFolders, &uris, &startup_id)
    }

    async fn show_items(&self, uris: Vec<String>, startup_id: String) -> zbus::fdo::Result<()> {
        self.forward(FileManagerMethod::ShowItems, &uris, &startup_id)
    }

    async fn show_item_properties(
        &self,
        uris: Vec<String>,
        startup_id: String,
    ) -> zbus::fdo::Result<()> {
        self.forward(FileManagerMethod::ShowItemProperties, &uris, &startup_id)
    }
}

/// The name held on a connection of its own, and the task watching for it to be taken away.
pub struct Owned {
    connection: Connection,
    watcher: tauri::async_runtime::JoinHandle<()>,
}

/// Takes `org.freedesktop.FileManager1` and serves it. `on_lost` runs when another process takes
/// the name over; the name is requested so that it may.
pub async fn own(
    sink: CallSink,
    on_lost: Arc<dyn Fn(String) + Send + Sync>,
) -> Result<Owned, ServiceError> {
    let builder = zbus::connection::Builder::session()
        .and_then(|builder| builder.serve_at(OBJECT_PATH, FileManager1::new(sink)))
        .map_err(|error| ServiceError::from_message(error.to_string()))?;
    let connection = with_timeout("the session bus", builder.build()).await?;

    // The loss stream must exist before the name is requested, or a loss right after could be
    // missed.
    let bus = with_timeout("the bus daemon", DBusProxy::new(&connection)).await?;
    let mut lost = with_timeout("NameLost", bus.receive_name_lost()).await?;

    // Without `DoNotQueue` a name held by another process would queue this one behind it and look
    // like success; `AllowReplacement` lets a file manager that insists on the name take it.
    let reply = with_timeout(
        "RequestName",
        connection.request_name_with_flags(
            BUS_NAME,
            RequestNameFlags::AllowReplacement | RequestNameFlags::DoNotQueue,
        ),
    )
    .await?;
    if !matches!(
        reply,
        RequestNameReply::PrimaryOwner | RequestNameReply::AlreadyOwner
    ) {
        return Err(ServiceError::new(
            ServiceErrorKind::Conflict,
            format!("another file manager owns {BUS_NAME}"),
        ));
    }

    let watcher = tauri::async_runtime::spawn(async move {
        let _bus = bus;
        while let Some(signal) = lost.next().await {
            if let Ok(args) = signal.args() {
                if args.name.as_str() == BUS_NAME {
                    on_lost(format!("{BUS_NAME} was taken over by another process"));
                    break;
                }
            }
        }
    });
    Ok(Owned {
        connection,
        watcher,
    })
}

impl Owned {
    /// Gives the name back and stops serving.
    pub async fn release(self) {
        self.watcher.abort();
        let _ = with_timeout("ReleaseName", self.connection.release_name(BUS_NAME)).await;
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Mutex;

    use super::*;

    fn object() -> (FileManager1, Arc<Mutex<Vec<FileManagerCall>>>) {
        let calls = Arc::new(Mutex::new(Vec::new()));
        let sink_calls = Arc::clone(&calls);
        let sink: CallSink = Arc::new(move |call| sink_calls.lock().unwrap().push(call));
        (FileManager1::new(sink), calls)
    }

    #[tokio::test]
    async fn each_method_forwards_its_call() {
        let (object, calls) = object();
        let uris = vec!["file:///tmp/a%20b".to_string()];
        object
            .show_folders(uris.clone(), "s1".into())
            .await
            .unwrap();
        object.show_items(uris.clone(), "s2".into()).await.unwrap();
        object
            .show_item_properties(uris, "s3".into())
            .await
            .unwrap();

        let calls = calls.lock().unwrap();
        let methods: Vec<_> = calls.iter().map(|c| c.method).collect();
        assert_eq!(
            methods,
            vec![
                FileManagerMethod::ShowFolders,
                FileManagerMethod::ShowItems,
                FileManagerMethod::ShowItemProperties
            ]
        );
        assert_eq!(calls[1].startup_id, "s2");
        assert_eq!(calls[0].targets[0].path.as_deref(), Some("/tmp/a b"));
    }

    #[tokio::test]
    async fn a_call_without_a_usable_uri_is_refused_and_not_forwarded() {
        let (object, calls) = object();
        let error = object
            .show_items(vec!["nonsense".into()], String::new())
            .await;
        assert!(matches!(error, Err(zbus::fdo::Error::InvalidArgs(_))));
        assert!(calls.lock().unwrap().is_empty());
    }
}
