// Keeps Windows awake with a power request
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

use windows::{
    core::PWSTR,
    Win32::{
        Foundation::{CloseHandle, HANDLE},
        System::{
            Power::{
                PowerClearRequest, PowerCreateRequest, PowerRequestSystemRequired, PowerSetRequest,
            },
            Threading::{POWER_REQUEST_CONTEXT_SIMPLE_STRING, REASON_CONTEXT, REASON_CONTEXT_0},
        },
    },
};

use super::failed;
use crate::error::ServiceError;

/// The version `PowerCreateRequest` expects in a `REASON_CONTEXT` (`POWER_REQUEST_CONTEXT_VERSION`).
const CONTEXT_VERSION: u32 = 0;

/// A live `PowerCreateRequest` that keeps the system from sleeping until it is dropped.
///
/// A power request, unlike `SetThreadExecutionState`, is not tied to the thread that made it, so
/// it can be taken on a worker and released from any other.
pub struct PowerRequest {
    handle: HANDLE,
}

// SAFETY: the handle is a kernel object handle, which any thread may use.
unsafe impl Send for PowerRequest {}
unsafe impl Sync for PowerRequest {}

impl PowerRequest {
    /// Asks Windows not to sleep, with `reason` shown in `powercfg /requests`.
    pub fn new(reason: &str) -> Result<Self, ServiceError> {
        let mut wide: Vec<u16> = reason.encode_utf16().chain(std::iter::once(0)).collect();
        let context = REASON_CONTEXT {
            Version: CONTEXT_VERSION,
            Flags: POWER_REQUEST_CONTEXT_SIMPLE_STRING,
            Reason: REASON_CONTEXT_0 {
                SimpleReasonString: PWSTR(wide.as_mut_ptr()),
            },
        };
        // SAFETY: `context` and the string it points to outlive the call, which copies them.
        let handle = unsafe { PowerCreateRequest(&context) }
            .map_err(|error| failed("PowerCreateRequest", error))?;
        // SAFETY: `handle` is the valid handle just created.
        if let Err(error) = unsafe { PowerSetRequest(handle, PowerRequestSystemRequired) } {
            // SAFETY: the handle is valid and not used again.
            let _ = unsafe { CloseHandle(handle) };
            return Err(failed("PowerSetRequest", error));
        }
        Ok(Self { handle })
    }
}

impl Drop for PowerRequest {
    fn drop(&mut self) {
        // SAFETY: the handle is valid until this point and not used again.
        unsafe {
            let _ = PowerClearRequest(self.handle, PowerRequestSystemRequired);
            let _ = CloseHandle(self.handle);
        }
    }
}
