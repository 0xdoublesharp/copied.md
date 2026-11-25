use super::*;
use copied_clipboard::{ClipboardSnapshot, FakeBoard, WrittenClipboard};
use copied_core::{ClipboardPayload, Diagnostic, DiagnosticCode};
use std::cell::Cell;

#[derive(Debug, Default)]
struct Picker {
    opened: Vec<PickerOpenRequest>,
    closed: Vec<OpId>,
}

impl PickerHost for Picker {
    fn open(&mut self, request: PickerOpenRequest) {
        self.opened.push(request);
    }

    fn close(&mut self, op_id: OpId) {
        self.closed.push(op_id);
    }
}

#[derive(Debug, Default)]
struct Notices {
    copies: Vec<CopyTarget>,
    warnings: Vec<(String, String)>,
    errors: Vec<String>,
}

impl Notifier for Notices {
    fn mark_copied(&mut self, target: CopyTarget) {
        self.copies.push(target);
    }

    fn warn(&mut self, title: &str, detail: &str) {
        self.warnings.push((title.to_owned(), detail.to_owned()));
    }

    fn error(&mut self, message: &str) {
        self.errors.push(message.to_owned());
    }
}

const SOURCE: &str = "**hello**";
const DEFAULT: CopyTarget = CopyTarget::RichText;
const OTHER: CopyTarget = CopyTarget::PlainText;
const PIN: &str = "app";

fn engine<B: Board>(board: B) -> CopyEngine<B, Picker, Notices> {
    CopyEngine::new(
        board,
        Picker::default(),
        Notices::default(),
        ConfigV2::default(),
    )
}

fn marked_conversion(
    text: &str,
    target: CopyTarget,
    options: &ConversionOptions,
) -> MarkdownConversion {
    let mut conversion = convert_markdown(text, target, options);
    conversion.representations.plain = format!("[{target}: {text}]");
    conversion.diagnostics = vec![
        Diagnostic {
            severity: DiagnosticSeverity::Info,
            code: DiagnosticCode::FormattingOmitted,
            message: "Info".into(),
        },
        Diagnostic {
            severity: DiagnosticSeverity::Warning,
            code: DiagnosticCode::FormattingOmitted,
            message: "Warning".into(),
        },
    ];
    conversion
}

fn opened(outcome: &CopyOutcome) -> OpId {
    let CopyOutcome::PickerOpened(op_id) = outcome else {
        panic!("expected picker, got {outcome:?}");
    };
    *op_id
}

#[test]
fn picker_uses_pin_to_select_and_commits_only_after_choice() {
    let mut engine = engine(FakeBoard::with_text(SOURCE));
    engine.config.shortcut_mode = ShortcutCopyMode::Picker;
    engine.config.picker_columns = 3;
    engine.config.pinned_formats.insert(PIN.into(), OTHER);
    engine.config.format_visibility.insert(DEFAULT, false);
    let op_id = opened(&engine.start_copy(engine.mode(), Some(PIN)).unwrap());

    let request = &engine.picker.opened[0];
    assert_eq!((request.op_id, request.columns), (op_id, 3));
    assert_eq!(
        (request.selected, request.suggested, &request.copied),
        (OTHER, Some(OTHER), &None)
    );
    assert!(
        request
            .visible_targets
            .iter()
            .all(|target| target.id != DEFAULT)
    );
    assert_eq!(engine.board.revision().unwrap(), Revision(0));
    assert_eq!(engine.config.last_target, None);

    assert!(matches!(
        engine.handle_picker_choice(op_id, Some(OTHER)),
        Ok(CopyOutcome::Copied(CopiedSummary {
            target: OTHER,
            warning: None,
            ..
        }))
    ));
    assert_eq!(engine.board.revision().unwrap(), Revision(1));
    assert_eq!(engine.config.last_target, Some(OTHER));
    assert_eq!(engine.picker.closed, [op_id]);
    assert!(!engine.is_busy());
}

