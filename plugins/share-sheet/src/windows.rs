// Shows the Windows share UI for a file or text through IDataTransferManagerInterop
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

use std::{cell::RefCell, collections::HashMap, marker::PhantomData, path::Path};

use log::{info, warn};
use serde::de::DeserializeOwned;
use tauri::{plugin::PluginApi, AppHandle, Runtime, WebviewWindow};
use windows::{
    core::{factory, Interface, HSTRING},
    ApplicationModel::DataTransfer::{DataRequestedEventArgs, DataTransferManager},
    Foundation::TypedEventHandler,
    Storage::{IStorageItem, StorageFile},
    Win32::{
        Foundation::HWND,
        System::Com::{CoInitializeEx, COINIT_MULTITHREADED},
        UI::Shell::IDataTransferManagerInterop,
    },
};
use windows_collections::IIterable;
use windows_core::AgileReference;

use crate::{
    error::{Error, Result},
    main_thread,
    models::{ShareCapabilities, SharePlatform, ShareResult, ShareStatus},
    validate::{self, FileRequest, TextRequest},
};

pub struct Backend<R: Runtime>(PhantomData<fn() -> R>);

pub fn init<R: Runtime, C: DeserializeOwned>(
    _app: &AppHandle<R>,
    _api: PluginApi<R, C>,
) -> Result<Backend<R>> {
    Ok(Backend(PhantomData))
}

/// What the share UI receives. The file is opened before the UI is shown, so a file Windows
/// cannot open is reported as an error rather than as an empty share sheet.
enum Content {
    File(AgileReference<StorageFile>),
    Text(String),
}

impl<R: Runtime> Backend<R> {
    pub async fn capabilities(&self, window: &WebviewWindow<R>) -> ShareCapabilities {
        let available = main_thread::run(window, share_ui_available)
            .await
            .unwrap_or(false);
        ShareCapabilities::for_platform(SharePlatform::Windows).with_runtime_support(available)
    }

    pub async fn share_file(
        &self,
        window: &WebviewWindow<R>,
        request: FileRequest,
    ) -> Result<ShareResult> {
        let path = validate::resolve_existing_file(&request.path)?;
        let path = validate::strip_verbatim_prefix(&path);
        let title = request
            .title
            .unwrap_or_else(|| validate::display_name(&path));
        let file = tauri::async_runtime::spawn_blocking(move || open_storage_file(&path))
            .await
            .map_err(|error| Error::failed(format!("the file could not be opened: {error}")))??;
        show(window, title, Content::File(file)).await
    }

    pub async fn share_text(
        &self,
        window: &WebviewWindow<R>,
        request: TextRequest,
    ) -> Result<ShareResult> {
        let title = request
            .title
            .unwrap_or_else(|| validate::text_title(&request.text));
        show(window, title, Content::Text(request.text)).await
    }
}

/// `DataTransferManager.IsSupported`, which needs Windows 10 version 1709 or later; any error
/// counts as no share UI.
fn share_ui_available() -> bool {
    DataTransferManager::IsSupported().unwrap_or(false)
}

/// Runs on a blocking worker thread, so waiting for the asynchronous open does not stall the
/// window's thread.
fn open_storage_file(path: &Path) -> Result<AgileReference<StorageFile>> {
    // SAFETY: joins this worker thread to the multithreaded apartment so it may call the Windows
    // Runtime. A thread already in an apartment returns an error that is safe to ignore.
    let _ = unsafe { CoInitializeEx(None, COINIT_MULTITHREADED) };
    let file = StorageFile::GetFileFromPathAsync(&HSTRING::from(path.as_os_str()))
        .and_then(|operation| operation.join())
        .map_err(|error| {
            Error::failed(format!(
                "Windows could not open {} for sharing: {error}",
                path.display()
            ))
        })?;
    AgileReference::new(&file)
        .map_err(|error| Error::failed(format!("the file could not be handed over: {error}")))
}

