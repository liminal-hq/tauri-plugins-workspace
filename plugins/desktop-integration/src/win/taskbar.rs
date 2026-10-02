// Shows progress on a window's taskbar button through ITaskbarList3
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

use windows::Win32::{
    Foundation::HWND,
    System::Com::{CoCreateInstance, CLSCTX_INPROC_SERVER},
    UI::Shell::{ITaskbarList3, TaskbarList, TBPFLAG},
};

use super::failed;
use crate::{
    error::ServiceError,
    launcher::{TaskbarPlan, TASKBAR_TOTAL},
};

/// Applies `plan` to the taskbar button of the window `hwnd`.
///
/// Must run on the thread that owns the window, which has COM initialised.
pub fn apply(hwnd: HWND, plan: TaskbarPlan) -> Result<(), ServiceError> {
    // SAFETY: COM is initialised on the UI thread by the windowing library, and the interface
    // pointer is used only within this call.
    unsafe {
        let list: ITaskbarList3 = CoCreateInstance(&TaskbarList, None, CLSCTX_INPROC_SERVER)
            .map_err(|error| failed("creating ITaskbarList3", error))?;
        list.HrInit()
            .map_err(|error| failed("ITaskbarList3::HrInit", error))?;
        // Set the value before the state so a normal bar never flashes at its old value.
        list.SetProgressValue(hwnd, plan.completed, TASKBAR_TOTAL)
            .map_err(|error| failed("SetProgressValue", error))?;
        list.SetProgressState(hwnd, TBPFLAG(plan.state as i32))
            .map_err(|error| failed("SetProgressState", error))?;
    }
    Ok(())
}