#[test]
fn copy_and_pick_uses_original_and_first_write_revision() {
    let mut engine = engine(FakeBoard::with_text(SOURCE)).with_converter(marked_conversion);
    let op_id = opened(
        &engine
            .start_copy(ShortcutCopyMode::CopyAndPick, None)
            .unwrap(),
    );
    let copied = engine.picker.opened[0].copied.as_ref().unwrap();
    assert_eq!(copied.target, DEFAULT);
    assert_eq!(copied.warning.as_deref(), Some("Warning"));
    assert_eq!(engine.notifier.warnings, [] as [(String, String); 0]);
    assert_eq!(engine.config.last_target, Some(DEFAULT));
    let active = engine.active.as_ref().unwrap();
    assert_eq!(active.original_markdown, SOURCE);
    assert_eq!(active.expected_revision, Revision(1));
    engine.handle_picker_choice(op_id, Some(OTHER)).unwrap();

    assert_eq!(
        engine.board.current_payload().unwrap().plain,
        format!("[{OTHER}: {SOURCE}]")
    );
    assert_eq!(engine.board.revision().unwrap(), Revision(2));
    assert_eq!(engine.config.last_target, Some(OTHER));
    assert_eq!(engine.notifier.copies, [DEFAULT, OTHER]);
    assert_eq!(engine.notifier.warnings[0].1, "Warning");
}

#[derive(Debug)]
struct GappedRevisionBoard(FakeBoard);

impl Board for GappedRevisionBoard {
    fn revision(&self) -> Result<Revision, WriteError> {
        self.0.revision().map(|revision| Revision(revision.0 * 7))
    }

    fn read_text(&self) -> Result<ClipboardSnapshot, WriteError> {
        Ok(ClipboardSnapshot {
            revision: self.revision()?,
            ..self.0.read_text()?
        })
    }

    fn write(
        &mut self,
        payload: &ClipboardPayload,
        expected: Option<Revision>,
    ) -> Result<WrittenClipboard, WriteError> {
        assert_eq!(expected, Some(self.revision()?));
        let written = self.0.write(payload, None)?;
        Ok(WrittenClipboard {
            revision: self.revision()?,
            ..written
        })
    }
}

#[test]
fn replacement_uses_returned_revision_even_when_revisions_skip() {
    let mut engine = engine(GappedRevisionBoard(FakeBoard::with_text(SOURCE)));
    let op_id = opened(
        &engine
            .start_copy(ShortcutCopyMode::CopyAndPick, None)
            .unwrap(),
    );
    assert_eq!(
        engine.active.as_ref().unwrap().expected_revision,
        Revision(7)
    );
    engine.handle_picker_choice(op_id, Some(OTHER)).unwrap();
    assert_eq!(engine.board.revision().unwrap(), Revision(14));
}

#[test]
fn pinned_auto_copy_is_preserved_on_cancel_dismiss_and_same_format() {
    for (dismiss, choice) in [(true, None), (false, None), (false, Some(OTHER))] {
        let mut engine = engine(FakeBoard::with_text(SOURCE));
        engine.config.last_target = Some(DEFAULT);
        engine.config.pinned_formats.insert(PIN.into(), OTHER);
        let op_id = opened(
            &engine
                .start_copy(ShortcutCopyMode::CopyAndPick, Some(PIN))
                .unwrap(),
        );
        let before = engine.board.current_payload().cloned();

        if dismiss {
            engine.handle_picker_dismiss(op_id).unwrap();
        } else {
            assert_eq!(
                engine.handle_picker_choice(op_id, choice),
                Ok(CopyOutcome::Unchanged)
            );
        }
        assert_eq!(engine.board.current_payload(), before.as_ref());
        assert_eq!(engine.board.revision().unwrap(), Revision(1));
        assert_eq!(engine.config.last_target, Some(DEFAULT));
        assert_eq!(engine.notifier.copies, [OTHER]);
        assert_eq!(engine.picker.closed, [op_id]);
        assert!(!engine.is_busy());
    }
}

