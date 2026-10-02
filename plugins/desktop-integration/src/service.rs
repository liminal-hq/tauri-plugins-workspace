// The managed service behind the notify, sleep, launcher, file manager and shortcut commands
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

#[cfg(target_os = "windows")]
use crate::error::ServiceErrorKind;
use tauri::{AppHandle, Manager, Runtime};
use tauri_plugin_xdg_portal::handles::HandleBook;
use tokio::sync::Mutex;

use crate::{
    error::ServiceError,
    models::{
        FileManagerOwnership, GlobalShortcutRequest, LauncherRequest, NotifyRequest, PluginStatus,
        SleepInhibitHandle, SleepInhibitRequest,
    },
    notify, sleep, status,
};

/// Rust access to the plugin's services: `app.desktop_services()`.
///
/// Every method is the call the matching command makes, so Rust code and the webview see one
/// behaviour.
pub trait DesktopServicesExt<R: Runtime> {
    fn desktop_services(&self) -> &DesktopServices<R>;
}

impl<R: Runtime, T: Manager<R>> DesktopServicesExt<R> for T {
    fn desktop_services(&self) -> &DesktopServices<R> {
        self.state::<DesktopServices<R>>().inner()
    }
}

#[cfg(target_os = "linux")]
#[derive(Default)]
struct LinuxState {
    session: tokio::sync::OnceCell<zbus::Connection>,
    system: tokio::sync::OnceCell<zbus::Connection>,
    notifications: std::sync::Arc<std::sync::Mutex<notify::IdBook>>,
    listener: Mutex<Option<tauri::async_runtime::JoinHandle<()>>>,
    file_manager: Mutex<crate::file_manager::OwnerSlot<crate::linux::file_manager::Owned>>,
}

#[cfg(target_os = "windows")]
#[derive(Default)]
struct WindowsState {
    toasts: crate::win::toast::Shown,
    /// Held only to clone or swap the `Arc`, never across a call into the thread, so one slow
    /// hotkey command cannot block the others.
    hotkeys:
        std::sync::Arc<std::sync::Mutex<Option<std::sync::Arc<crate::win::hotkeys::HotkeyThread>>>>,
}

pub struct DesktopServices<R: Runtime> {
    #[cfg_attr(not(any(target_os = "linux", target_os = "windows")), allow(dead_code))]
    app: AppHandle<R>,
    sleep: Mutex<HandleBook<sleep::Inhibitor>>,
    #[cfg(target_os = "linux")]
    linux: LinuxState,
    #[cfg(target_os = "windows")]
    windows: WindowsState,
}

impl<R: Runtime> DesktopServices<R> {
    pub(crate) fn new(app: AppHandle<R>) -> Self {
        Self {
            app,
            sleep: Mutex::new(HandleBook::default()),
            #[cfg(target_os = "linux")]
            linux: LinuxState::default(),
            #[cfg(target_os = "windows")]
            windows: WindowsState::default(),
        }
    }

    #[cfg(target_os = "linux")]
    fn app_name(&self) -> String {
        self.app.package_info().name.clone()
    }

    #[cfg(target_os = "linux")]
    fn default_desktop_id(&self) -> String {
        self.app.config().identifier.clone()
    }

    /// Which services work on this system, and why the others do not.
    pub async fn status(&self) -> PluginStatus {
        #[cfg(target_os = "linux")]
        {
            let session = self.session().await;
            let notify_server = match &session {
                Ok(connection) => crate::linux::notify::server_name(connection)
                    .await
                    .map_err(|error| error.message),
                Err(error) => Err(error.message.clone()),
            };
            let logind = match self.system().await {
                Ok(connection) => {
                    match crate::linux::has_owner(connection, crate::linux::logind::BUS_NAME).await
                    {
                        Ok(true) => Ok(()),
                        Ok(false) => Err("org.freedesktop.login1 has no owner".to_string()),
                        Err(error) => Err(error.message),
                    }
                }
                Err(error) => Err(error.message),
            };
            let probes = status::LinuxProbes {
                session_bus: session.map(|_| ()).map_err(|error| error.message),
                notify_server,
                logind,
                shortcut_path: if std::env::var_os("WAYLAND_DISPLAY").is_some() {
                    Some("wayland-portal")
                } else if std::env::var_os("DISPLAY").is_some() {
                    Some("x11-grab")
                } else {
                    None
                },
            };
            let owned = self.linux.file_manager.lock().await.is_owned();
            status::linux_status(&probes, owned)
        }
        #[cfg(target_os = "windows")]
        {
            status::windows_status(crate::win::toast::app_user_model_id().as_deref())
        }
        #[cfg(not(any(target_os = "linux", target_os = "windows")))]
        {
            status::unsupported_status()
        }
    }

