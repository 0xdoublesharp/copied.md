use copied_clipboard::{Board, Revision, WriteError};
use copied_config::{ConfigV2, ShortcutCopyMode, resolve_shortcut_target, visible_formats};
use copied_core::{
    ClipboardPayload, ConversionOptions, CopyTarget, CopyTargetDefinition, DiagnosticSeverity,
    MarkdownConversion, convert_markdown, html_to_markdown,
};
use serde::{Deserialize, Serialize};
use thiserror::Error;

mod shortcut;
pub use shortcut::{ShortcutController, ShortcutError};

/// Monotone identifier tagging each picker session so stale UI replies are ignored.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct OpId(pub u64);

/// Summary of an automatic copy already performed before opening the picker.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CopiedSummary {
    pub target: CopyTarget,
    pub label: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub warning: Option<String>,
}

/// Immutable snapshot delivered to the picker UI when opening.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PickerOpenRequest {
    pub op_id: OpId,
    pub visible_targets: Vec<CopyTargetDefinition>,
    pub columns: u8,
    pub selected: CopyTarget,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub copied: Option<CopiedSummary>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub suggested: Option<CopyTarget>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source_preview: Option<String>,
}

/// Picker window controller (never called in `ShortcutCopyMode::CopyOnly`).
pub trait PickerHost: Send {
    fn open(&mut self, request: PickerOpenRequest);
    fn close(&mut self, op_id: OpId);
}

/// Native feedback channel (tray icon highlight + native warnings/errors).
pub trait Notifier: Send {
    fn mark_copied(&mut self, target: CopyTarget);
    fn warn(&mut self, title: &str, detail: &str);
    fn error(&mut self, message: &str);
}

/// Errors returned by engine operations.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum EngineError {
    #[error("an operation is already in progress")]
    Busy,
    #[error("stale picker operation id")]
    StaleOpId,
    #[error("picker operation ids are exhausted")]
    OpIdExhausted,
    #[error("clipboard error: {0}")]
    Clipboard(#[from] WriteError),
}

/// Active picker session state retaining the original captured Markdown.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ActiveSession {
    pub op_id: OpId,
    pub original_markdown: String,
    pub expected_revision: Revision,
    pub auto_copied: Option<CopyTarget>,
}

/// The original Markdown and the clipboard generation that holds its latest conversion.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RecentSource {
    pub markdown: String,
    pub written_revision: Revision,
    pub target: CopyTarget,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CopyOutcome {
    Copied(CopiedSummary),
    PickerOpened(OpId),
    /// The picker was cancelled or confirmed the automatic copy.
    Unchanged,
}

pub type Converter = fn(&str, CopyTarget, &ConversionOptions) -> MarkdownConversion;

const SOURCE_PREVIEW_BYTES: usize = 2_000;

/// Single-flight clipboard operations backed by a revision-checked board.
#[derive(Debug)]
pub struct CopyEngine<B: Board, P: PickerHost, N: Notifier> {
    pub board: B,
    pub picker: P,
    pub notifier: N,
    pub config: ConfigV2,
    pub next_op_id: u64,
    pub active: Option<ActiveSession>,
    last_source: Option<RecentSource>,
    converter: Converter,
}

impl<B: Board, P: PickerHost, N: Notifier> CopyEngine<B, P, N> {
    pub fn new(board: B, picker: P, notifier: N, config: ConfigV2) -> Self {
        Self {
            board,
            picker,
            notifier,
            config,
            next_op_id: 1,
            active: None,
            last_source: None,
            converter: convert_markdown,
        }
    }

    #[must_use]
    pub fn with_converter(mut self, converter: Converter) -> Self {
        self.converter = converter;
        self
    }

    pub fn is_busy(&self) -> bool {
        self.active.is_some()
    }

    pub fn mode(&self) -> ShortcutCopyMode {
        self.config.shortcut_mode
    }

    pub fn last_source(&self) -> Option<&RecentSource> {
        self.last_source.as_ref()
    }

