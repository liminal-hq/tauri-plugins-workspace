// Shows NSSharingServicePicker over the calling window and reports the person's choice
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

use std::{cell::RefCell, marker::PhantomData, path::PathBuf};

use log::{info, warn};
use objc2::{
    define_class, msg_send,
    rc::Retained,
    runtime::{AnyObject, NSObject, NSObjectProtocol, ProtocolObject},
    AllocAnyThread, DefinedClass, MainThreadMarker, MainThreadOnly,
};
use objc2_app_kit::{
    NSSharingService, NSSharingServicePicker, NSSharingServicePickerDelegate, NSView,
};
use objc2_foundation::{NSArray, NSPoint, NSRect, NSRectEdge, NSSize, NSString, NSURL};
use serde::de::DeserializeOwned;
use tauri::{plugin::PluginApi, AppHandle, Runtime, WebviewWindow};
use tokio::sync::oneshot;

use crate::{
    error::{Error, Result},
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

type Reply = oneshot::Sender<Result<ShareResult>>;

enum Item {
    File(PathBuf),
    Text(String),
}

impl<R: Runtime> Backend<R> {
    pub async fn capabilities(&self, _window: &WebviewWindow<R>) -> ShareCapabilities {
        ShareCapabilities::for_platform(SharePlatform::Macos)
    }

    pub async fn share_file(
        &self,
        window: &WebviewWindow<R>,
        request: FileRequest,
    ) -> Result<ShareResult> {
        let path = validate::resolve_existing_file(&request.path)?;
        present(window, Item::File(path), request.title).await
    }

    pub async fn share_text(
        &self,
        window: &WebviewWindow<R>,
        request: TextRequest,
    ) -> Result<ShareResult> {
        present(window, Item::Text(request.text), request.title).await
    }
}

/// Shows the picker on the main thread and waits for its delegate. If the delegate is released
/// without answering, the outcome is `unknown`.
async fn present<R: Runtime>(
    window: &WebviewWindow<R>,
    item: Item,
    title: Option<String>,
) -> Result<ShareResult> {
    let (reply, outcome) = oneshot::channel();
    let target = window.clone();
    window
        .run_on_main_thread(move || show(&target, item, title, reply))
        .map_err(|error| Error::failed(format!("cannot reach the main thread: {error}")))?;
    outcome
        .await
        .unwrap_or(Ok(ShareResult::new(ShareStatus::Unknown)))
}

pub struct PickerIvars {
    reply: RefCell<Option<Reply>>,
    title: Option<String>,
}

define_class!(
    // SAFETY: NSObject has no subclassing requirements, and `PickerDelegate` does not implement
    // `Drop`.
    #[unsafe(super(NSObject))]
    #[thread_kind = MainThreadOnly]
    #[name = "LiminalShareSheetPickerDelegate"]
    #[ivars = PickerIvars]
    struct PickerDelegate;

    unsafe impl NSObjectProtocol for PickerDelegate {}

    unsafe impl NSSharingServicePickerDelegate for PickerDelegate {
        /// AppKit calls this with the chosen service, or with none when the picker was dismissed.
        #[unsafe(method(sharingServicePicker:didChooseSharingService:))]
        fn did_choose(&self, _picker: &NSSharingServicePicker, service: Option<&NSSharingService>) {
            if let (Some(service), Some(title)) = (service, self.ivars().title.as_deref()) {
                service.setSubject(Some(&NSString::from_str(title)));
            }
            info!(
                "macos share picker (untested native path): {}",
                if service.is_some() {
                    "a service was chosen"
                } else {
                    "dismissed"
                }
            );
            self.finish(Ok(ShareResult::from_chosen_service(service.is_some())));
        }
    }
);

impl PickerDelegate {
    fn new(mtm: MainThreadMarker, reply: Reply, title: Option<String>) -> Retained<Self> {
        let this = Self::alloc(mtm).set_ivars(PickerIvars {
            reply: RefCell::new(Some(reply)),
            title,
        });
        // SAFETY: `init` is NSObject's designated initialiser.
        unsafe { msg_send![super(this), init] }
    }

    fn finish(&self, result: Result<ShareResult>) {
        if let Some(reply) = self.ivars().reply.borrow_mut().take() {
            let _ = reply.send(result);
        }
    }
}

/// The picker that is showing and its delegate. The picker holds its delegate weakly, so both
/// are kept here until the next share replaces them; the delegate is never released from inside
/// its own callback.
struct Active {
    picker: Retained<NSSharingServicePicker>,
    delegate: Retained<PickerDelegate>,
}

thread_local! {
    static ACTIVE: RefCell<Option<Active>> = const { RefCell::new(None) };
}

/// Must run on the main thread.
fn show<R: Runtime>(window: &WebviewWindow<R>, item: Item, title: Option<String>, reply: Reply) {
    let Some(mtm) = MainThreadMarker::new() else {
        let _ = reply.send(Err(Error::failed(
            "the share picker must open on the main thread",
        )));
        return;
    };

    // The view is looked up here, on the main thread, so a window that closed after the request
    // was queued cannot leave a dangling pointer.
    let raw_view = match window.ns_view() {
        Ok(raw_view) => raw_view,
        Err(error) => {
            let _ = reply.send(Err(Error::failed(format!(
                "the window has no content view: {error}"
            ))));
            return;
        }
    };
    // SAFETY: Tauri returns either null or the window's content view, an `NSView`, and this runs
    // on the main thread that owns it, so nothing can release it between this check and the retain.
    let Some(view) = (unsafe { Retained::retain(raw_view.cast::<NSView>()) }) else {
        let _ = reply.send(Err(Error::failed("the window's content view is gone")));
        return;
    };

    let object: Retained<AnyObject> = match item {
        Item::File(path) => {
            let url = NSURL::fileURLWithPath(&NSString::from_str(&path.to_string_lossy()));
            Retained::into_super(Retained::into_super(url))
        }
        Item::Text(text) => Retained::into_super(Retained::into_super(NSString::from_str(&text))),
    };
    let items = NSArray::from_retained_slice(&[object]);

    // SAFETY: the items are an NSURL or an NSString, both of which conform to
    // NSPasteboardWriting as `initWithItems:` requires.
    let picker =
        unsafe { NSSharingServicePicker::initWithItems(NSSharingServicePicker::alloc(), &items) };
    let delegate = PickerDelegate::new(mtm, reply, title);
    picker.setDelegate(Some(ProtocolObject::from_ref(&*delegate)));

    let previous = ACTIVE.with(|active| {
        active.borrow_mut().replace(Active {
            picker: picker.clone(),
            delegate,
        })
    });
    if let Some(previous) = previous {
        // A second share while a picker is open supersedes the first.
        warn!("macos share picker: a new share replaced one that was still open");
        previous
            .delegate
            .finish(Ok(ShareResult::new(ShareStatus::Unknown)));
        previous.picker.close();
    }

    let bounds = view.bounds();
    let anchor = NSRect::new(
        NSPoint::new(bounds.size.width / 2.0, bounds.size.height / 2.0),
        NSSize::new(0.0, 0.0),
    );
    info!("macos share picker (untested native path): showing the picker");
    picker.showRelativeToRect_ofView_preferredEdge(anchor, &view, NSRectEdge::MinY);
}