    /// Shows a notification, or replaces the one with the same id.
    pub async fn notify(&self, request: NotifyRequest) -> Result<(), ServiceError> {
        request.validate()?;
        #[cfg(target_os = "linux")]
        {
            let connection = self.session().await?.clone();
            self.ensure_listener(&connection).await?;
            let replaces = self
                .linux
                .notifications
                .lock()
                .ok()
                .and_then(|book| book.server_id(&request.id))
                .unwrap_or(0);
            let call = notify::build_call(
                &request,
                &self.app_name(),
                &self.default_desktop_id(),
                replaces,
            );
            let server_id = crate::linux::notify::notify(&connection, &call).await?;
            if let Ok(mut book) = self.linux.notifications.lock() {
                book.record(&request.id, server_id, request.default_action.as_deref());
            }
            Ok(())
        }
        #[cfg(target_os = "windows")]
        {
            use tauri::Emitter;

            let app = self.app.clone();
            let request = request.clone();
            let shown = &self.windows.toasts;
            crate::win::toast::show(shown, &request, move |action| {
                let _ = app.emit(notify::ACTION_EVENT, action);
            })
        }
        #[cfg(not(any(target_os = "linux", target_os = "windows")))]
        {
            Err(ServiceError::unsupported("notifications"))
        }
    }

    /// Takes a notification off screen. Withdrawing an id that is not shown is not an error.
    pub async fn withdraw_notification(&self, id: String) -> Result<(), ServiceError> {
        notify::validate_id(&id)?;
        #[cfg(target_os = "linux")]
        {
            let server_id = self
                .linux
                .notifications
                .lock()
                .ok()
                .and_then(|mut book| book.forget(&id));
            match server_id {
                Some(server_id) => {
                    let connection = self.session().await?;
                    crate::linux::notify::close(connection, server_id).await
                }
                None => Ok(()),
            }
        }
        #[cfg(target_os = "windows")]
        {
            crate::win::toast::withdraw(&self.windows.toasts, &id)
        }
        #[cfg(not(any(target_os = "linux", target_os = "windows")))]
        {
            Err(ServiceError::unsupported("notifications"))
        }
    }

    /// Keeps the machine awake until the handle is released.
    pub async fn inhibit_sleep(
        &self,
        request: SleepInhibitRequest,
    ) -> Result<SleepInhibitHandle, ServiceError> {
        request.validate()?;
        #[cfg(target_os = "linux")]
        {
            let connection = self.system().await?;
            let inhibitor = crate::linux::logind::inhibit(
                connection,
                sleep::logind_what(&request.kinds),
                &self.app_name(),
                &request.reason,
            )
            .await?;
            let handle = self.sleep.lock().await.insert(inhibitor);
            Ok(SleepInhibitHandle { handle })
        }
        #[cfg(target_os = "windows")]
        {
            let inhibitor = crate::win::power::PowerRequest::new(&request.reason)?;
            let handle = self.sleep.lock().await.insert(inhibitor);
            Ok(SleepInhibitHandle { handle })
        }
        #[cfg(not(any(target_os = "linux", target_os = "windows")))]
        {
            Err(ServiceError::unsupported("sleep inhibit"))
        }
    }

