// Registers global hotkeys on a message-only window thread
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

use std::{
    collections::{HashMap, VecDeque},
    sync::{mpsc, Arc, Mutex},
    thread::JoinHandle,
};

use windows::{
    core::{w, PCWSTR},
    Win32::{
        Foundation::{HINSTANCE, HWND, LPARAM, LRESULT, WPARAM},
        System::{LibraryLoader::GetModuleHandleW, Threading::GetCurrentThreadId},
        UI::{
            Input::KeyboardAndMouse::{
                RegisterHotKey, UnregisterHotKey, HOT_KEY_MODIFIERS, MOD_NOREPEAT,
            },
            WindowsAndMessaging::{
                CreateWindowExW, DefWindowProcW, DestroyWindow, DispatchMessageW, GetMessageW,
                PostMessageW, RegisterClassW, TranslateMessage, HWND_MESSAGE, MSG, WINDOW_EX_STYLE,
                WINDOW_STYLE, WM_APP, WM_HOTKEY, WNDCLASSW,
            },
        },
    },
};

use super::failed;
use crate::{
    error::{ServiceError, ServiceErrorKind},
    shortcuts::Accelerator,
};

/// Posted to the window to make the thread drain its command queue.
const WM_COMMANDS: u32 = WM_APP + 1;

enum Command {
    Register {
        id: String,
        accelerator: Accelerator,
        reply: mpsc::Sender<Result<(), ServiceError>>,
    },
    Unregister {
        id: String,
        reply: mpsc::Sender<Result<(), ServiceError>>,
    },
    Stop,
}

/// Called on the hotkey thread with the shortcut id each time a registered hotkey is pressed.
pub type PressedSink = Arc<dyn Fn(String) + Send + Sync>;

/// The hotkey thread: a message-only window whose thread owns every registration, because
/// `RegisterHotKey` ties a hotkey to the thread that made the call and `WM_HOTKEY` is delivered
/// to that thread's queue.
pub struct HotkeyThread {
    hwnd: isize,
    queue: Arc<Mutex<VecDeque<Command>>>,
    join: Option<JoinHandle<()>>,
}

unsafe extern "system" fn window_proc(
    hwnd: HWND,
    message: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    // SAFETY: forwards the arguments the system gave this callback unchanged.
    unsafe { DefWindowProcW(hwnd, message, wparam, lparam) }
}

impl HotkeyThread {
    pub fn start(on_pressed: PressedSink) -> Result<Self, ServiceError> {
        let queue: Arc<Mutex<VecDeque<Command>>> = Arc::default();
        let thread_queue = Arc::clone(&queue);
        let (ready_tx, ready_rx) = mpsc::channel();
        let join = std::thread::Builder::new()
            .name("desktop-integration-hotkeys".into())
            .spawn(move || run(thread_queue, on_pressed, ready_tx))
            .map_err(|error| failed("starting the hotkey thread", error))?;
        match ready_rx.recv() {
            Ok(Ok(hwnd)) => Ok(Self {
                hwnd,
                queue,
                join: Some(join),
            }),
            Ok(Err(error)) => {
                let _ = join.join();
                Err(error)
            }
            Err(_) => Err(ServiceError::new(
                ServiceErrorKind::Failed,
                "the hotkey thread stopped while starting",
            )),
        }
    }

    fn send(&self, command: Command) -> Result<(), ServiceError> {
        self.queue
            .lock()
            .map_err(|_| {
                ServiceError::new(ServiceErrorKind::Failed, "the hotkey queue is poisoned")
            })?
            .push_back(command);
        // SAFETY: the window belongs to the hotkey thread, which outlives `self`.
        unsafe {
            PostMessageW(
                Some(HWND(self.hwnd as *mut _)),
                WM_COMMANDS,
                WPARAM(0),
                LPARAM(0),
            )
        }
        .map_err(|error| failed("PostMessageW", error))
    }

    fn round_trip(
        &self,
        build: impl FnOnce(mpsc::Sender<Result<(), ServiceError>>) -> Command,
    ) -> Result<(), ServiceError> {
        let (reply_tx, reply_rx) = mpsc::channel();
        self.send(build(reply_tx))?;
        reply_rx
            .recv_timeout(crate::error::CALL_TIMEOUT)
            .map_err(|_| ServiceError::timeout("the hotkey thread"))?
    }

    /// Registers `accelerator` under `id`, replacing an earlier registration of the same id.
    pub fn register(&self, id: &str, accelerator: Accelerator) -> Result<(), ServiceError> {
        let id = id.to_string();
        self.round_trip(|reply| Command::Register {
            id,
            accelerator,
            reply,
        })
    }

    pub fn unregister(&self, id: &str) -> Result<(), ServiceError> {
        let id = id.to_string();
        self.round_trip(|reply| Command::Unregister { id, reply })
    }
}

impl Drop for HotkeyThread {
    fn drop(&mut self) {
        let _ = self.send(Command::Stop);
        if let Some(join) = self.join.take() {
            let _ = join.join();
        }
    }
}

