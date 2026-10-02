// Shows toast notifications through WinRT with the process's AppUserModelID
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

use std::{collections::HashMap, sync::Mutex};

use windows::{
    core::{IInspectable, Interface, Ref, HSTRING, PWSTR},
    Data::Xml::Dom::XmlDocument,
    Foundation::TypedEventHandler,
    Win32::{
        System::{
            Com::CoTaskMemFree,
            WinRT::{RoInitialize, RO_INIT_MULTITHREADED},
        },
        UI::Shell::{
            GetCurrentProcessExplicitAppUserModelID, SetCurrentProcessExplicitAppUserModelID,
        },
    },
    UI::Notifications::{ToastActivatedEventArgs, ToastNotification, ToastNotificationManager},
};

use super::failed;
use crate::{
    error::{ServiceError, ServiceErrorKind},
    models::{NotificationAction, NotifyRequest},
    notify::{parse_toast_launch, toast_tag, toast_xml},
};

/// How many shown toasts are kept alive so their activation handlers keep firing.
const KEEP: usize = 64;

/// The AppUserModelID this process has set explicitly, if any.
pub fn app_user_model_id() -> Option<String> {
    // SAFETY: the call writes a CoTaskMem string pointer, which is read and freed below.
    let id: PWSTR = unsafe { GetCurrentProcessExplicitAppUserModelID() }.ok()?;
    // SAFETY: `id` is a valid nul-terminated string allocated by the call.
    let text = unsafe { id.to_string() }.ok();
    // SAFETY: the string was allocated with CoTaskMemAlloc and is not used again.
    unsafe { CoTaskMemFree(Some(id.0.cast())) };
    text.filter(|text| !text.is_empty())
}

/// Sets the AppUserModelID of this process, which toasts need to be attributed to the app.
pub fn set_app_user_model_id(id: &str) -> Result<(), ServiceError> {
    if id.is_empty() {
        return Err(ServiceError::invalid(
            "the AppUserModelID must not be empty",
        ));
    }
    // SAFETY: the string outlives the call, which copies it.
    unsafe { SetCurrentProcessExplicitAppUserModelID(&HSTRING::from(id)) }
        .map_err(|error| failed("SetCurrentProcessExplicitAppUserModelID", error))
}

/// The toasts shown so far, kept alive until withdrawn or pushed out.
#[derive(Default)]
pub struct Shown {
    toasts: Mutex<(Vec<String>, HashMap<String, ToastNotification>)>,
}

impl Shown {
    fn keep(&self, tag: String, toast: ToastNotification) {
        if let Ok(mut guard) = self.toasts.lock() {
            let (order, map) = &mut *guard;
            order.retain(|existing| existing != &tag);
            order.push(tag.clone());
            map.insert(tag, toast);
            while order.len() > KEEP {
                let oldest = order.remove(0);
                map.remove(&oldest);
            }
        }
    }

    fn drop_tag(&self, tag: &str) {
        if let Ok(mut guard) = self.toasts.lock() {
            let (order, map) = &mut *guard;
            order.retain(|existing| existing != tag);
            map.remove(tag);
        }
    }
}

/// Shows (or replaces, by tag) a toast. `on_action` runs when the user clicks it.
pub fn show(
    shown: &Shown,
    request: &NotifyRequest,
    on_action: impl Fn(NotificationAction) + Send + 'static,
) -> Result<(), ServiceError> {
    let app_id = app_user_model_id().ok_or_else(|| {
        ServiceError::new(
            ServiceErrorKind::NeedsAppId,
            "Windows toasts need an AppUserModelID; set one with set_app_user_model_id",
        )
    })?;
    // SAFETY: initialising the WinRT apartment is idempotent; a different apartment mode already
    // set on this thread is fine for the calls below.
    let _ = unsafe { RoInitialize(RO_INIT_MULTITHREADED) };

    let document = XmlDocument::new().map_err(|error| failed("XmlDocument", error))?;
    document
        .LoadXml(&HSTRING::from(toast_xml(request)))
        .map_err(|error| failed("loading the toast XML", error))?;
    let toast = ToastNotification::CreateToastNotification(&document)
        .map_err(|error| failed("CreateToastNotification", error))?;
    let tag = toast_tag(&request.id);
    toast
        .SetTag(&HSTRING::from(tag.as_str()))
        .map_err(|error| failed("SetTag", error))?;
    toast
        .Activated(&TypedEventHandler::new(
            move |_toast: Ref<ToastNotification>, args: Ref<IInspectable>| {
                let launch = args
                    .as_ref()
                    .and_then(|args| args.cast::<ToastActivatedEventArgs>().ok())
                    .and_then(|args| args.Arguments().ok())
                    .map(|arguments| arguments.to_string());
                if let Some(action) = launch.as_deref().and_then(parse_toast_launch) {
                    on_action(action);
                }
                Ok(())
            },
        ))
        .map_err(|error| failed("Activated", error))?;
    ToastNotificationManager::CreateToastNotifierWithId(&HSTRING::from(app_id))
        .and_then(|notifier| notifier.Show(&toast))
        .map_err(|error| failed("showing the toast", error))?;
    shown.keep(tag, toast);
    Ok(())
}

/// Removes a toast from the notification centre.
pub fn withdraw(shown: &Shown, id: &str) -> Result<(), ServiceError> {
    if app_user_model_id().is_none() {
        return Err(ServiceError::new(
            ServiceErrorKind::NeedsAppId,
            "no AppUserModelID is set",
        ));
    }
    let tag = toast_tag(id);
    shown.drop_tag(&tag);
    // SAFETY: see `show`.
    let _ = unsafe { RoInitialize(RO_INIT_MULTITHREADED) };
    ToastNotificationManager::History()
        .and_then(|history| history.Remove(&HSTRING::from(tag)))
        .map_err(|error| failed("removing the toast", error))
}