    /// Ends a sleep inhibitor. An unknown or already released handle is a `not-found` error.
    pub async fn release_sleep_inhibit(&self, handle: u32) -> Result<(), ServiceError> {
        // Dropping the removed inhibitor closes logind's descriptor or clears the power request.
        let removed = self.sleep.lock().await.remove(handle);
        match removed {
            Some(_) => Ok(()),
            None => Err(ServiceError::not_found(format!(
                "no sleep inhibitor with handle {handle}"
            ))),
        }
    }

    /// Shows progress and a count on the app's dock or taskbar entry.
    pub async fn set_launcher_progress(
        &self,
        request: LauncherRequest,
    ) -> Result<(), ServiceError> {
        // Validate on every platform, so a bad request fails the same way everywhere.
        crate::launcher::properties(&request)?;
        #[cfg(target_os = "linux")]
        {
            let connection = self.session().await?;
            let desktop_id = request
                .desktop_id
                .clone()
                .unwrap_or_else(|| self.default_desktop_id());
            crate::linux::launcher::update(connection, &desktop_id, &request).await
        }
        #[cfg(target_os = "windows")]
        {
            let plan = crate::launcher::taskbar_plan(&request.progress)?;
            let window = match &request.window_label {
                Some(label) => self.app.get_webview_window(label),
                None => {
                    let windows = self.app.webview_windows();
                    let candidates: Vec<(&str, bool)> = windows
                        .iter()
                        .map(|(label, window)| {
                            (label.as_str(), window.is_focused().unwrap_or(false))
                        })
                        .collect();
                    crate::launcher::default_window(&candidates)
                        .and_then(|label| windows.get(label).cloned())
                }
            }
            .ok_or_else(|| ServiceError::not_found("no such window"))?;
            let hwnd = window
                .hwnd()
                .map_err(|error| ServiceError::new(ServiceErrorKind::Failed, error.to_string()))?;
            // A window handle is a raw pointer and so not `Send`; the number crosses threads.
            let hwnd = hwnd.0 as isize;
            let (tx, rx) = std::sync::mpsc::channel();
            window
                .run_on_main_thread(move || {
                    let hwnd = windows::Win32::Foundation::HWND(hwnd as *mut _);
                    let _ = tx.send(crate::win::taskbar::apply(hwnd, plan));
                })
                .map_err(|error| ServiceError::new(ServiceErrorKind::Failed, error.to_string()))?;
            tokio::task::spawn_blocking(move || rx.recv_timeout(crate::error::CALL_TIMEOUT))
                .await
                .map_err(|error| ServiceError::new(ServiceErrorKind::Failed, error.to_string()))?
                .map_err(|_| ServiceError::timeout("the main thread"))?
        }
        #[cfg(not(any(target_os = "linux", target_os = "windows")))]
        {
            Err(ServiceError::unsupported("launcher progress"))
        }
    }

    /// Takes `org.freedesktop.FileManager1` and forwards its calls as the
    /// `desktop-integration://file-manager` event. Owning it twice is not an error.
    pub async fn own_file_manager(&self) -> Result<FileManagerOwnership, ServiceError> {
        #[cfg(target_os = "linux")]
        {
            use std::sync::Arc;

            use tauri::Emitter;

            let mut slot = self.linux.file_manager.lock().await;
            if slot.is_owned() {
                return Ok(FileManagerOwnership {
                    owned: true,
                    reason: None,
                });
            }
            let call_app = self.app.clone();
            let sink: crate::linux::file_manager::CallSink = Arc::new(move |call| {
                if let Err(error) = call_app.emit(crate::file_manager::CALL_EVENT, call) {
                    log::warn!("could not emit a FileManager1 call: {error}");
                }
            });
            let generation = slot.next_generation();
            let lost_app = self.app.clone();
            let on_lost: Arc<dyn Fn(String) + Send + Sync> = Arc::new(move |reason| {
                let app = lost_app.clone();
                tauri::async_runtime::spawn(async move {
                    let services = app.desktop_services();
                    // Only the ownership that lost the name: if the name was given back and taken
                    // again since the signal, the new ownership is healthy and stays.
                    let owned = services
                        .linux
                        .file_manager
                        .lock()
                        .await
                        .take_generation(generation);
                    if let Some(owned) = owned {
                        owned.release().await;
                        let _ = app.emit(
                            crate::file_manager::OWNERSHIP_EVENT,
                            FileManagerOwnership {
                                owned: false,
                                reason: Some(reason),
                            },
                        );
                    }
                });
            });
            let owned = crate::linux::file_manager::own(sink, on_lost).await?;
            slot.set(generation, owned);
            Ok(FileManagerOwnership {
                owned: true,
                reason: None,
            })
        }
        #[cfg(not(target_os = "linux"))]
        {
            Err(ServiceError::unsupported("org.freedesktop.FileManager1"))
        }
    }

