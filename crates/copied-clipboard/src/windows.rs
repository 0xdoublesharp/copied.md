use std::{ptr, slice, thread, time::Duration};

use copied_core::ClipboardPayload;
use windows::{
    Win32::{
        Foundation::{GlobalFree, HANDLE, HGLOBAL, HWND},
        System::{
            DataExchange::{
                CloseClipboard, EmptyClipboard, GetClipboardData, GetClipboardSequenceNumber,
                IsClipboardFormatAvailable, OpenClipboard, RegisterClipboardFormatW,
                SetClipboardData,
            },
            Memory::{GMEM_MOVEABLE, GlobalAlloc, GlobalLock, GlobalSize, GlobalUnlock},
        },
        UI::WindowsAndMessaging::{
            CreateWindowExW, DestroyWindow, HWND_MESSAGE, WINDOW_EX_STYLE, WINDOW_STYLE,
        },
    },
    core::{PCWSTR, w},
};

use crate::{
    Board, ClipboardSnapshot, Revision, WriteError, WrittenClipboard,
    bundle::{prepare_bundle, windows_html},
    require_revision, stable_snapshot,
};

const CF_UNICODETEXT: u32 = 13;
const OPEN_ATTEMPTS: usize = 8;

/// Win32 clipboard using eager, moveable global memory for every representation.
#[derive(Debug, Default)]
pub struct WinClipboard;

struct OwnerWindow(HWND);

#[allow(unsafe_code)]
impl OwnerWindow {
    fn new() -> Result<Self, WriteError> {
        // SAFETY: STATIC is a predefined class. This message-only window stays on the caller thread.
        unsafe {
            CreateWindowExW(
                WINDOW_EX_STYLE(0),
                w!("STATIC"),
                PCWSTR::null(),
                WINDOW_STYLE(0),
                0,
                0,
                0,
                0,
                Some(HWND_MESSAGE),
                None,
                None,
                None,
            )
        }
        .map(Self)
        .map_err(|error| WriteError::Unavailable(error.to_string()))
    }
}

impl Drop for OwnerWindow {
    #[allow(unsafe_code)]
    fn drop(&mut self) {
        // SAFETY: We own the window, and its guard never leaves the thread that created it.
        let _ = unsafe { DestroyWindow(self.0) };
    }
}

struct ClipboardLock;

impl ClipboardLock {
    #[allow(unsafe_code)]
    fn open(owner: Option<HWND>) -> Result<Self, WriteError> {
        for attempt in 1..=OPEN_ATTEMPTS {
            // SAFETY: Owner is either None for reads or a live window on this thread for writes.
            match unsafe { OpenClipboard(owner) } {
                Ok(()) => return Ok(Self),
                Err(error) if attempt == OPEN_ATTEMPTS => {
                    return Err(WriteError::Unavailable(error.to_string()));
                }
                Err(_) => thread::sleep(Duration::from_millis(5)),
            }
        }
        unreachable!("OPEN_ATTEMPTS is positive")
    }
}

impl Drop for ClipboardLock {
    #[allow(unsafe_code)]
    fn drop(&mut self) {
        // SAFETY: This guard closes a clipboard opened on the current thread.
        let _ = unsafe { CloseClipboard() };
    }
}

struct GlobalData(Option<HGLOBAL>);

impl GlobalData {
    #[allow(unsafe_code)]
    fn new(bytes: &[u8]) -> Result<Self, WriteError> {
        // SAFETY: The allocation is at least as large as the buffer copied below.
        let memory = unsafe { GlobalAlloc(GMEM_MOVEABLE, bytes.len().max(1)) }
            .map_err(|error| WriteError::Rejected(error.to_string()))?;
        let allocation = Self(Some(memory));
        // SAFETY: The allocation is live and owned by this guard.
        let target = unsafe { GlobalLock(memory) }.cast::<u8>();
        if target.is_null() {
            return Err(WriteError::Rejected(
                "could not lock clipboard memory".into(),
            ));
        }
        // SAFETY: The allocation is locked and independent from the source byte slice.
        unsafe {
            ptr::copy_nonoverlapping(bytes.as_ptr(), target, bytes.len());
            if bytes.is_empty() {
                *target = 0;
            }
            let _ = GlobalUnlock(memory);
        }
        Ok(allocation)
    }

    #[allow(unsafe_code)]
    fn publish(&mut self, format: u32, _lock: &ClipboardLock) -> Result<(), WriteError> {
        let memory = self.0.expect("each representation is published once");
        // SAFETY: ClipboardLock owns the open clipboard; memory is unlocked and moveable.
        unsafe { SetClipboardData(format, Some(HANDLE(memory.0))) }
            .map_err(|error| WriteError::Rejected(error.to_string()))?;
        self.0 = None; // The OS owns allocations accepted by SetClipboardData.
        Ok(())
    }
}

