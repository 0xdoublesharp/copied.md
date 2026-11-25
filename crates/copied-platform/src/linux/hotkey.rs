use std::{
    sync::mpsc::{self, Receiver, RecvTimeoutError, Sender},
    thread::{self, JoinHandle},
    time::Duration,
};

use copied_clipboard::linux::{Session, desktop_session};
use copied_config::ShortcutSpec;
use x11rb::{
    connection::Connection,
    errors::ReplyError,
    protocol::{
        ErrorKind, Event,
        xproto::{ConnectionExt, GrabMode, ModMask, Window},
    },
    rust_connection::RustConnection,
};

use crate::{PlatformError, ShortcutBackend};

enum Command {
    Bind(ShortcutSpec, Sender<Result<(), PlatformError>>),
    Unbind(Sender<()>),
}

/// X11 passive key grabs, serviced on an owned connection and event thread.
/// The callback should enqueue work and return promptly.
#[derive(Debug)]
pub struct X11HotKey {
    commands: Option<Sender<Command>>,
    worker: Option<JoinHandle<()>>,
    current: Option<ShortcutSpec>,
}

impl X11HotKey {
    pub fn new(callback: impl Fn() + Send + 'static) -> Result<Self, PlatformError> {
        if desktop_session() != Session::X11 {
            return Err(PlatformError::Unsupported(
                "direct global shortcuts require X11".into(),
            ));
        }
        let (connection, screen) = x11rb::connect(None).map_err(shortcut_error)?;
        let root = connection.setup().roots[screen].root;
        let (commands, receiver) = mpsc::channel();
        let worker = thread::Builder::new()
            .name("copied-x11-hotkey".into())
            .spawn(move || run(&connection, root, &receiver, &callback))
            .map_err(shortcut_error)?;
        Ok(Self {
            commands: Some(commands),
            worker: Some(worker),
            current: None,
        })
    }
}

fn shortcut_error(error: impl std::fmt::Display) -> PlatformError {
    PlatformError::ShortcutFailed(error.to_string())
}

fn stopped() -> PlatformError {
    shortcut_error("X11 hotkey worker stopped")
}