    /// Gives `org.freedesktop.FileManager1` back. Not owning it is not an error.
    pub async fn disown_file_manager(&self) -> Result<FileManagerOwnership, ServiceError> {
        #[cfg(target_os = "linux")]
        {
            let owned = self.linux.file_manager.lock().await.take();
            if let Some(owned) = owned {
                owned.release().await;
            }
            Ok(FileManagerOwnership {
                owned: false,
                reason: None,
            })
        }
        #[cfg(not(target_os = "linux"))]
        {
            Err(ServiceError::unsupported("org.freedesktop.FileManager1"))
        }
    }

    /// Registers a Windows global shortcut; the `desktop-integration://shortcut-pressed` event
    /// reports each press. Registering an id again replaces its accelerator.
    pub async fn register_global_shortcut(
        &self,
        request: GlobalShortcutRequest,
    ) -> Result<(), ServiceError> {
        if request.id.is_empty() {
            return Err(ServiceError::invalid("a shortcut needs an id"));
        }
        let accelerator = crate::shortcuts::parse_accelerator(&request.accelerator)?;
        #[cfg(target_os = "windows")]
        {
            use tauri::Emitter;

            let slot = std::sync::Arc::clone(&self.windows.hotkeys);
            let app = self.app.clone();
            // Starting the thread and the round trip to it block, so they run off the async
            // workers; the slot is released before the round trip.
            tokio::task::spawn_blocking(move || {
                let thread = {
                    let mut slot = lock_hotkeys(&slot)?;
                    match slot.as_ref() {
                        Some(thread) => std::sync::Arc::clone(thread),
                        None => {
                            let sink: crate::win::hotkeys::PressedSink =
                                std::sync::Arc::new(move |id| {
                                    let _ = app.emit(
                                        crate::shortcuts::PRESSED_EVENT,
                                        crate::models::GlobalShortcutPressed { id },
                                    );
                                });
                            let thread = std::sync::Arc::new(
                                crate::win::hotkeys::HotkeyThread::start(sink)?,
                            );
                            *slot = Some(std::sync::Arc::clone(&thread));
                            thread
                        }
                    }
                };
                thread.register(&request.id, accelerator)
            })
            .await
            .map_err(|error| ServiceError::new(ServiceErrorKind::Failed, error.to_string()))?
        }
        #[cfg(not(target_os = "windows"))]
        {
            let _ = accelerator;
            Err(ServiceError::unsupported("Windows global shortcuts"))
        }
    }

    /// Removes a Windows global shortcut.
    pub async fn unregister_global_shortcut(&self, id: String) -> Result<(), ServiceError> {
        #[cfg(target_os = "windows")]
        {
            let thread = lock_hotkeys(&self.windows.hotkeys)?.clone();
            match thread {
                Some(thread) => tokio::task::spawn_blocking(move || thread.unregister(&id))
                    .await
                    .map_err(|error| {
                        ServiceError::new(ServiceErrorKind::Failed, error.to_string())
                    })?,
                None => Err(ServiceError::not_found(format!(
                    "no shortcut is registered as {id:?}"
                ))),
            }
        }
        #[cfg(not(target_os = "windows"))]
        {
            let _ = id;
            Err(ServiceError::unsupported("Windows global shortcuts"))
        }
    }

