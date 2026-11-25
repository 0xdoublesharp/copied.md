use copied_core::ClipboardPayload;
use serde::{Deserialize, Serialize};
use thiserror::Error;

/// Opaque monotone clipboard generation token (maps to `NSPasteboard.changeCount`, Win32 sequence number, `XFixes` revision).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct Revision(pub u64);

/// Captured clipboard text plus its generation token.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ClipboardSnapshot {
    pub text: String,
    pub revision: Revision,
}

/// Result of a committed clipboard write.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WrittenClipboard {
    pub types: Vec<String>,
    pub revision: Revision,
}

/// Errors produced by clipboard reads and stale-guarded writes.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum WriteError {
    #[error("The clipboard changed. Copy the Markdown again.")]
    Stale,
    #[error("There is no text to copy.")]
    Empty,
    #[error("Clipboard write rejected: {0}")]
    Rejected(String),
    #[error("Clipboard unavailable: {0}")]
    Unavailable(String),
}

/// Cross-platform clipboard abstraction owned by `copied-clipboard`.
pub trait Board: Send {
    fn revision(&self) -> Result<Revision, WriteError>;
    fn read_text(&self) -> Result<ClipboardSnapshot, WriteError>;
    fn write(
        &mut self,
        payload: &ClipboardPayload,
        expected: Option<Revision>,
    ) -> Result<WrittenClipboard, WriteError>;
}

mod bundle;
mod fake;
#[cfg(target_os = "linux")]
pub mod linux;
#[cfg(target_os = "macos")]
pub mod macos;
#[cfg(target_os = "windows")]
pub mod windows;

pub use fake::FakeBoard;

/// Opens the clipboard for the current desktop session.
pub fn system_board() -> Result<Box<dyn Board>, WriteError> {
    #[cfg(target_os = "macos")]
    return Ok(Box::new(macos::MacPasteboard::general()));
    #[cfg(target_os = "windows")]
    return Ok(Box::new(windows::WinClipboard));
    #[cfg(target_os = "linux")]
    return Ok(Box::new(linux::LinuxClipboard::connect()?));
    #[cfg(not(any(target_os = "macos", target_os = "windows", target_os = "linux")))]
    Err(WriteError::Unavailable(
        "no system clipboard backend".into(),
    ))
}

fn require_revision(expected: Option<Revision>, actual: Revision) -> Result<(), WriteError> {
    if expected.is_some_and(|revision| revision != actual) {
        return Err(WriteError::Stale);
    }
    Ok(())
}

#[cfg(any(test, target_os = "macos", target_os = "windows"))]
fn stable_snapshot(
    revision: impl Fn() -> Result<Revision, WriteError>,
    read: impl FnOnce() -> Result<Option<String>, WriteError>,
) -> Result<ClipboardSnapshot, WriteError> {
    let initial = revision()?;
    let text = read();
    require_revision(Some(initial), revision()?)?;
    Ok(ClipboardSnapshot {
        text: text?.ok_or(WriteError::Empty)?,
        revision: initial,
    })
}
