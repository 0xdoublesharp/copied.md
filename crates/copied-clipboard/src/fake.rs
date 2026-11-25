use copied_core::ClipboardPayload;

use crate::{
    Board, ClipboardSnapshot, Revision, WriteError, WrittenClipboard, bundle::prepare_bundle,
    require_revision,
};

/// Deterministic in-memory clipboard with one-shot interleaving hooks.
#[derive(Debug, Default, Clone)]
pub struct FakeBoard {
    revision: u64,
    payload: Option<ClipboardPayload>,
    pub on_before_clear: Option<fn(&mut Self)>,
    pub on_after_clear: Option<fn(&mut Self)>,
    pub fail_write: bool,
    pub fail_prepare: bool,
    write_calls: usize,
}

impl FakeBoard {
    pub fn with_text(text: impl Into<String>) -> Self {
        Self {
            payload: Some(ClipboardPayload {
                plain: text.into(),
                ..ClipboardPayload::default()
            }),
            ..Self::default()
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

    /// Counts attempts to publish prepared contents, including rejected attempts.
    pub fn write_calls(&self) -> usize {
        self.write_calls
    }
}

impl Board for FakeBoard {
    fn revision(&self) -> Result<Revision, WriteError> {
        Ok(Revision(self.revision))
    }

    fn read_text(&self) -> Result<ClipboardSnapshot, WriteError> {
        let payload = self.payload.as_ref().ok_or(WriteError::Empty)?;
        Ok(ClipboardSnapshot {
            text: payload.plain.clone(),
            revision: Revision(self.revision),
        })
    }

    fn read_html(&self) -> Result<Option<ClipboardSnapshot>, WriteError> {
        Ok(self
            .payload
            .as_ref()
            .and_then(|payload| payload.html.clone())
            .map(|text| ClipboardSnapshot {
                text,
                revision: Revision(self.revision),
            }))
    }

    fn write(
        &mut self,
        payload: &ClipboardPayload,
        expected: Option<Revision>,
    ) -> Result<WrittenClipboard, WriteError> {
        let bundle = prepare_bundle(payload)?;
        if self.fail_prepare {
            return Err(WriteError::Rejected(
                "clipboard flavors could not be prepared".into(),
            ));
        }
        if let Some(hook) = self.on_before_clear.take() {
            hook(self);
        }
        require_revision(expected, Revision(self.revision))?;
        self.revision += 1;
        self.payload = None;
        let prepared = Revision(self.revision);
        if let Some(hook) = self.on_after_clear.take() {
            hook(self);
        }
        require_revision(Some(prepared), Revision(self.revision))?;
        self.write_calls += 1;
        if self.fail_write {
            return Err(WriteError::Rejected("clipboard write failed".into()));
        }
        self.payload = Some(ClipboardPayload {
            plain: bundle.plain.to_owned(),
            html: bundle.html.map(str::to_owned),
            rtf: bundle.rtf.map(str::to_owned),
            signal: payload.signal.clone().filter(|value| !value.is_empty()),
        });
        Ok(WrittenClipboard {
            types: bundle.types(),
            revision: prepared,
        })
    }
}

#[cfg(test)]
mod tests;