impl ShortcutBackend for X11HotKey {
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
            .ok_or_else(stopped)?
            .send(Command::Bind(shortcut.clone(), reply))
            .map_err(|_| stopped())?;
        response.recv().map_err(|_| stopped())??;
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

impl Drop for X11HotKey {
    fn drop(&mut self) {
        self.commands.take();
        if let Some(worker) = self.worker.take()
            && worker.thread().id() != thread::current().id()
        {
            let _ = worker.join();
        }
    }
}

fn keysym(code: &str) -> Option<u32> {
    match code.as_bytes() {
        [b'K', b'e', b'y', key @ b'A'..=b'Z'] => Some(u32::from(key.to_ascii_lowercase())),
        [b'D', b'i', b'g', b'i', b't', key @ b'0'..=b'9'] => Some(u32::from(*key)),
        _ => {
            let function = code.strip_prefix('F')?.parse::<u32>().ok()?;
            (1..=35).contains(&function).then(|| 0xffbd + function)
        }
    }
}

#[derive(PartialEq, Eq)]
struct Grab {
    key: u8,
    mask: u16,
}

fn requested_grabs(
    connection: &RustConnection,
    shortcut: &ShortcutSpec,
) -> Result<Vec<Grab>, PlatformError> {
    let symbol =
        keysym(&shortcut.code).ok_or_else(|| shortcut_error(shortcut.format_portable()))?;
    let min = connection.setup().min_keycode;
    let count = connection.setup().max_keycode - min + 1;
    let keyboard = connection
        .get_keyboard_mapping(min, count)
        .map_err(shortcut_error)?
        .reply()
        .map_err(shortcut_error)?;
    let width = usize::from(keyboard.keysyms_per_keycode);
    if width == 0 {
        return Err(shortcut_error("empty X11 keyboard mapping"));
    }
    let key = keyboard
        .keysyms
        .chunks(width)
        .position(|symbols| symbols.first() == Some(&symbol))
        .and_then(|offset| u8::try_from(offset).ok())
        .map(|offset| min + offset)
        .ok_or_else(|| {
            shortcut_error(format!("{} is absent from the X11 keymap", shortcut.code))
        })?;
    let map = connection
        .get_modifier_mapping()
        .map_err(shortcut_error)?
        .reply()
        .map_err(shortcut_error)?;
    let modifier_width = usize::from(map.keycodes_per_modifier());
    if modifier_width == 0 {
        return Err(shortcut_error("empty X11 modifier mapping"));
    }
    let modifier = |symbols: &[u32]| -> u16 {
        map.keycodes
            .chunks(modifier_width)
            .enumerate()
            .find_map(|(index, keys)| {
                keys.iter()
                    .filter_map(|key| key.checked_sub(min))
                    .any(|offset| {
                        let start = usize::from(offset) * width;
                        keyboard
                            .keysyms
                            .get(start..start + width)
                            .is_some_and(|keysyms| {
                                keysyms.iter().any(|value| symbols.contains(value))
                            })
                    })
                    .then_some(1 << index)
            })
            .unwrap_or(0)
    };
    let mut mask = 0;
    for (enabled, bits) in [
        (shortcut.ctrl, u16::from(ModMask::CONTROL)),
        (shortcut.shift, u16::from(ModMask::SHIFT)),
        (shortcut.alt, modifier(&[0xffe9, 0xffea])), // Alt_L, Alt_R
        (shortcut.meta, modifier(&[0xffeb, 0xffec])), // Super_L, Super_R
    ] {
        if enabled && (bits == 0 || mask & bits != 0) {
            return Err(shortcut_error(
                "requested X11 modifiers have no distinct mapping",
            ));
        }
        if enabled {
            mask |= bits;
        }
    }
    let mut variants = vec![mask];
    for lock in [
        u16::from(ModMask::LOCK),
        modifier(&[0xff7f]),
        modifier(&[0xff14]),
    ] {
        if lock != 0 && variants.iter().all(|variant| variant & lock == 0) {
            variants.extend(variants.clone().into_iter().map(|mask| mask | lock));
        }
    }
    Ok(variants
        .into_iter()
        .map(|mask| Grab { key, mask })
        .collect())
}

fn ungrab(connection: &RustConnection, root: Window, grabs: impl IntoIterator<Item = Grab>) {
    for grab in grabs {
        if let Ok(cookie) = connection.ungrab_key(grab.key, root, grab.mask.into()) {
            let _ = cookie.check();
        }
    }
}

fn bind(
    connection: &RustConnection,
    root: Window,
    old: &mut Vec<Grab>,
    shortcut: &ShortcutSpec,
) -> Result<(), PlatformError> {
    let requested = requested_grabs(connection, shortcut)?;
    let mut installed = Vec::new();
    for grab in requested.iter().filter(|grab| !old.contains(grab)) {
        let result = connection
            .grab_key(
                false,
                root,
                grab.mask.into(),
                grab.key,
                GrabMode::ASYNC,
                GrabMode::ASYNC,
            )
            .map_err(ReplyError::from)
            .and_then(x11rb::cookie::VoidCookie::check);
        if let Err(error) = result {
            ungrab(connection, root, installed);
            return Err(match error {
                ReplyError::X11Error(ref native) if native.error_kind == ErrorKind::Access => {
                    PlatformError::ShortcutInUse(shortcut.format_portable())
                }
                _ => shortcut_error(error),
            });
        }
        installed.push(Grab {
            key: grab.key,
            mask: grab.mask,
        });
    }
    ungrab(
        connection,
        root,
        std::mem::replace(old, requested)
            .into_iter()
            .filter(|grab| !old.contains(grab)),
    );
    Ok(())
}

fn run(
    connection: &RustConnection,
    root: Window,
    commands: &Receiver<Command>,
    callback: &impl Fn(),
) {
    let mut grabs = Vec::new();
    loop {
        for _ in 0..64 {
            match connection.poll_for_event() {
                Ok(Some(Event::KeyPress(event)))
                    if grabs.iter().any(|grab: &Grab| {
                        grab.key == event.detail && grab.mask == u16::from(event.state) & 0xff
                    }) =>
                {
                    let _ = std::panic::catch_unwind(std::panic::AssertUnwindSafe(callback));
                }
                Ok(Some(_)) => {}
                Ok(None) => break,
                Err(_) => return,
            }
        }
        match commands.recv_timeout(Duration::from_millis(10)) {
            Ok(Command::Bind(shortcut, result)) => {
                let _ = result.send(bind(connection, root, &mut grabs, &shortcut));
            }
            Ok(Command::Unbind(reply)) => {
                ungrab(connection, root, std::mem::take(&mut grabs));
                let _ = reply.send(());
            }
            Err(RecvTimeoutError::Timeout) => {}
            Err(RecvTimeoutError::Disconnected) => break,
        }
    }
    ungrab(connection, root, grabs);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn maps_x11_keysyms() {
        assert_eq!(keysym("KeyC"), Some(0x63));
        assert_eq!(keysym("Digit0"), Some(0x30));
        assert_eq!(keysym("F19"), Some(0xffd0));
        assert_eq!(keysym("Space"), None);
    }
}
