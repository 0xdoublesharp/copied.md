use std::{
    collections::HashMap,
    ffi::c_void,
    fmt,
    panic::{AssertUnwindSafe, catch_unwind},
    ptr,
    sync::{
        Arc, LazyLock, Mutex,
        atomic::{AtomicU32, Ordering},
    },
};

use copied_config::{ShortcutSpec, w3c_to_carbon_key_code};
use objc2::MainThreadMarker;

use crate::{PlatformError, ShortcutBackend};

const SIGNATURE: u32 = u32::from_be_bytes(*b"CMDC");
const HOTKEY_EXISTS: i32 = -9878;
const NOT_HANDLED: i32 = -9874;
const KEYBOARD: u32 = u32::from_be_bytes(*b"keyb");
const PRESSED: u32 = 5;
const DIRECT_OBJECT: u32 = u32::from_be_bytes(*b"----");
const HOTKEY_ID: u32 = u32::from_be_bytes(*b"hkid");
const EXCLUSIVE: u32 = 1;

type Handler = extern "C" fn(*mut c_void, *mut c_void, *mut c_void) -> i32;
type Press = Arc<dyn Fn() + Send + Sync>;
static NEXT_ID: AtomicU32 = AtomicU32::new(1);
static CALLBACKS: LazyLock<Mutex<HashMap<u32, Press>>> = LazyLock::new(Mutex::default);

#[repr(C)]
struct EventType {
    class: u32,
    kind: u32,
}

#[repr(C)]
struct HotKeyId {
    signature: u32,
    id: u32,
}

#[allow(unsafe_code)]
#[link(name = "Carbon", kind = "framework")]
unsafe extern "C" {
    fn GetApplicationEventTarget() -> *mut c_void;
    fn InstallEventHandler(
        target: *mut c_void,
        handler: Handler,
        count: u32,
        events: *const EventType,
        context: *mut c_void,
        output: *mut *mut c_void,
    ) -> i32;
    fn RemoveEventHandler(handler: *mut c_void) -> i32;
    fn GetEventParameter(
        event: *mut c_void,
        name: u32,
        kind: u32,
        actual_kind: *mut u32,
        buffer_size: usize,
        actual_size: *mut usize,
        output: *mut c_void,
    ) -> i32;
    fn RegisterEventHotKey(
        key: u32,
        modifiers: u32,
        id: HotKeyId,
        target: *mut c_void,
        options: u32,
        output: *mut *mut c_void,
    ) -> i32;
    fn UnregisterEventHotKey(hotkey: *mut c_void) -> i32;
}

#[allow(unsafe_code)]
unsafe extern "C" {
    static _dispatch_main_q: c_void;
    fn dispatch_async_f(
        queue: *const c_void,
        context: *mut c_void,
        work: extern "C" fn(*mut c_void),
    );
}

#[allow(unsafe_code)]
extern "C" fn handle_event(_: *mut c_void, event: *mut c_void, _: *mut c_void) -> i32 {
    if event.is_null() {
        return NOT_HANDLED;
    }
    let mut id = HotKeyId {
        signature: 0,
        id: 0,
    };
    // SAFETY: Carbon supplied the event, and the stack buffer holds exactly one EventHotKeyID.
    let status = unsafe {
        GetEventParameter(
            event,
            DIRECT_OBJECT,
            HOTKEY_ID,
            ptr::null_mut(),
            size_of::<HotKeyId>(),
            ptr::null_mut(),
            (&raw mut id).cast(),
        )
    };
    if status != 0 || id.signature != SIGNATURE {
        return NOT_HANDLED;
    }
    let callback = CALLBACKS
        .lock()
        .ok()
        .and_then(|callbacks| callbacks.get(&id.id).cloned());
    if let Some(callback) = callback {
        let _ = catch_unwind(AssertUnwindSafe(|| callback()));
        return 0;
    }
    NOT_HANDLED
}

#[derive(Default)]
struct Handles {
    registration: usize,
    handler: usize,
}