#[test]
fn busy_and_old_picker_replies_leave_the_active_session_alone() {
    let mut engine = engine(FakeBoard::with_text(SOURCE));
    let first = opened(&engine.start_copy(ShortcutCopyMode::Picker, None).unwrap());
    engine.handle_picker_dismiss(first).unwrap();
    let second = opened(&engine.start_copy(ShortcutCopyMode::Picker, None).unwrap());
    assert!(first < second);

    assert_eq!(
        engine.start_copy(ShortcutCopyMode::CopyOnly, None),
        Err(EngineError::Busy)
    );
    assert_eq!(
        engine.handle_picker_choice(first, Some(OTHER)),
        Err(EngineError::StaleOpId)
    );
    assert_eq!(
        engine.handle_picker_dismiss(first),
        Err(EngineError::StaleOpId)
    );
    assert_eq!(engine.active.as_ref().unwrap().op_id, second);
    assert_eq!(engine.picker.opened.len(), 2);
    assert_eq!(engine.picker.closed, [first]);
    assert_eq!(engine.board.revision().unwrap(), Revision(0));
    assert_eq!(engine.notifier.errors, [] as [String; 0]);

    engine.handle_picker_dismiss(second).unwrap();
    assert_eq!(
        engine.handle_picker_dismiss(second),
        Err(EngineError::StaleOpId)
    );
}

#[test]
fn copy_only_marks_copied_and_warns_without_opening_picker() {
    let mut engine = engine(FakeBoard::with_text(SOURCE)).with_converter(marked_conversion);
    engine.config.last_target = Some(DEFAULT);
    engine.config.pinned_formats.insert(PIN.into(), OTHER);

    assert!(matches!(
        engine.start_copy(ShortcutCopyMode::CopyOnly, Some(PIN)),
        Ok(CopyOutcome::Copied(CopiedSummary {
            target: OTHER,
            warning: Some(_),
            ..
        }))
    ));
    assert_eq!(engine.config.last_target, Some(DEFAULT));
    assert_eq!(engine.notifier.copies, [OTHER]);
    assert_eq!(engine.notifier.warnings[0].1, "Warning");
    assert_eq!(engine.picker.opened, [] as [PickerOpenRequest; 0]);
    assert_eq!(engine.picker.closed, [] as [OpId; 0]);
    assert!(!engine.is_busy());

    let mut engine = self::engine(FakeBoard::with_text(SOURCE));
    engine.start_copy(ShortcutCopyMode::CopyOnly, None).unwrap();
    assert_eq!(engine.config.last_target, Some(DEFAULT));
    assert_eq!(engine.notifier.warnings, [] as [(String, String); 0]);
    assert_eq!(engine.picker.opened, [] as [PickerOpenRequest; 0]);
}

#[test]
fn rejected_auto_copy_leaves_the_source_and_config_intact() {
    fn empty_output(
        text: &str,
        target: CopyTarget,
        options: &ConversionOptions,
    ) -> MarkdownConversion {
        let mut conversion = marked_conversion(text, target, options);
        conversion.representations.plain.clear();
        conversion
    }

    for mode in [ShortcutCopyMode::CopyOnly, ShortcutCopyMode::CopyAndPick] {
        let mut engine = engine(FakeBoard::with_text(SOURCE)).with_converter(empty_output);
        assert_eq!(
            engine.start_copy(mode, None),
            Err(EngineError::Clipboard(WriteError::Empty))
        );
        assert_eq!(engine.board.current_payload().unwrap().plain, SOURCE);
        assert_eq!(engine.board.revision().unwrap(), Revision(0));
        assert_eq!(engine.config.last_target, None);
        assert_eq!(engine.picker.opened, [] as [PickerOpenRequest; 0]);
        assert_eq!(engine.notifier.copies, [] as [CopyTarget; 0]);
        assert_eq!(engine.notifier.warnings, [] as [(String, String); 0]);
        assert_eq!(engine.notifier.errors, [WriteError::Empty.to_string()]);
    }
}