    pub fn start_copy(
        &mut self,
        mode: ShortcutCopyMode,
        app_id: Option<&str>,
    ) -> Result<CopyOutcome, EngineError> {
        if self.is_busy() {
            return Err(EngineError::Busy);
        }
        let snapshot = self
            .board
            .read_text()
            .and_then(|mut snapshot| {
                if let Some(recent) = self
                    .last_source()
                    .filter(|recent| recent.written_revision == snapshot.revision)
                {
                    snapshot.text.clone_from(&recent.markdown);
                }
                if snapshot.text.trim().is_empty() {
                    Err(WriteError::Empty)
                } else {
                    Ok(snapshot)
                }
            })
            .inspect_err(|error| self.notifier.error(&error.to_string()))?;
        let resolved = resolve_shortcut_target(&self.config, app_id);
        let target = resolved.target;
        if mode == ShortcutCopyMode::CopyOnly {
            let (copied, _) = self.copy(
                &snapshot.text,
                target,
                snapshot.revision,
                !resolved.from_pin,
            )?;
            self.present_warning(&copied);
            return Ok(CopyOutcome::Copied(copied));
        }

        let op_id = OpId(self.next_op_id);
        let next_op_id = op_id.0.checked_add(1).ok_or(EngineError::OpIdExhausted)?;
        let (copied, suggested, expected_revision) = if mode == ShortcutCopyMode::CopyAndPick {
            let (summary, revision) = self.copy(
                &snapshot.text,
                target,
                snapshot.revision,
                !resolved.from_pin,
            )?;
            (Some(summary), None, revision)
        } else {
            (None, resolved.from_pin.then_some(target), snapshot.revision)
        };

        let preview_end = snapshot.text.floor_char_boundary(SOURCE_PREVIEW_BYTES);
        let source_preview = Some(snapshot.text[..preview_end].to_owned());
        self.next_op_id = next_op_id;
        self.active = Some(ActiveSession {
            op_id,
            original_markdown: snapshot.text,
            expected_revision,
            auto_copied: copied.as_ref().map(|summary| summary.target),
        });
        self.picker.open(PickerOpenRequest {
            op_id,
            visible_targets: visible_formats(&self.config.format_visibility)
                .into_iter()
                .cloned()
                .collect(),
            columns: self.config.picker_columns,
            selected: target,
            copied,
            suggested,
            source_preview,
        });
        Ok(CopyOutcome::PickerOpened(op_id))
    }

    pub fn handle_picker_choice(
        &mut self,
        op_id: OpId,
        choice: Option<CopyTarget>,
    ) -> Result<CopyOutcome, EngineError> {
        let session = self.close_picker(op_id)?;
        let Some(target) = choice.filter(|&target| Some(target) != session.auto_copied) else {
            return Ok(CopyOutcome::Unchanged);
        };
        let (copied, _) = self.copy(
            &session.original_markdown,
            target,
            session.expected_revision,
            true,
        )?;
        self.present_warning(&copied);
        Ok(CopyOutcome::Copied(copied))
    }

    pub fn handle_picker_dismiss(&mut self, op_id: OpId) -> Result<(), EngineError> {
        self.close_picker(op_id)?;
        Ok(())
    }

    /// Converts the clipboard's HTML representation and replaces it with Markdown text.
    pub fn convert_html_to_markdown(&mut self) -> Result<String, EngineError> {
        if self.is_busy() {
            return Err(EngineError::Busy);
        }
        let (markdown, revision) = self
            .board
            .read_html()
            .and_then(|snapshot| {
                let snapshot = snapshot.ok_or(WriteError::Empty)?;
                let markdown = html_to_markdown(&snapshot.text);
                if markdown.is_empty() {
                    return Err(WriteError::Empty);
                }
                Ok((markdown, snapshot.revision))
            })
            .inspect_err(|error| self.notifier.error(&error.to_string()))?;
        let payload = ClipboardPayload {
            plain: markdown.clone(),
            ..ClipboardPayload::default()
        };
        self.write(&markdown, CopyTarget::PlainText, &payload, revision)?;
        self.notifier.mark_copied(CopyTarget::PlainText);
        Ok(markdown)
    }

    fn close_picker(&mut self, op_id: OpId) -> Result<ActiveSession, EngineError> {
        let session = self
            .active
            .take_if(|session| session.op_id == op_id)
            .ok_or(EngineError::StaleOpId)?;
        self.picker.close(op_id);
        Ok(session)
    }

    fn copy(
        &mut self,
        original: &str,
        target: CopyTarget,
        expected_revision: Revision,
        remember_target: bool,
    ) -> Result<(CopiedSummary, Revision), EngineError> {
        let conversion = (self.converter)(original, target, &ConversionOptions::default());
        let revision = self.write(
            original,
            target,
            &conversion.representations,
            expected_revision,
        )?;
        if remember_target {
            self.config.last_target = Some(target);
        }
        self.notifier.mark_copied(target);
        let summary = CopiedSummary {
            target,
            label: target.definition().label.to_string(),
            warning: conversion
                .diagnostics
                .into_iter()
                .find(|diagnostic| diagnostic.severity == DiagnosticSeverity::Warning)
                .map(|diagnostic| diagnostic.message),
        };
        Ok((summary, revision))
    }

    fn write(
        &mut self,
        original: &str,
        target: CopyTarget,
        payload: &ClipboardPayload,
        expected_revision: Revision,
    ) -> Result<Revision, EngineError> {
        let written = self
            .board
            .write(payload, Some(expected_revision))
            .inspect_err(|error| self.notifier.error(&error.to_string()))?;
        self.last_source = Some(RecentSource {
            markdown: original.to_owned(),
            written_revision: written.revision,
            target,
        });
        Ok(written.revision)
    }

    fn present_warning(&mut self, summary: &CopiedSummary) {
        if let Some(warning) = &summary.warning {
            self.notifier.warn("Copied with a warning", warning);
        }
    }
}

#[cfg(test)]
mod tests;