    /// Sets the AppUserModelID of the process, which Windows toasts need.
    pub fn set_app_user_model_id(&self, id: &str) -> Result<(), ServiceError> {
        #[cfg(target_os = "windows")]
        {
            crate::win::toast::set_app_user_model_id(id)
        }
        #[cfg(not(target_os = "windows"))]
        {
            let _ = id;
            Err(ServiceError::unsupported("AppUserModelIDs"))
        }
    }

    /// Releases what outlives a command: sleep inhibitors, the bus name and the hotkey thread.
    pub(crate) async fn shutdown(&self) {
        drop(self.sleep.lock().await.drain());
        #[cfg(target_os = "linux")]
        {
            let owned = self.linux.file_manager.lock().await.take();
            if let Some(owned) = owned {
                owned.release().await;
            }
        }
        #[cfg(target_os = "windows")]
        {
            // Dropping the thread joins it, so that happens off the async workers.
            let thread = lock_hotkeys(&self.windows.hotkeys)
                .ok()
                .and_then(|mut slot| slot.take());
            let _ = tokio::task::spawn_blocking(move || drop(thread)).await;
        }
    }

    #[cfg(target_os = "linux")]
    async fn session(&self) -> Result<&zbus::Connection, ServiceError> {
        self.linux
            .session
            .get_or_try_init(crate::linux::session)
            .await
    }

    #[cfg(target_os = "linux")]
    async fn system(&self) -> Result<&zbus::Connection, ServiceError> {
        self.linux
            .system
            .get_or_try_init(crate::linux::system)
            .await
    }

    #[cfg(target_os = "linux")]
    async fn ensure_listener(&self, connection: &zbus::Connection) -> Result<(), ServiceError> {
        // A listener whose signal streams ended is started again here by the next notification.
        tauri_plugin_xdg_portal::task_slot::ensure_running(&self.linux.listener, || {
            crate::linux::notify::listen(
                self.app.clone(),
                connection.clone(),
                std::sync::Arc::clone(&self.linux.notifications),
            )
        })
        .await
    }
}

/// Locks the hotkey slot, which is only ever held to clone or replace its `Arc`.
#[cfg(target_os = "windows")]
fn lock_hotkeys(
    slot: &std::sync::Mutex<Option<std::sync::Arc<crate::win::hotkeys::HotkeyThread>>>,
) -> Result<
    std::sync::MutexGuard<'_, Option<std::sync::Arc<crate::win::hotkeys::HotkeyThread>>>,
    ServiceError,
> {
    slot.lock()
        .map_err(|_| ServiceError::new(ServiceErrorKind::Failed, "hotkey state poisoned"))
}

#[cfg(test)]
mod tests {
    #[cfg(target_os = "linux")]
    use std::time::Duration;

    use super::*;
    use crate::{
        error::ServiceErrorKind,
        models::{LauncherProgress, SleepKind},
    };

    fn services() -> (
        tauri::App<tauri::test::MockRuntime>,
        DesktopServices<tauri::test::MockRuntime>,
    ) {
        let app = tauri::test::mock_app();
        let services = DesktopServices::new(app.handle().clone());
        (app, services)
    }

    fn launcher(progress: LauncherProgress) -> LauncherRequest {
        LauncherRequest {
            progress,
            count: None,
            desktop_id: Some("ca.liminalhq.waypoint.test".into()),
            window_label: None,
        }
    }

    #[tokio::test]
    async fn releasing_an_unknown_handle_is_not_found() {
        let (_app, services) = services();
        let error = services.release_sleep_inhibit(9).await.unwrap_err();
        assert_eq!(error.kind, ServiceErrorKind::NotFound);
    }