impl Handles {
    #[allow(unsafe_code)]
    fn release(self) {
        if self.registration == 0 && self.handler == 0 {
            return;
        }
        let context = Box::into_raw(Box::new(self)).cast();
        if MainThreadMarker::new().is_some() {
            release_handles(context);
        } else {
            // SAFETY: The main dispatch queue owns the Box until release_handles runs there.
            unsafe { dispatch_async_f(&raw const _dispatch_main_q, context, release_handles) };
        }
    }
}

#[allow(unsafe_code)]
extern "C" fn release_handles(context: *mut c_void) {
    // SAFETY: Handles::release transfers this Box here exactly once, always on the main thread.
    let handles = unsafe { Box::from_raw(context.cast::<Handles>()) };
    // SAFETY: The handles came from Carbon and are removed on the application event thread.
    unsafe {
        if handles.registration != 0 {
            UnregisterEventHotKey(ptr::with_exposed_provenance_mut(handles.registration));
        }
        if handles.handler != 0 {
            RemoveEventHandler(ptr::with_exposed_provenance_mut(handles.handler));
        }
    }
}

/// Application-event-queue global hotkey. Construct and bind on the main thread.
/// The callback runs on that thread while the application's event loop is active.
pub struct MacHotKey {
    handler: usize,
    registration: usize,
    id: u32,
    current: Option<ShortcutSpec>,
    callback: Press,
}

impl fmt::Debug for MacHotKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("MacHotKey")
            .field("current", &self.current)
            .finish_non_exhaustive()
    }
}

impl MacHotKey {
    #[allow(unsafe_code)]
    pub fn new(callback: impl Fn() + Send + Sync + 'static) -> Result<Self, PlatformError> {
        require_main_thread()?;
        let mut handler = ptr::null_mut();
        let event = EventType {
            class: KEYBOARD,
            kind: PRESSED,
        };
        // Carbon distinguishes installations of the same handler by the context value.
        let context = ptr::without_provenance_mut(NEXT_ID.fetch_add(1, Ordering::Relaxed) as usize);
        // SAFETY: Called on the main thread with a valid callback, event spec, and out-parameter.
        let status = unsafe {
            InstallEventHandler(
                GetApplicationEventTarget(),
                handle_event,
                1,
                &raw const event,
                context,
                &raw mut handler,
            )
        };
        if status != 0 || handler.is_null() {
            return Err(PlatformError::ShortcutFailed(format!(
                "macOS event handler ({status})"
            )));
        }
        Ok(Self {
            handler: handler.expose_provenance(),
            registration: 0,
            id: 0,
            current: None,
            callback: Arc::new(callback),
        })
    }
}

fn require_main_thread() -> Result<(), PlatformError> {
    MainThreadMarker::new().map(|_| ()).ok_or_else(|| {
        PlatformError::ShortcutFailed(
            "Carbon hotkeys must be installed on the application main thread".into(),
        )
    })
}

fn carbon_registration(
    status: i32,
    has_handle: bool,
    shortcut: &ShortcutSpec,
) -> Result<(), PlatformError> {
    if status == HOTKEY_EXISTS {
        Err(PlatformError::ShortcutInUse(shortcut.format_macos()))
    } else if status == 0 && has_handle {
        Ok(())
    } else {
        Err(PlatformError::ShortcutFailed(format!(
            "{} ({status})",
            shortcut.format_macos()
        )))
    }
}

impl ShortcutBackend for MacHotKey {
    fn current(&self) -> Option<&ShortcutSpec> {
        self.current.as_ref()
    }