/// Shows the share UI and resolves as soon as it is up. `ShowShareUIForWindow` returns once the
/// UI is shown and Windows does not report reliably how it ends, so the status is `unknown`.
async fn show<R: Runtime>(
    window: &WebviewWindow<R>,
    title: String,
    content: Content,
) -> Result<ShareResult> {
    let target = window.clone();
    main_thread::run(window, move || show_on_main_thread(&target, title, content))
        .await
        .ok_or_else(|| Error::failed("cannot reach the window's thread"))??;
    Ok(ShareResult::new(ShareStatus::Unknown))
}

thread_local! {
    /// The `DataRequested` handler registered for each window, so a later share replaces it
    /// instead of adding a second one.
    static REGISTRATIONS: RefCell<HashMap<isize, (DataTransferManager, i64)>> =
        RefCell::new(HashMap::new());
}

/// Must run on the thread that owns the window.
///
/// This native path has not been run on Windows yet. It logs before each call into the Windows
/// Runtime, so the last "untested native path" line in the log names a step that failed.
fn show_on_main_thread<R: Runtime>(
    window: &WebviewWindow<R>,
    title: String,
    content: Content,
) -> Result<()> {
    // The handle is resolved here, on the window's thread, so a window that closed after the
    // request was queued cannot leave a handle Windows has since reused.
    let hwnd = window
        .hwnd()
        .map_err(|error| Error::failed(format!("the window has no handle: {error}")))?;

    info!("windows share (untested native path): loading IDataTransferManagerInterop");
    let interop =
        factory::<DataTransferManager, IDataTransferManagerInterop>().map_err(|error| {
            Error::unsupported(format!("the Windows share UI is not available: {error}"))
        })?;
    // SAFETY: `hwnd` is the live handle of a window owned by this thread.
    let manager: DataTransferManager = unsafe { interop.GetForWindow(hwnd) }.map_err(|error| {
        Error::failed(format!(
            "cannot get the window's DataTransferManager: {error}"
        ))
    })?;

    let handler = TypedEventHandler::<DataTransferManager, DataRequestedEventArgs>::new(
        move |_, args| {
            let args = args.ok()?;
            let request = args.Request()?;
            let fill = || -> windows_core::Result<()> {
                let data = request.Data()?;
                // The share UI refuses a package without a title.
                data.Properties()?
                    .SetTitle(&HSTRING::from(title.as_str()))?;
                match &content {
                    Content::File(file) => {
                        let item: IStorageItem = file.resolve()?.cast()?;
                        let items = IIterable::<IStorageItem>::from(vec![Some(item)]);
                        data.SetStorageItems(&items, true)?;
                    }
                    Content::Text(text) => data.SetText(&HSTRING::from(text.as_str()))?,
                }
                Ok(())
            };
            if let Err(error) = fill() {
                warn!("windows share (untested native path): filling the data package failed: {error}");
                let _ =
                    request.FailWithDisplayText(&HSTRING::from("This item could not be shared."));
            }
            Ok(())
        },
    );
    info!("windows share (untested native path): registering DataRequested");
    let token = manager
        .DataRequested(&handler)
        .map_err(|error| Error::failed(format!("cannot register for DataRequested: {error}")))?;
    replace_registration(hwnd, manager, token);

    info!("windows share (untested native path): calling ShowShareUIForWindow");
    // SAFETY: `hwnd` is valid (above) and the call runs on the thread that owns it.
    unsafe { interop.ShowShareUIForWindow(hwnd) }
        .map_err(|error| Error::failed(format!("ShowShareUIForWindow failed: {error}")))?;
    info!("windows share (untested native path): the share UI is up");
    Ok(())
}

fn replace_registration(hwnd: HWND, manager: DataTransferManager, token: i64) {
    let previous = REGISTRATIONS.with(|registrations| {
        registrations
            .borrow_mut()
            .insert(hwnd.0 as isize, (manager, token))
    });
    if let Some((manager, token)) = previous {
        if let Err(error) = manager.RemoveDataRequested(token) {
            warn!("windows share: removing the previous DataRequested handler failed: {error}");
        }
    }
}