    #[tokio::test]
    async fn invalid_requests_fail_before_any_service_call() {
        let (_app, services) = services();
        let invalid = |error: ServiceError| error.kind == ServiceErrorKind::InvalidArgument;
        assert!(invalid(
            services
                .notify(NotifyRequest {
                    id: String::new(),
                    title: "x".into(),
                    body: None,
                    default_action: None,
                    urgency: None,
                    app_name: None,
                    desktop_id: None,
                })
                .await
                .unwrap_err()
        ));
        assert!(invalid(
            services
                .inhibit_sleep(SleepInhibitRequest {
                    reason: String::new(),
                    kinds: vec![SleepKind::Sleep],
                })
                .await
                .unwrap_err()
        ));
        assert!(invalid(
            services
                .set_launcher_progress(launcher(LauncherProgress::Value { value: 2.0 }))
                .await
                .unwrap_err()
        ));
        assert!(invalid(
            services
                .register_global_shortcut(GlobalShortcutRequest {
                    id: "toggle".into(),
                    accelerator: "K".into(),
                })
                .await
                .unwrap_err()
        ));
    }

    #[cfg(not(target_os = "windows"))]
    #[tokio::test]
    async fn windows_only_commands_report_an_unsupported_platform() {
        let (_app, services) = services();
        let error = services
            .register_global_shortcut(GlobalShortcutRequest {
                id: "toggle".into(),
                accelerator: "Ctrl+Alt+K".into(),
            })
            .await
            .unwrap_err();
        assert_eq!(error.kind, ServiceErrorKind::UnsupportedPlatform);
        assert_eq!(
            services.set_app_user_model_id("x").unwrap_err().kind,
            ServiceErrorKind::UnsupportedPlatform
        );
    }

    /// Probes the session. Read-only.
    #[cfg(target_os = "linux")]
    #[tokio::test]
    #[ignore = "needs a session and system bus"]
    async fn live_status() {
        let (_app, services) = services();
        let status = services.status().await;
        println!("{status:#?}");
        assert_eq!(status.features.len(), 5);
    }

    /// Takes a sleep inhibitor from logind and holds it for 8 seconds so
    /// `systemd-inhibit --list` can show it, then releases it.
    #[cfg(target_os = "linux")]
    #[tokio::test(flavor = "multi_thread")]
    #[ignore = "takes a real sleep inhibitor"]
    async fn live_inhibit() {
        let (_app, services) = services();
        let handle = services
            .inhibit_sleep(SleepInhibitRequest {
                reason: "tauri-plugin-desktop-integration live_inhibit test".into(),
                kinds: vec![SleepKind::Sleep],
            })
            .await
            .expect("inhibit");
        println!("holding sleep inhibitor {handle:?}");
        tokio::time::sleep(Duration::from_secs(8)).await;
        services
            .release_sleep_inhibit(handle.handle)
            .await
            .expect("release");
        println!("released");
    }

    /// Emits launcher progress signals for `ca.liminalhq.waypoint.test` that
    /// `dbus-monitor "interface=com.canonical.Unity.LauncherEntry"` shows: 40%, 80%, then cleared.
    #[cfg(target_os = "linux")]
    #[tokio::test(flavor = "multi_thread")]
    #[ignore = "emits launcher entry signals"]
    async fn live_launcher_entry() {
        let (_app, services) = services();
        for progress in [
            LauncherProgress::Value { value: 0.4 },
            LauncherProgress::Value { value: 0.8 },
            LauncherProgress::Cleared,
        ] {
            let mut request = launcher(progress);
            request.count = Some(2);
            services
                .set_launcher_progress(request)
                .await
                .expect("update");
            tokio::time::sleep(Duration::from_millis(700)).await;
        }
    }