#[test]
fn empty_clipboard_notifies_and_never_opens_picker() {
    for mode in [
        ShortcutCopyMode::CopyOnly,
        ShortcutCopyMode::CopyAndPick,
        ShortcutCopyMode::Picker,
    ] {
        for board in [FakeBoard::default(), FakeBoard::with_text(" \t\n")] {
            let mut engine = engine(board);
            assert_eq!(
                engine.start_copy(mode, None),
                Err(EngineError::Clipboard(WriteError::Empty))
            );
            assert_eq!(engine.notifier.errors, [WriteError::Empty.to_string()]);
            assert_eq!(engine.picker.opened, [] as [PickerOpenRequest; 0]);
            assert_eq!(engine.picker.closed, [] as [OpId; 0]);
            assert!(!engine.is_busy());
        }
    }
}

#[derive(Debug)]
struct CopyDuringFirstWrite {
    board: FakeBoard,
    read: Cell<bool>,
}

impl Board for CopyDuringFirstWrite {
    fn revision(&self) -> Result<Revision, WriteError> {
        self.board.revision()
    }

    fn read_text(&self) -> Result<ClipboardSnapshot, WriteError> {
        assert!(!self.read.replace(true));
        self.board.read_text()
    }

    fn write(
        &mut self,
        payload: &ClipboardPayload,
        expected: Option<Revision>,
    ) -> Result<WrittenClipboard, WriteError> {
        self.board.external_copy("external");
        self.board.write(payload, expected)
    }
}

#[test]
fn stale_first_write_leaves_external_copy_and_never_opens_picker() {
    for mode in [ShortcutCopyMode::CopyOnly, ShortcutCopyMode::CopyAndPick] {
        let mut engine = engine(CopyDuringFirstWrite {
            board: FakeBoard::with_text(SOURCE),
            read: Cell::new(false),
        });
        assert_eq!(
            engine.start_copy(mode, None),
            Err(EngineError::Clipboard(WriteError::Stale))
        );
        assert_eq!(
            engine.board.board.current_payload().unwrap().plain,
            "external"
        );
        assert_eq!(engine.board.revision().unwrap(), Revision(1));
        assert_eq!(engine.config.last_target, None);
        assert_eq!(engine.notifier.errors, [WriteError::Stale.to_string()]);
        assert_eq!(engine.notifier.copies, [] as [CopyTarget; 0]);
        assert_eq!(engine.picker.opened, [] as [PickerOpenRequest; 0]);
        assert_eq!(engine.picker.closed, [] as [OpId; 0]);
        assert!(!engine.is_busy());
    }
}

#[test]
fn stale_picker_choice_preserves_external_copy_and_remembered_target() {
    for mode in [ShortcutCopyMode::Picker, ShortcutCopyMode::CopyAndPick] {
        let mut engine = engine(FakeBoard::with_text(SOURCE));
        let op_id = opened(&engine.start_copy(mode, None).unwrap());
        let last_target = engine.config.last_target;
        let revision = engine.board.external_copy("external");
        assert_eq!(
            engine.handle_picker_choice(op_id, Some(OTHER)),
            Err(EngineError::Clipboard(WriteError::Stale))
        );
        assert_eq!(engine.board.current_payload().unwrap().plain, "external");
        assert_eq!(engine.board.revision().unwrap(), revision);
        assert_eq!(engine.config.last_target, last_target);
        assert_eq!(engine.notifier.errors, [WriteError::Stale.to_string()]);
        assert_eq!(engine.picker.closed, [op_id]);
        assert!(!engine.is_busy());
    }
}

#[test]
fn picker_id_exhaustion_is_rejected_before_writing() {
    let mut engine = engine(FakeBoard::with_text(SOURCE));
    engine.next_op_id = u64::MAX;
    assert_eq!(
        engine.start_copy(ShortcutCopyMode::CopyAndPick, None),
        Err(EngineError::OpIdExhausted)
    );
    assert_eq!(engine.board.revision().unwrap(), Revision(0));
    assert_eq!(engine.picker.opened, [] as [PickerOpenRequest; 0]);
    assert_eq!(engine.notifier.copies, [] as [CopyTarget; 0]);

    engine.start_copy(ShortcutCopyMode::CopyOnly, None).unwrap();
    assert_eq!(engine.board.revision().unwrap(), Revision(1));
}