    #[allow(unsafe_code)]
    fn bind(&mut self, shortcut: &ShortcutSpec) -> Result<(), PlatformError> {
        require_main_thread()?;
        if self.current() == Some(shortcut) {
            return Ok(());
        }
        let key = w3c_to_carbon_key_code(&shortcut.code)
            .ok_or_else(|| PlatformError::ShortcutFailed(shortcut.format_macos()))?;
        let modifiers = [
            (shortcut.meta, 256),
            (shortcut.shift, 512),
            (shortcut.alt, 2048),
            (shortcut.ctrl, 4096),
        ]
        .into_iter()
        .filter_map(|(enabled, mask)| enabled.then_some(mask))
        .fold(0, |all, mask| all | mask);
        let id = NEXT_ID.fetch_add(1, Ordering::Relaxed);
        let mut replacement = ptr::null_mut();
        // SAFETY: Called on the main thread with the application's Carbon target and a valid out-parameter.
        let status = unsafe {
            RegisterEventHotKey(
                key,
                modifiers,
                HotKeyId {
                    signature: SIGNATURE,
                    id,
                },
                GetApplicationEventTarget(),
                EXCLUSIVE,
                &raw mut replacement,
            )
        };
        carbon_registration(status, !replacement.is_null(), shortcut)?;
        self.unbind();
        CALLBACKS
            .lock()
            .expect("hotkey callback map poisoned")
            .insert(id, self.callback.clone());
        self.id = id;
        self.registration = replacement.expose_provenance();
        self.current = Some(shortcut.clone());
        Ok(())
    }

    fn unbind(&mut self) {
        if self.registration != 0 {
            CALLBACKS
                .lock()
                .expect("hotkey callback map poisoned")
                .remove(&self.id);
            Handles {
                registration: std::mem::take(&mut self.registration),
                handler: 0,
            }
            .release();
        }
        self.current = None;
    }
}

impl Drop for MacHotKey {
    fn drop(&mut self) {
        self.unbind();
        Handles {
            registration: 0,
            handler: self.handler,
        }
        .release();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn construction_off_the_main_thread_is_rejected() {
        let result = std::thread::spawn(|| MacHotKey::new(|| {})).join().unwrap();
        assert!(matches!(result, Err(PlatformError::ShortcutFailed(_))));
    }

    #[test]
    #[allow(unsafe_code)]
    fn carbon_event_dispatches_only_to_the_current_registration() {
        unsafe extern "C" {
            fn CreateEvent(
                allocator: *const c_void,
                class: u32,
                kind: u32,
                time: f64,
                attributes: u32,
                out: *mut *mut c_void,
            ) -> i32;
            fn SetEventParameter(
                event: *mut c_void,
                name: u32,
                kind: u32,
                length: usize,
                value: *const c_void,
            ) -> i32;
            fn ReleaseEvent(event: *mut c_void);
        }

        let id = NEXT_ID.fetch_add(1, Ordering::Relaxed);
        let key = HotKeyId {
            signature: SIGNATURE,
            id,
        };
        let mut event = ptr::null_mut();
        // SAFETY: Carbon creates a private event and copies the typed EventHotKeyID from the stack.
        unsafe {
            assert_eq!(
                CreateEvent(ptr::null(), KEYBOARD, PRESSED, 0.0, 0, &raw mut event),
                0
            );
            assert_eq!(
                SetEventParameter(
                    event,
                    DIRECT_OBJECT,
                    HOTKEY_ID,
                    size_of::<HotKeyId>(),
                    (&raw const key).cast()
                ),
                0
            );
        }
        let pressed = Arc::new(AtomicU32::new(0));
        CALLBACKS.lock().unwrap().insert(id, {
            let pressed = pressed.clone();
            Arc::new(move || {
                pressed.fetch_add(1, Ordering::Relaxed);
            })
        });
        let response = handle_event(ptr::null_mut(), event, ptr::null_mut());
        CALLBACKS.lock().unwrap().remove(&id);
        let stale = handle_event(ptr::null_mut(), event, ptr::null_mut());
        // SAFETY: This test owns the private event and no callback holds it.
        unsafe { ReleaseEvent(event) };
        assert_eq!(response, 0);
        assert_eq!(stale, NOT_HANDLED);
        assert_eq!(pressed.load(Ordering::Relaxed), 1);
    }

    #[test]
    fn registration_errors_remain_distinct() {
        let shortcut = ShortcutSpec::parse("Ctrl+Meta+C").unwrap();
        assert!(matches!(
            carbon_registration(HOTKEY_EXISTS, false, &shortcut),
            Err(PlatformError::ShortcutInUse(_))
        ));
        assert!(matches!(
            carbon_registration(0, false, &shortcut),
            Err(PlatformError::ShortcutFailed(_))
        ));
        assert_eq!(carbon_registration(0, true, &shortcut), Ok(()));
    }
}
