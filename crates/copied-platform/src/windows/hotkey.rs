use std::{
    sync::mpsc::{self, Receiver, RecvTimeoutError, Sender},
    thread::{self, JoinHandle},
    time::Duration,
};

use copied_config::ShortcutSpec;
use windows::{
    Win32::{
        Foundation::ERROR_HOTKEY_ALREADY_REGISTERED,
        UI::{
            Input::KeyboardAndMouse::{
                HOT_KEY_MODIFIERS, MOD_ALT, MOD_CONTROL, MOD_NOREPEAT, MOD_SHIFT, MOD_WIN,
                RegisterHotKey, UnregisterHotKey,
            },
            WindowsAndMessaging::{MSG, PM_REMOVE, PeekMessageW, WM_HOTKEY},
        },
    },
    core::HRESULT,
};

use crate::{PlatformError, ShortcutBackend};

enum Command {
    Bind(ShortcutSpec, Sender<Result<(), PlatformError>>),
    Unbind(Sender<()>),
}

/// Direct Win32 global shortcut registration on an owned message-loop thread.
/// The callback should enqueue work and return promptly.
#[derive(Debug)]
pub struct WinHotKey {
    commands: Option<Sender<Command>>,
    worker: Option<JoinHandle<()>>,
    current: Option<ShortcutSpec>,
}

impl WinHotKey {
    pub fn new(callback: impl Fn() + Send + 'static) -> Result<Self, PlatformError> {
        let (commands, receiver) = mpsc::channel();
        let worker = thread::Builder::new()
            .name("copied-windows-hotkey".into())
            .spawn(move || run(&receiver, &callback))
            .map_err(|error| PlatformError::ShortcutFailed(error.to_string()))?;
        Ok(Self {
            commands: Some(commands),
            worker: Some(worker),
            current: None,
        })
    }
}

fn worker_stopped() -> PlatformError {
    PlatformError::ShortcutFailed("Windows hotkey worker stopped".into())
}

impl ShortcutBackend for WinHotKey {
    fn current(&self) -> Option<&ShortcutSpec> {
        self.current.as_ref()
    }

    fn bind(&mut self, shortcut: &ShortcutSpec) -> Result<(), PlatformError> {
        if self.current() == Some(shortcut) {
            return Ok(());
        }
        let (reply, response) = mpsc::channel();
        self.commands
            .as_ref()
            .ok_or_else(worker_stopped)?
            .send(Command::Bind(shortcut.clone(), reply))
            .map_err(|_| worker_stopped())?;
        response.recv().map_err(|_| worker_stopped())??;
        self.current = Some(shortcut.clone());
        Ok(())
    }

    fn unbind(&mut self) {
        let (reply, response) = mpsc::channel();
        if self
            .commands
            .as_ref()
            .is_some_and(|commands| commands.send(Command::Unbind(reply)).is_ok())
        {
            let _ = response.recv();
        }
        self.current = None;
    }
}

impl Drop for WinHotKey {
    fn drop(&mut self) {
        self.commands.take();
        if let Some(worker) = self.worker.take()
            && worker.thread().id() != thread::current().id()
        {
            let _ = worker.join();
        }
    }
}

fn virtual_key(code: &str) -> Option<u32> {
    match code.as_bytes() {
        [b'K', b'e', b'y', key @ b'A'..=b'Z']
        | [b'D', b'i', b'g', b'i', b't', key @ b'0'..=b'9'] => Some(u32::from(*key)),
        _ => {
            let function = code.strip_prefix('F')?.parse::<u32>().ok()?;
            (1..=24).contains(&function).then(|| 0x6f + function)
        }
    }
}

#[allow(unsafe_code)]
fn register(shortcut: &ShortcutSpec, id: i32) -> Result<(), PlatformError> {
    let key = virtual_key(&shortcut.code)
        .ok_or_else(|| PlatformError::ShortcutFailed(shortcut.format_portable()))?;
    let flags = [
        (shortcut.alt, MOD_ALT),
        (shortcut.ctrl, MOD_CONTROL),
        (shortcut.shift, MOD_SHIFT),
        (shortcut.meta, MOD_WIN),
    ]
    .into_iter()
    .fold(
        MOD_NOREPEAT,
        |flags, (on, bit)| {
            if on { flags | bit } else { flags }
        },
    );
    // SAFETY: This function runs on the worker that owns the thread's hotkey message queue.
    unsafe { RegisterHotKey(None, id, HOT_KEY_MODIFIERS(flags.0), key) }.map_err(|error| {
        if error.code() == HRESULT::from_win32(ERROR_HOTKEY_ALREADY_REGISTERED.0) {
            PlatformError::ShortcutInUse(shortcut.format_portable())
        } else {
            PlatformError::ShortcutFailed(format!("{} ({error})", shortcut.format_portable()))
        }
    })
}

#[allow(unsafe_code)]
fn remove(id: Option<i32>) {
    if let Some(id) = id {
        // SAFETY: This worker owns the thread hotkey with this ID.
        let _ = unsafe { UnregisterHotKey(None, id) };
    }
}

#[allow(unsafe_code)]
fn run(commands: &Receiver<Command>, callback: &impl Fn()) {
    let mut registration = None;
    let mut next = 1..=0xbfff;
    loop {
        let mut event = MSG::default();
        // SAFETY: The mutable MSG is valid; this reads only this thread's WM_HOTKEY messages.
        while unsafe { PeekMessageW(&raw mut event, None, WM_HOTKEY, WM_HOTKEY, PM_REMOVE) }
            .as_bool()
        {
            if i32::try_from(event.wParam.0).ok() == registration {
                let _ = std::panic::catch_unwind(std::panic::AssertUnwindSafe(callback));
            }
        }
        match commands.recv_timeout(Duration::from_millis(10)) {
            Ok(Command::Bind(shortcut, result)) => {
                let id = next.next().unwrap_or_else(|| {
                    next = 2..=0xbfff;
                    1
                });
                let status = register(&shortcut, id);
                if status.is_ok() {
                    remove(registration.replace(id));
                }
                let _ = result.send(status);
            }
            Ok(Command::Unbind(reply)) => {
                remove(registration.take());
                let _ = reply.send(());
            }
            Err(RecvTimeoutError::Timeout) => {}
            Err(RecvTimeoutError::Disconnected) => break,
        }
    }
    remove(registration);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn maps_supported_virtual_keys() {
        assert_eq!(virtual_key("KeyQ"), Some(0x51));
        assert_eq!(virtual_key("Digit9"), Some(0x39));
        assert_eq!(virtual_key("F19"), Some(0x82));
        assert_eq!(virtual_key("F25"), None);
    }
}
