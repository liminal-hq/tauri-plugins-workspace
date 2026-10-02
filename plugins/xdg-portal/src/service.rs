// The managed portal service behind the notification, inhibit and open-URI commands
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

use tauri::{AppHandle, Manager, Runtime};
use tokio::sync::Mutex;

use crate::{
    error::ServiceError,
    handles::HandleBook,
    inhibit::Inhibitor,
    models::{InhibitHandle, InhibitRequest, NotificationRequest, OpenUriRequest, PortalStatus},
    notification, open_uri, status,
};

/// Rust access to the plugin's portal features: `app.portal()`.
///
/// Every method is the same call the matching command makes, so Rust code and the webview see one
/// behaviour.
pub trait PortalExt<R: Runtime> {
    fn portal(&self) -> &Portal<R>;
}

impl<R: Runtime, T: Manager<R>> PortalExt<R> for T {
    fn portal(&self) -> &Portal<R> {
        self.state::<Portal<R>>().inner()
    }
}

/// Holds the session bus connection, the action listener and the live inhibitors.
pub struct Portal<R: Runtime> {
    #[cfg_attr(not(target_os = "linux"), allow(dead_code))]
    app: AppHandle<R>,
    #[cfg(target_os = "linux")]
    connection: tokio::sync::OnceCell<ashpd::zbus::Connection>,
    #[cfg(target_os = "linux")]
    listener: Mutex<Option<tauri::async_runtime::JoinHandle<()>>>,
    inhibitors: Mutex<HandleBook<Inhibitor>>,
}

impl<R: Runtime> Portal<R> {
    pub(crate) fn new(app: AppHandle<R>) -> Self {
        Self {
            app,
            #[cfg(target_os = "linux")]
            connection: tokio::sync::OnceCell::new(),
            #[cfg(target_os = "linux")]
            listener: Mutex::new(None),
            inhibitors: Mutex::new(HandleBook::default()),
        }
    }

    /// Which portal features work on this session, and why the others do not.
    pub async fn status(&self) -> PortalStatus {
        #[cfg(target_os = "linux")]
        {
            let connection = self.connection().await.ok();
            status::probe(connection).await
        }
        #[cfg(not(target_os = "linux"))]
        {
            status::unsupported_status()
        }
    }

    /// Shows a notification, or replaces the one with the same id.
    pub async fn send_notification(
        &self,
        request: NotificationRequest,
    ) -> Result<(), ServiceError> {
        request.validate()?;
        #[cfg(target_os = "linux")]
        {
            use ashpd::desktop::notification::NotificationProxy;

            use crate::timeout::with_timeout;

            let connection = self.connection().await?.clone();
            self.ensure_action_listener(&connection).await?;
            let proxy = with_timeout(
                "the notification portal",
                NotificationProxy::with_connection(connection),
            )
            .await?;
            let notification = notification::build_notification(&request);
            with_timeout(
                "AddNotification",
                proxy.add_notification(&request.id, notification),
            )
            .await
        }
        #[cfg(not(target_os = "linux"))]
        {
            let _ = request;
            Err(ServiceError::unsupported())
        }
    }

    /// Takes a notification off screen. Withdrawing an id that is not shown is not an error.
    pub async fn withdraw_notification(&self, id: String) -> Result<(), ServiceError> {
        notification::validate_id(&id)?;
        #[cfg(target_os = "linux")]
        {
            use ashpd::desktop::notification::NotificationProxy;

            use crate::timeout::with_timeout;

            let connection = self.connection().await?.clone();
            let proxy = with_timeout(
                "the notification portal",
                NotificationProxy::with_connection(connection),
            )
            .await?;
            with_timeout("RemoveNotification", proxy.remove_notification(&id)).await
        }
        #[cfg(not(target_os = "linux"))]
        {
            Err(ServiceError::unsupported())
        }
    }

    /// Keeps the session from idling or suspending until the handle is released.
    pub async fn inhibit(&self, request: InhibitRequest) -> Result<InhibitHandle, ServiceError> {
        request.validate()?;
        #[cfg(target_os = "linux")]
        {
            let connection = self.connection().await?;
            let flags = crate::inhibit::flags_for(&request.kinds);
            let inhibitor = crate::inhibit::acquire(connection, flags, &request.reason).await?;
            let handle = self.inhibitors.lock().await.insert(inhibitor);
            Ok(InhibitHandle { handle })
        }
        #[cfg(not(target_os = "linux"))]
        {
            Err(ServiceError::unsupported())
        }
    }

    /// Ends an inhibitor. An unknown or already released handle is a `not-found` error.
    pub async fn release_inhibit(&self, handle: u32) -> Result<(), ServiceError> {
        let not_found = || {
            ServiceError::new(
                crate::error::ServiceErrorKind::NotFound,
                format!("no inhibitor with handle {handle}"),
            )
        };
        #[cfg(target_os = "linux")]
        {
            // The inhibitor stays in the book if `Close` fails, so it can be released again.
            crate::handles::release_with(&self.inhibitors, handle, |inhibitor| async move {
                let connection = match self.connection().await {
                    Ok(connection) => connection,
                    Err(error) => return Err((inhibitor, error)),
                };
                crate::inhibit::release(connection, &inhibitor)
                    .await
                    .map_err(|error| (inhibitor, error))
            })
            .await
            .unwrap_or_else(|| Err(not_found()))
        }
        #[cfg(not(target_os = "linux"))]
        {
            match self.inhibitors.lock().await.remove(handle) {
                Some(inhibitor) => match inhibitor {},
                None => Err(not_found()),
            }
        }
    }