fn run(
    queue: Arc<Mutex<VecDeque<Command>>>,
    on_pressed: PressedSink,
    ready: mpsc::Sender<Result<isize, ServiceError>>,
) {
    let class = w!("DesktopIntegrationHotkeys");
    // SAFETY: plain window creation on this thread; every pointer passed outlives its call.
    let hwnd = unsafe {
        let instance = match GetModuleHandleW(PCWSTR::null()) {
            Ok(module) => HINSTANCE(module.0),
            Err(error) => {
                let _ = ready.send(Err(failed("GetModuleHandleW", error)));
                return;
            }
        };
        let class_def = WNDCLASSW {
            lpfnWndProc: Some(window_proc),
            hInstance: instance,
            lpszClassName: class,
            ..Default::default()
        };
        // Registering twice (a second thread after the first stopped) fails harmlessly.
        RegisterClassW(&class_def);
        match CreateWindowExW(
            WINDOW_EX_STYLE(0),
            class,
            w!(""),
            WINDOW_STYLE(0),
            0,
            0,
            0,
            0,
            Some(HWND_MESSAGE),
            None,
            Some(instance),
            None,
        ) {
            Ok(hwnd) => hwnd,
            Err(error) => {
                let _ = ready.send(Err(failed("CreateWindowExW", error)));
                return;
            }
        }
    };
    let _ = ready.send(Ok(hwnd.0 as isize));
    // SAFETY: valid on any thread.
    let _thread = unsafe { GetCurrentThreadId() };

    // Hotkey ids are small numbers private to this window.
    let mut next_id: i32 = 1;
    let mut by_name: HashMap<String, i32> = HashMap::new();
    let mut by_hotkey: HashMap<i32, String> = HashMap::new();

    let mut message = MSG::default();
    loop {
        // SAFETY: `message` is a valid out-parameter.
        let got = unsafe { GetMessageW(&mut message, None, 0, 0) };
        if got.0 <= 0 {
            break;
        }
        match message.message {
            WM_HOTKEY => {
                if let Some(id) = by_hotkey.get(&(message.wParam.0 as i32)) {
                    on_pressed(id.clone());
                }
            }
            WM_COMMANDS => {
                let commands: Vec<Command> = queue
                    .lock()
                    .map(|mut queue| queue.drain(..).collect())
                    .unwrap_or_default();
                let mut stop = false;
                for command in commands {
                    match command {
                        Command::Register {
                            id,
                            accelerator,
                            reply,
                        } => {
                            let result = register(
                                hwnd,
                                &mut next_id,
                                &mut by_name,
                                &mut by_hotkey,
                                &id,
                                accelerator,
                            );
                            let _ = reply.send(result);
                        }
                        Command::Unregister { id, reply } => {
                            let result = match by_name.remove(&id) {
                                Some(hotkey) => {
                                    by_hotkey.remove(&hotkey);
                                    // SAFETY: the hotkey was registered on this window.
                                    unsafe { UnregisterHotKey(Some(hwnd), hotkey) }
                                        .map_err(|error| failed("UnregisterHotKey", error))
                                }
                                None => Err(ServiceError::not_found(format!(
                                    "no shortcut is registered as {id:?}"
                                ))),
                            };
                            let _ = reply.send(result);
                        }
                        Command::Stop => stop = true,
                    }
                }
                if stop {
                    break;
                }
            }
            _ => {
                // SAFETY: the message came from GetMessageW.
                unsafe {
                    let _ = TranslateMessage(&message);
                    DispatchMessageW(&message);
                }
            }
        }
    }
    for hotkey in by_hotkey.keys() {
        // SAFETY: each hotkey was registered on this window.
        let _ = unsafe { UnregisterHotKey(Some(hwnd), *hotkey) };
    }
    // SAFETY: the window was created on this thread.
    let _ = unsafe { DestroyWindow(hwnd) };
}

fn register(
    hwnd: HWND,
    next_id: &mut i32,
    by_name: &mut HashMap<String, i32>,
    by_hotkey: &mut HashMap<i32, String>,
    id: &str,
    accelerator: Accelerator,
) -> Result<(), ServiceError> {
    let hotkey = *next_id;
    // SAFETY: the window belongs to this thread.
    unsafe {
        RegisterHotKey(
            Some(hwnd),
            hotkey,
            HOT_KEY_MODIFIERS(accelerator.modifiers) | MOD_NOREPEAT,
            accelerator.key,
        )
    }
    .map_err(|error| {
        // ERROR_HOTKEY_ALREADY_REGISTERED: another process holds the combination.
        if error.code().0 as u32 == 0x8007_0581 {
            ServiceError::new(
                ServiceErrorKind::Conflict,
                "another application already uses that shortcut",
            )
        } else {
            failed("RegisterHotKey", error)
        }
    })?;
    *next_id += 1;
    // A repeated id replaces the old binding, after the new one is safely registered.
    if let Some(previous) = by_name.insert(id.to_string(), hotkey) {
        by_hotkey.remove(&previous);
        // SAFETY: the previous hotkey was registered on this window.
        let _ = unsafe { UnregisterHotKey(Some(hwnd), previous) };
    }
    by_hotkey.insert(hotkey, id.to_string());
    Ok(())
}
