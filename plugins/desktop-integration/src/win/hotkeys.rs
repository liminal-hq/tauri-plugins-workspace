// Registers global hotkeys on a message-only window thread
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

use std::{
    collections::VecDeque,
    sync::{mpsc, Arc, Mutex},
    thread::JoinHandle,
    time::Duration,
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
    hotkey_book::{join_within, HotkeyApi, HotkeyBook, Ticket},
    shortcuts::Accelerator,
};

/// How long dropping the thread waits for it to end before leaving it behind.
const STOP_TIMEOUT: Duration = Duration::from_secs(2);

/// Posted to the window to make the thread drain its command queue.
const WM_COMMANDS: u32 = WM_APP + 1;

enum Command {
    Register {
        id: String,
        accelerator: Accelerator,
        reply: mpsc::Sender<Result<(), ServiceError>>,
        ticket: Ticket,
    },
    Unregister {
        id: String,
        reply: mpsc::Sender<Result<(), ServiceError>>,
        ticket: Ticket,
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
    /// Closed by the thread as it ends.
    done: Mutex<mpsc::Receiver<()>>,
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
        let (done_tx, done) = mpsc::channel();
        let join = std::thread::Builder::new()
            .name("desktop-integration-hotkeys".into())
            .spawn(move || run(thread_queue, on_pressed, ready_tx, done_tx))
            .map_err(|error| failed("starting the hotkey thread", error))?;
        match ready_rx.recv_timeout(crate::error::CALL_TIMEOUT) {
            Ok(Ok(hwnd)) => Ok(Self {
                hwnd,
                queue,
                join: Some(join),
                done: Mutex::new(done),
            }),
            Ok(Err(error)) => {
                let _ = join.join();
                Err(error)
            }
            Err(mpsc::RecvTimeoutError::Timeout) => Err(ServiceError::timeout("the hotkey thread")),
            Err(mpsc::RecvTimeoutError::Disconnected) => Err(ServiceError::new(
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

    /// Queues a command and waits for its result. A command the thread has not started by the
    /// time the wait ends is cancelled, so a call that reports a timeout never takes effect later
    /// (and a retry never finds its own earlier registration).
    fn round_trip(
        &self,
        build: impl FnOnce(mpsc::Sender<Result<(), ServiceError>>, Ticket) -> Command,
    ) -> Result<(), ServiceError> {
        let (reply_tx, reply_rx) = mpsc::channel();
        let ticket = Ticket::default();
        if let Err(error) = self.send(build(reply_tx, ticket.clone())) {
            ticket.cancel();
            return Err(error);
        }
        match reply_rx.recv_timeout(crate::error::CALL_TIMEOUT) {
            Ok(result) => result,
            Err(_) if ticket.cancel() => Err(ServiceError::timeout("the hotkey thread")),
            // The thread is already running the command, so its outcome is the answer.
            Err(_) => reply_rx
                .recv_timeout(crate::error::CALL_TIMEOUT)
                .map_err(|_| ServiceError::timeout("the hotkey thread"))?,
        }
    }

    /// Registers `accelerator` under `id`, replacing an earlier registration of the same id.
    pub fn register(&self, id: &str, accelerator: Accelerator) -> Result<(), ServiceError> {
        let id = id.to_string();
        self.round_trip(|reply, ticket| Command::Register {
            id,
            accelerator,
            reply,
            ticket,
        })
    }

    pub fn unregister(&self, id: &str) -> Result<(), ServiceError> {
        let id = id.to_string();
        self.round_trip(|reply, ticket| Command::Unregister { id, reply, ticket })
    }
}

impl Drop for HotkeyThread {
    fn drop(&mut self) {
        let stop = self.send(Command::Stop);
        let Some(join) = self.join.take() else {
            return;
        };
        let done = self
            .done
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        match stop {
            // Without the stop message the thread never ends, so there is nothing to wait for.
            Err(error) => log::warn!("could not stop the hotkey thread: {}", error.message),
            // A hotkey callback that blocks would otherwise hold up the app's exit for good.
            Ok(()) if !join_within(join, &done, STOP_TIMEOUT) => {
                log::warn!("the hotkey thread did not stop in time; leaving it behind");
            }
            Ok(()) => {}
        }
    }
}

fn run(
    queue: Arc<Mutex<VecDeque<Command>>>,
    on_pressed: PressedSink,
    ready: mpsc::Sender<Result<isize, ServiceError>>,
    // Dropped as the thread ends, which is how `HotkeyThread` knows it can join.
    _done: mpsc::Sender<()>,
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

    let mut api = Win32Hotkeys { hwnd };
    let mut book = HotkeyBook::default();

    let mut message = MSG::default();
    loop {
        // SAFETY: `message` is a valid out-parameter.
        let got = unsafe { GetMessageW(&mut message, None, 0, 0) };
        if got.0 <= 0 {
            break;
        }
        match message.message {
            WM_HOTKEY => {
                if let Some(id) = book.id_for(message.wParam.0 as i32) {
                    on_pressed(id.to_string());
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
                            ticket,
                        } => {
                            if ticket.start() {
                                let _ = reply.send(book.register(&mut api, &id, accelerator));
                            }
                        }
                        Command::Unregister { id, reply, ticket } => {
                            if ticket.start() {
                                let _ = reply.send(book.unregister(&mut api, &id));
                            }
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
    book.unregister_all(&mut api);
    // SAFETY: the window was created on this thread.
    let _ = unsafe { DestroyWindow(hwnd) };
}

/// `RegisterHotKey` and `UnregisterHotKey` on the hotkey window.
struct Win32Hotkeys {
    hwnd: HWND,
}

impl HotkeyApi for Win32Hotkeys {
    fn register(&mut self, hotkey: i32, accelerator: Accelerator) -> Result<(), ServiceError> {
        // SAFETY: the window belongs to this thread.
        unsafe {
            RegisterHotKey(
                Some(self.hwnd),
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
        })
    }

    fn unregister(&mut self, hotkey: i32) -> Result<(), ServiceError> {
        // SAFETY: the hotkey was registered on this window.
        unsafe { UnregisterHotKey(Some(self.hwnd), hotkey) }
            .map_err(|error| failed("UnregisterHotKey", error))
    }
}