    /// Owns `org.freedesktop.FileManager1`, calls `ShowItems` on it from a second connection and
    /// checks that the call arrives as the app event; then gives the name back.
    #[cfg(target_os = "linux")]
    #[tokio::test(flavor = "multi_thread")]
    #[ignore = "briefly owns org.freedesktop.FileManager1; run with --test-threads=1"]
    async fn live_file_manager() {
        use std::sync::{Arc, Mutex};

        use tauri::Listener;

        let (app, services) = services();
        let received: Arc<Mutex<Vec<String>>> = Arc::default();
        let sink = Arc::clone(&received);
        app.listen(crate::file_manager::CALL_EVENT, move |event| {
            sink.lock().unwrap().push(event.payload().to_string());
        });

        match services.own_file_manager().await {
            Ok(ownership) => println!("owned: {ownership:?}"),
            Err(error) => {
                println!("could not own the name: {error:?}");
                assert_eq!(error.kind, ServiceErrorKind::Conflict);
                return;
            }
        }
        let caller = zbus::Connection::session().await.expect("session bus");
        let proxy = zbus::Proxy::new(
            &caller,
            crate::file_manager::BUS_NAME,
            crate::file_manager::OBJECT_PATH,
            "org.freedesktop.FileManager1",
        )
        .await
        .expect("proxy");
        proxy
            .call::<_, _, ()>(
                "ShowItems",
                &(vec!["file:///tmp/My%20Folder/a.txt"], "live-test"),
            )
            .await
            .expect("ShowItems");
        let refused = proxy
            .call::<_, _, ()>("ShowFolders", &(vec!["nonsense"], ""))
            .await;
        println!("ShowFolders with a bad URI: {refused:?}");
        assert!(refused.is_err());
        tokio::time::sleep(Duration::from_millis(300)).await;

        let events = received.lock().unwrap().clone();
        println!("events: {events:?}");
        assert_eq!(events.len(), 1);
        assert!(events[0].contains("show-items") && events[0].contains("/tmp/My Folder/a.txt"));

        let status = services.status().await;
        assert!(status.file_manager_owned);
        let ownership = services.disown_file_manager().await.expect("disown");
        assert!(!ownership.owned);
        assert!(!services.status().await.file_manager_owned);
    }

    /// Takes the name over from the service with a second connection and checks that the loss is
    /// reported and the service forgets it owned the name; then checks that a name held without
    /// permission to replace it gives a `conflict`.
    #[cfg(target_os = "linux")]
    #[tokio::test(flavor = "multi_thread")]
    #[ignore = "briefly owns org.freedesktop.FileManager1; run with --test-threads=1"]
    async fn live_file_manager_name_lost_and_taken() {
        use std::sync::{Arc, Mutex};

        use tauri::Listener;

        // The service is looked up in the app's state when the name is lost, so it must be managed.
        let app = tauri::test::mock_app();
        app.manage(DesktopServices::new(app.handle().clone()));
        let services = app.desktop_services();
        let received: Arc<Mutex<Vec<String>>> = Arc::default();
        let sink = Arc::clone(&received);
        app.listen(crate::file_manager::OWNERSHIP_EVENT, move |event| {
            sink.lock().unwrap().push(event.payload().to_string());
        });
        services.own_file_manager().await.expect("own");

        let thief = zbus::connection::Builder::session()
            .unwrap()
            .name(crate::file_manager::BUS_NAME)
            .unwrap()
            .replace_existing_names(true)
            .build()
            .await
            .expect("take the name over");
        tokio::time::sleep(Duration::from_millis(500)).await;
        let events = received.lock().unwrap().clone();
        println!("ownership events: {events:?}");
        assert_eq!(events.len(), 1);
        assert!(events[0].contains("\"owned\":false"));
        assert!(!services.status().await.file_manager_owned);

        // The thief did not allow replacement, so owning again must fail with a conflict.
        let error = services.own_file_manager().await.unwrap_err();
        println!("own while held elsewhere: {error:?}");
        assert_eq!(error.kind, ServiceErrorKind::Conflict);
        drop(thief);
    }

    /// Shows one test notification through org.freedesktop.Notifications and closes it. Sends a
    /// real notification.
    #[cfg(target_os = "linux")]
    #[tokio::test(flavor = "multi_thread")]
    #[ignore = "shows a real notification"]
    async fn live_notify() {
        let (_app, services) = services();
        services
            .notify(NotifyRequest {
                id: "live-notify".into(),
                title: "tauri-plugin-desktop-integration test".into(),
                body: Some("A single test notification from the live_notify test.".into()),
                default_action: Some("open".into()),
                urgency: None,
                app_name: Some("Desktop integration test".into()),
                desktop_id: None,
            })
            .await
            .expect("notify");
        tokio::time::sleep(Duration::from_secs(4)).await;
        services
            .withdraw_notification("live-notify".into())
            .await
            .expect("withdraw");
    }
}
