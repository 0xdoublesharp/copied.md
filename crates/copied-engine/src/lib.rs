use copied_clipboard::{Board, Revision, WriteError};
use copied_config::{ConfigV2, ShortcutCopyMode};
use copied_core::{CopyTarget, CopyTargetDefinition};
use serde::{Deserialize, Serialize};
use thiserror::Error;

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
    #[error("clipboard error: {0}")]
    Clipboard(#[from] WriteError),
}

/// Active picker session state retaining the original captured Markdown.
#[derive(Debug, Clone)]
pub struct ActiveSession {
    pub op_id: OpId,
    pub original_markdown: String,
    pub expected_revision: Revision,
    pub auto_copied: Option<CopyTarget>,
}

/// Minimal M1 placeholder; full TLA+-aligned state machine implemented in M3.
#[derive(Debug)]
pub struct CopyEngine<B: Board, P: PickerHost, N: Notifier> {
    pub board: B,
    pub picker: P,
    pub notifier: N,
    pub config: ConfigV2,
    pub next_op_id: u64,
    pub active: Option<ActiveSession>,
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
        }
    }

    pub fn is_busy(&self) -> bool {
        self.active.is_some()
    }

    pub fn mode(&self) -> ShortcutCopyMode {
        self.config.shortcut_mode
    }
}