    /// Opens a URI, file or folder with the user's chosen application.
    pub async fn open_uri(&self, request: OpenUriRequest) -> Result<(), ServiceError> {
        let target = open_uri::parse_target(&request.uri)?;
        #[cfg(target_os = "linux")]
        {
            let connection = self.connection().await?;
            open_uri::open(connection, &target, request.ask, request.writable).await
        }
        #[cfg(not(target_os = "linux"))]
        {
            let _ = target;
            Err(ServiceError::unsupported())
        }
    }

    /// Releases every live inhibitor; called when the app exits.
    pub(crate) async fn release_all(&self) {
        let inhibitors = self.inhibitors.lock().await.drain();
        #[cfg(target_os = "linux")]
        if let Some(connection) = self.connection.get() {
            for inhibitor in inhibitors {
                if let Err(error) = crate::inhibit::release(connection, &inhibitor).await {
                    log::warn!("could not release an inhibitor on exit: {error}");
                }
            }
        }
        #[cfg(not(target_os = "linux"))]
        drop(inhibitors);
    }

    #[cfg(target_os = "linux")]
    async fn connection(&self) -> Result<&ashpd::zbus::Connection, ServiceError> {
        self.connection
            .get_or_try_init(|| async {
                crate::timeout::with_timeout("the session bus", ashpd::zbus::Connection::session())
                    .await
                    .map_err(|error| {
                        // Not being able to reach the bus at all means there is no portal.
                        ServiceError::new(
                            crate::error::ServiceErrorKind::PortalUnavailable,
                            error.message,
                        )
                    })
            })
            .await
    }

    #[cfg(target_os = "linux")]
    async fn ensure_action_listener(
        &self,
        connection: &ashpd::zbus::Connection,
    ) -> Result<(), ServiceError> {
        let mut listener = self.listener.lock().await;
        if listener.is_none() {
            *listener =
                Some(notification::listen_for_actions(self.app.clone(), connection.clone()).await?);
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::error::ServiceErrorKind;
    use crate::models::InhibitKind;

    fn portal() -> Portal<tauri::test::MockRuntime> {
        let app = tauri::test::mock_app();
        Portal::new(app.handle().clone())
    }

    #[tokio::test]
    async fn releasing_an_unknown_handle_is_not_found() {
        let error = portal().release_inhibit(42).await.unwrap_err();
        assert_eq!(error.kind, ServiceErrorKind::NotFound);
    }

    #[tokio::test]
    async fn invalid_requests_fail_before_any_portal_call() {
        let portal = portal();
        let notification = NotificationRequest {
            id: String::new(),
            title: "x".into(),
            body: None,
            default_action: None,
            urgency: None,
        };
        assert_eq!(
            portal
                .send_notification(notification)
                .await
                .unwrap_err()
                .kind,
            ServiceErrorKind::InvalidArgument
        );
        assert_eq!(
            portal
                .inhibit(InhibitRequest {
                    reason: String::new(),
                    kinds: vec![InhibitKind::Idle],
                })
                .await
                .unwrap_err()
                .kind,
            ServiceErrorKind::InvalidArgument
        );
        assert_eq!(
            portal
                .open_uri(OpenUriRequest {
                    uri: "not a uri".into(),
                    ask: None,
                    writable: None,
                })
                .await
                .unwrap_err()
                .kind,
            ServiceErrorKind::InvalidArgument
        );
    }

    /// Reads which portal interfaces answer on this session. Read-only.
    #[tokio::test]
    #[ignore = "needs a session bus with xdg-desktop-portal"]
    async fn live_status() {
        let status = portal().status().await;
        println!("{status:#?}");
        assert_eq!(status.features.len(), 3);
    }

    /// Takes an idle and suspend inhibitor through the portal, holds it for a few seconds so it
    /// can be inspected, then releases it.
    #[tokio::test(flavor = "multi_thread")]
    #[ignore = "needs a session bus with xdg-desktop-portal"]
    async fn live_inhibit() {
        let portal = portal();
        let handle = portal
            .inhibit(InhibitRequest {
                reason: "tauri-plugin-xdg-portal live_inhibit test".into(),
                kinds: vec![InhibitKind::Idle, InhibitKind::Suspend],
            })
            .await
            .expect("inhibit");
        println!("holding inhibitor {handle:?}");
        tokio::time::sleep(std::time::Duration::from_secs(4)).await;
        portal
            .release_inhibit(handle.handle)
            .await
            .expect("release");
        assert_eq!(
            portal
                .release_inhibit(handle.handle)
                .await
                .unwrap_err()
                .kind,
            ServiceErrorKind::NotFound
        );
    }

    /// Shows one test notification and withdraws it. Sends a real notification.
    #[tokio::test(flavor = "multi_thread")]
    #[ignore = "shows a real notification"]
    async fn live_notification() {
        let portal = portal();
        portal
            .send_notification(NotificationRequest {
                id: "live-notification".into(),
                title: "tauri-plugin-xdg-portal test".into(),
                body: Some("A single test notification from the live_notification test.".into()),
                default_action: Some("open".into()),
                urgency: None,
            })
            .await
            .expect("send");
        tokio::time::sleep(std::time::Duration::from_secs(4)).await;
        portal
            .withdraw_notification("live-notification".into())
            .await
            .expect("withdraw");
    }
}
