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

/// Deterministic in-memory [`Board`] for unit tests, property tests, and state-machine verification.
#[derive(Debug, Default, Clone)]
pub struct FakeBoard {
    revision: u64,
    payload: Option<ClipboardPayload>,
}

impl FakeBoard {
    pub fn with_text(text: impl Into<String>) -> Self {
        Self {
            revision: 0,
            payload: Some(ClipboardPayload {
                plain: text.into(),
                ..ClipboardPayload::default()
            }),
        }
    }

    pub fn external_copy(&mut self, text: impl Into<String>) -> Revision {
        self.revision += 1;
        self.payload = Some(ClipboardPayload {
            plain: text.into(),
            ..ClipboardPayload::default()
        });
        Revision(self.revision)
    }

    pub fn current_payload(&self) -> Option<&ClipboardPayload> {
        self.payload.as_ref()
    }
}

impl Board for FakeBoard {
    fn revision(&self) -> Result<Revision, WriteError> {
        Ok(Revision(self.revision))
    }

    fn read_text(&self) -> Result<ClipboardSnapshot, WriteError> {
        let Some(payload) = &self.payload else {
            return Err(WriteError::Empty);
        };
        Ok(ClipboardSnapshot {
            text: payload.plain.clone(),
            revision: Revision(self.revision),
        })
    }

    fn write(
        &mut self,
        payload: &ClipboardPayload,
        expected: Option<Revision>,
    ) -> Result<WrittenClipboard, WriteError> {
        if payload.plain.trim().is_empty() {
            return Err(WriteError::Empty);
        }
        if let Some(exp) = expected
            && exp != Revision(self.revision)
        {
            return Err(WriteError::Stale);
        }
        let mut types = vec![String::from("text/plain")];
        if payload.html.as_deref().is_some_and(|s| !s.is_empty()) {
            types.push(String::from("text/html"));
        }
        if payload.rtf.as_deref().is_some_and(|s| !s.is_empty()) {
            types.push(String::from("text/rtf"));
        }
        if payload.signal.as_deref().is_some_and(|s| !s.is_empty()) {
            types.push(String::from("text/signal"));
        }
        self.revision += 1;
        self.payload = Some(payload.clone());
        Ok(WrittenClipboard {
            types,
            revision: Revision(self.revision),
        })
    }
}