impl Drop for GlobalData {
    #[allow(unsafe_code)]
    fn drop(&mut self) {
        if let Some(memory) = self.0 {
            // SAFETY: Only allocations that never transferred to the OS reach this branch.
            let _ = unsafe { GlobalFree(Some(memory)) };
        }
    }
}

#[allow(unsafe_code)]
fn registered_format(name: PCWSTR) -> Result<u32, WriteError> {
    // SAFETY: All callers use statically NUL-terminated format names.
    let format = unsafe { RegisterClipboardFormatW(name) };
    if format == 0 {
        return Err(WriteError::Rejected(
            "could not register a clipboard format".into(),
        ));
    }
    Ok(format)
}

#[allow(unsafe_code)]
fn unicode_text(_lock: &ClipboardLock) -> Result<Option<String>, WriteError> {
    // SAFETY: The clipboard stays open for this function, protecting clipboard-owned handles.
    unsafe {
        if IsClipboardFormatAvailable(CF_UNICODETEXT).is_err() {
            return Ok(None);
        }
        let handle = GetClipboardData(CF_UNICODETEXT)
            .map_err(|error| WriteError::Unavailable(error.to_string()))?;
        let memory = HGLOBAL(handle.0);
        let capacity = GlobalSize(memory) / size_of::<u16>();
        if capacity == 0 {
            return Err(WriteError::Unavailable(
                "invalid Unicode clipboard allocation".into(),
            ));
        }
        let data = GlobalLock(memory).cast::<u16>();
        if data.is_null() {
            return Err(WriteError::Unavailable(
                "could not read clipboard memory".into(),
            ));
        }
        let units = slice::from_raw_parts(data, capacity);
        let text = units
            .iter()
            .position(|unit| *unit == 0)
            .and_then(|end| String::from_utf16(&units[..end]).ok());
        let _ = GlobalUnlock(memory);
        text.map(Some)
            .ok_or_else(|| WriteError::Unavailable("invalid Unicode clipboard text".into()))
    }
}

fn terminated(text: &str) -> Vec<u8> {
    let mut bytes = text.as_bytes().to_vec();
    bytes.push(0);
    bytes
}

impl Board for WinClipboard {
    #[allow(unsafe_code)]
    fn revision(&self) -> Result<Revision, WriteError> {
        // SAFETY: This API reads a sequence number and takes no handles or pointers.
        Ok(Revision(u64::from(unsafe { GetClipboardSequenceNumber() })))
    }

    fn read_text(&self) -> Result<ClipboardSnapshot, WriteError> {
        let lock = ClipboardLock::open(None)?;
        // Delayed rendering can advance the sequence on the first read.
        let snapshot = stable_snapshot(|| self.revision(), || unicode_text(&lock));
        if snapshot == Err(WriteError::Stale) {
            stable_snapshot(|| self.revision(), || unicode_text(&lock))
        } else {
            snapshot
        }
    }

    #[allow(unsafe_code)]
    fn write(
        &mut self,
        payload: &ClipboardPayload,
        expected: Option<Revision>,
    ) -> Result<WrittenClipboard, WriteError> {
        let bundle = prepare_bundle(payload)?;
        let types = bundle.types();
        if [Some(bundle.plain), bundle.html, bundle.rtf]
            .into_iter()
            .flatten()
            .any(|text| text.contains('\0'))
        {
            return Err(WriteError::Rejected(
                "Windows text representations cannot contain NUL".into(),
            ));
        }
        let plain: Vec<u8> = bundle
            .plain
            .encode_utf16()
            .chain([0])
            .flat_map(u16::to_le_bytes)
            .collect();
        let mut formats = vec![(CF_UNICODETEXT, GlobalData::new(&plain)?)];
        if let Some(html) = bundle.html {
            formats.push((
                registered_format(w!("HTML Format"))?,
                GlobalData::new(&windows_html(html))?,
            ));
        }
        if let Some(rtf) = bundle.rtf {
            formats.push((
                registered_format(w!("Rich Text Format"))?,
                GlobalData::new(&terminated(rtf))?,
            ));
        }
        if let Some(signal) = &bundle.signal_pickle {
            formats.push((
                registered_format(w!("Chromium Web Custom MIME Data Format"))?,
                GlobalData::new(signal)?,
            ));
        }
        let owner = OwnerWindow::new()?;
        let lock = ClipboardLock::open(Some(owner.0))?;
        require_revision(expected, self.revision()?)?;
        // SAFETY: This thread has the clipboard open with a live owner window.
        unsafe { EmptyClipboard() }.map_err(|error| WriteError::Rejected(error.to_string()))?;
        for (format, data) in &mut formats {
            data.publish(*format, &lock)?;
        }
        Ok(WrittenClipboard {
            types,
            revision: self.revision()?,
        })
    }
}
