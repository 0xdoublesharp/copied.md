use copied_clipboard::{Board, FakeBoard, Revision, WriteError};
use copied_config::{ConfigV2, ShortcutCopyMode, ShortcutSpec, default_shortcut};
use copied_core::{
    ConversionOptions, CopyTarget, Diagnostic, DiagnosticCode, DiagnosticSeverity,
    MarkdownConversion, convert_markdown,
};
use copied_engine::{
    ActiveSession, CopiedSummary, CopyEngine, CopyOutcome, EngineError, Notifier, OpId, PickerHost,
    PickerOpenRequest, ShortcutController, ShortcutError,
};
use copied_platform::{PlatformError, ShortcutBackend};
use proptest::prelude::*;
use proptest_state_machine::{ReferenceStateMachine, StateMachineTest, prop_state_machine};

const SOURCE: &str = "**source**";
const PIN_APP: &str = "app-with-pin";
const PIN: CopyTarget = CopyTarget::Discord;
const FIRST: CopyTarget = CopyTarget::RichText;
const WARNING: &str = "model conversion warning";
const EMPTY_OUTPUT: &str = "source with no converted text";

#[derive(Debug, Clone, Copy)]
enum Action {
    StartCopy(ShortcutCopyMode, bool),
    ExternalCopy(&'static str),
    PickerChoose(Option<CopyTarget>),
    PickerDismiss,
    StalePickerReply { dismiss: bool },
    BeginRecord,
    FinishRecord(Key),
    CancelRecord,
    SetEnabled(bool),
    Reset,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Key {
    Default,
    Alternate,
    Reserved,
    Invalid,
}

impl Key {
    fn spec(self) -> ShortcutSpec {
        let mut spec = default_shortcut();
        spec.code = match self {
            Self::Default => "KeyC",
            Self::Alternate => "KeyD",
            Self::Reserved => "KeyX",
            Self::Invalid => "ArrowLeft",
        }
        .into();
        spec
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum Notice {
    Copied(CopyTarget),
    Warning(String),
    Error(String),
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum BindingEvent {
    Register(ShortcutSpec),
    Unbind(ShortcutSpec),
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct Open {
    id: OpId,
    target: CopyTarget,
    suggested: Option<CopyTarget>,
    auto: Option<CopiedSummary>,
    preview: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum Clipboard {
    Source(&'static str),
    External(&'static str),
    Converted { target: CopyTarget, source: String },
}

impl Clipboard {
    fn text(&self) -> String {
        match self {
            Self::Source(text) | Self::External(text) => (*text).into(),
            Self::Converted { target, source } => format!("{target}({source})"),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum Reply {
    None,
    Copy(Result<CopyOutcome, EngineError>),
    Dismiss(Result<(), EngineError>),
    Shortcut(Result<(), ShortcutError>),
}

#[derive(Debug, Clone)]
struct Model {
    clipboard: Clipboard,
    revision: Revision,
    last_target: Option<CopyTarget>,
    active: Option<ActiveSession>,
    opens: Vec<Open>,
    closes: Vec<OpId>,
    notices: Vec<Notice>,
    binding_events: Vec<BindingEvent>,
    configured: Key,
    registered: Option<Key>,
    enabled: bool,
    recording: bool,
    paused: Option<Key>,
    reply: Reply,
}

impl Model {
    fn start(&mut self, mode: ShortcutCopyMode, pinned: bool) -> Result<CopyOutcome, EngineError> {
        if self.active.is_some() {
            return Err(EngineError::Busy);
        }
        let source = match &self.clipboard {
            Clipboard::Converted { source, .. } => source.clone(),
            clipboard => clipboard.text(),
        };
        if source.trim().is_empty() {
            return Err(self.error(WriteError::Empty));
        }
        if source == EMPTY_OUTPUT && mode != ShortcutCopyMode::Picker {
            return Err(self.error(WriteError::Empty));
        }
        let target = if pinned {
            PIN
        } else {
            self.last_target.unwrap_or(FIRST)
        };
        let auto = match mode {
            ShortcutCopyMode::CopyOnly => {
                let copied = self.commit(target, source, !pinned, true);
                return Ok(CopyOutcome::Copied(copied));
            }
            ShortcutCopyMode::CopyAndPick => {
                Some(self.commit(target, source.clone(), !pinned, false))
            }
            ShortcutCopyMode::Picker => None,
        };

        let id = OpId(self.opens.len() as u64 + 1);
        self.active = Some(ActiveSession {
            op_id: id,
            original_markdown: source.clone(),
            expected_revision: self.revision,
            auto_copied: auto.as_ref().map(|summary| summary.target),
        });
        self.opens.push(Open {
            id,
            target,
            suggested: (pinned && mode == ShortcutCopyMode::Picker).then_some(target),
            auto,
            preview: Some(source),
        });
        Ok(CopyOutcome::PickerOpened(id))
    }

    fn close(&mut self) -> Result<ActiveSession, EngineError> {
        let session = self.active.take().ok_or(EngineError::StaleOpId)?;
        self.closes.push(session.op_id);
        Ok(session)
    }

    fn pick(&mut self, choice: Option<CopyTarget>) -> Result<CopyOutcome, EngineError> {
        let session = self.close()?;
        match choice {
            None => Ok(CopyOutcome::Unchanged),
            Some(target) if Some(target) == session.auto_copied => Ok(CopyOutcome::Unchanged),
            Some(_) if session.original_markdown == EMPTY_OUTPUT => {
                Err(self.error(WriteError::Empty))
            }
            Some(_) if self.revision != session.expected_revision => {
                Err(self.error(WriteError::Stale))
            }
            Some(target) => Ok(CopyOutcome::Copied(self.commit(
                target,
                session.original_markdown,
                true,
                true,
            ))),
        }
    }

    fn commit(
        &mut self,
        target: CopyTarget,
        source: String,
        remember: bool,
        warn: bool,
    ) -> CopiedSummary {
        self.clipboard = Clipboard::Converted { target, source };
        self.revision.0 += 1;
        if remember {
            self.last_target = Some(target);
        }
        self.notices.push(Notice::Copied(target));
        let warning = (target == PIN).then(|| WARNING.to_owned());
        if warn && let Some(detail) = &warning {
            self.notices.push(Notice::Warning(detail.clone()));
        }
        CopiedSummary {
            target,
            label: target.definition().label.to_string(),
            warning,
        }
    }

    fn error(&mut self, error: WriteError) -> EngineError {
        self.notices.push(Notice::Error(error.to_string()));
        EngineError::Clipboard(error)
    }

    fn register(&mut self, key: Key) {
        if self.registered != Some(key) {
            self.binding_events.push(BindingEvent::Register(key.spec()));
            self.unbind();
            self.registered = Some(key);
        }
    }

    fn unbind(&mut self) {
        if let Some(key) = self.registered.take() {
            self.binding_events.push(BindingEvent::Unbind(key.spec()));
        }
    }

    fn begin_recording(&mut self) -> Result<(), ShortcutError> {
        if self.recording {
            return Err(ShortcutError::AlreadyRecording);
        }
        self.paused = self.registered;
        self.unbind();
        self.recording = true;
        Ok(())
    }

    fn cancel_recording(&mut self) -> Result<(), ShortcutError> {
        if !self.recording {
            return Err(ShortcutError::NotRecording);
        }
        if let Some(key) = self.paused.take() {
            self.register(key);
        }
        self.recording = false;
        Ok(())
    }

    fn finish_recording(&mut self, candidate: Key) -> Result<(), ShortcutError> {
        if !self.recording {
            return Err(ShortcutError::NotRecording);
        }
        match candidate {
            Key::Invalid => {
                self.cancel_recording()?;
                Err(ShortcutError::InvalidShortcut)
            }
            Key::Reserved => {
                self.binding_events
                    .push(BindingEvent::Register(candidate.spec()));
                self.cancel_recording()?;
                Err(PlatformError::ShortcutInUse(candidate.spec().code).into())
            }
            key => {
                self.register(key);
                self.configured = key;
                self.enabled = true;
                self.recording = false;
                self.paused = None;
                Ok(())
            }
        }
    }

    fn settings(&mut self, enabled: bool, key: Key) -> Result<(), ShortcutError> {
        if self.recording {
            return Err(ShortcutError::AlreadyRecording);
        }
        if enabled {
            self.register(key);
        } else {
            self.unbind();
        }
        self.enabled = enabled;
        self.configured = key;
        Ok(())
    }
}

impl ReferenceStateMachine for Model {
    type State = Self;
    type Transition = Action;

    fn init_state() -> BoxedStrategy<Self> {
        Just(Self {
            clipboard: Clipboard::Source(SOURCE),
            revision: Revision(0),
            last_target: None,
            active: None,
            opens: Vec::new(),
            closes: Vec::new(),
            notices: Vec::new(),
            binding_events: Vec::new(),
            configured: Key::Default,
            registered: Some(Key::Default),
            enabled: true,
            recording: false,
            paused: None,
            reply: Reply::None,
        })
        .boxed()
    }

    fn transitions(_: &Self) -> BoxedStrategy<Action> {
        let modes = prop::sample::select(vec![
            ShortcutCopyMode::Picker,
            ShortcutCopyMode::CopyAndPick,
            ShortcutCopyMode::CopyOnly,
        ]);
        let choices = prop::option::of(prop::sample::select(vec![
            FIRST,
            CopyTarget::PlainText,
            PIN,
        ]));
        let external =
            prop::sample::select(vec!["external copy", "*new* source", " \n", EMPTY_OUTPUT]);
        let keys = prop::sample::select(vec![
            Key::Default,
            Key::Alternate,
            Key::Reserved,
            Key::Invalid,
        ]);
        prop_oneof![
            5 => (modes, any::<bool>()).prop_map(|(mode, pinned)| Action::StartCopy(mode, pinned)),
            3 => external.prop_map(Action::ExternalCopy),
            3 => choices.prop_map(Action::PickerChoose),
            2 => Just(Action::PickerDismiss),
            2 => any::<bool>().prop_map(|dismiss| Action::StalePickerReply { dismiss }),
            2 => Just(Action::BeginRecord),
            2 => keys.prop_map(Action::FinishRecord),
            2 => Just(Action::CancelRecord),
            1 => any::<bool>().prop_map(Action::SetEnabled),
            1 => Just(Action::Reset),
        ]
        .boxed()
    }

    fn apply(mut state: Self, action: &Action) -> Self {
        state.reply = match *action {
            Action::StartCopy(mode, pinned) => Reply::Copy(state.start(mode, pinned)),
            Action::ExternalCopy(text) => {
                state.clipboard = Clipboard::External(text);
                state.revision.0 += 1;
                Reply::None
            }
            Action::PickerChoose(choice) => Reply::Copy(state.pick(choice)),
            Action::PickerDismiss => Reply::Dismiss(state.close().map(|_| ())),
            Action::StalePickerReply { dismiss: true } => {
                Reply::Dismiss(Err(EngineError::StaleOpId))
            }
            Action::StalePickerReply { dismiss: false } => Reply::Copy(Err(EngineError::StaleOpId)),
            Action::BeginRecord => Reply::Shortcut(state.begin_recording()),
            Action::FinishRecord(candidate) => Reply::Shortcut(state.finish_recording(candidate)),
            Action::CancelRecord => Reply::Shortcut(state.cancel_recording()),
            Action::SetEnabled(enabled) => {
                Reply::Shortcut(state.settings(enabled, state.configured))
            }
            Action::Reset => Reply::Shortcut(state.settings(true, Key::Default)),
        };
        state
    }
}

#[derive(Debug, Default)]
struct CountingPicker {
    opens: Vec<Open>,
    closes: Vec<OpId>,
}

impl PickerHost for CountingPicker {
    fn open(&mut self, request: PickerOpenRequest) {
        self.opens.push(Open {
            id: request.op_id,
            target: request.selected,
            suggested: request.suggested,
            auto: request.copied,
            preview: request.source_preview,
        });
    }

    fn close(&mut self, id: OpId) {
        self.closes.push(id);
    }
}

#[derive(Debug, Default)]
struct RecordingNotifier(Vec<Notice>);

impl Notifier for RecordingNotifier {
    fn mark_copied(&mut self, target: CopyTarget) {
        self.0.push(Notice::Copied(target));
    }

    fn warn(&mut self, title: &str, detail: &str) {
        assert_eq!(title, "Copied with a warning");
        self.0.push(Notice::Warning(detail.into()));
    }

    fn error(&mut self, message: &str) {
        self.0.push(Notice::Error(message.into()));
    }
}

#[derive(Debug)]
struct RecordingBackend {
    current: Option<ShortcutSpec>,
    events: Vec<BindingEvent>,
}

impl ShortcutBackend for RecordingBackend {
    fn current(&self) -> Option<&ShortcutSpec> {
        self.current.as_ref()
    }

    fn bind(&mut self, shortcut: &ShortcutSpec) -> Result<(), PlatformError> {
        if self.current() == Some(shortcut) {
            return Ok(());
        }
        self.events.push(BindingEvent::Register(shortcut.clone()));
        if shortcut.code == "KeyX" {
            return Err(PlatformError::ShortcutInUse(shortcut.code.clone()));
        }
        self.unbind();
        self.current = Some(shortcut.clone());
        Ok(())
    }

    fn unbind(&mut self) {
        if let Some(shortcut) = self.current.take() {
            self.events.push(BindingEvent::Unbind(shortcut));
        }
    }
}

fn convert(text: &str, target: CopyTarget, options: &ConversionOptions) -> MarkdownConversion {
    let mut conversion = convert_markdown(text, target, options);
    conversion.representations.plain = format!("{}({text})", target.id());
    if text == EMPTY_OUTPUT {
        conversion.representations.plain.clear();
    }
    conversion.diagnostics.clear();
    if target == PIN {
        conversion.diagnostics.push(Diagnostic {
            code: DiagnosticCode::FormattingOmitted,
            severity: DiagnosticSeverity::Warning,
            message: WARNING.into(),
        });
    }
    conversion
}

struct Desktop {
    engine: CopyEngine<FakeBoard, CountingPicker, RecordingNotifier>,
    shortcuts: ShortcutController<RecordingBackend>,
}

impl StateMachineTest for Desktop {
    type SystemUnderTest = Self;
    type Reference = Model;

    fn init_test(model: &Model) -> Self {
        let mut config = ConfigV2::default();
        config.pinned_formats.insert(PIN_APP.into(), PIN);
        Self {
            shortcuts: ShortcutController::new(RecordingBackend {
                current: Some(config.shortcut.clone()),
                events: Vec::new(),
            }),
            engine: CopyEngine::new(
                FakeBoard::with_text(model.clipboard.text()),
                CountingPicker::default(),
                RecordingNotifier::default(),
                config,
            )
            .with_converter(convert),
        }
    }

    fn apply(mut desktop: Self, model: &Model, action: Action) -> Self {
        let engine = &mut desktop.engine;
        let shortcuts = &mut desktop.shortcuts;
        let current = engine.active.as_ref().map(|session| session.op_id);
        let stale = OpId(engine.next_op_id - if current.is_some() { 2 } else { 1 });
        let reply = match action {
            Action::StartCopy(mode, pinned) => {
                Reply::Copy(engine.start_copy(mode, pinned.then_some(PIN_APP)))
            }
            Action::ExternalCopy(text) => {
                engine.board.external_copy(text);
                Reply::None
            }
            Action::PickerChoose(choice) => {
                Reply::Copy(engine.handle_picker_choice(current.unwrap_or(stale), choice))
            }
            Action::PickerDismiss => {
                Reply::Dismiss(engine.handle_picker_dismiss(current.unwrap_or(stale)))
            }
            Action::StalePickerReply { dismiss: true } => {
                Reply::Dismiss(engine.handle_picker_dismiss(stale))
            }
            Action::StalePickerReply { dismiss: false } => {
                Reply::Copy(engine.handle_picker_choice(stale, Some(PIN)))
            }
            Action::BeginRecord => Reply::Shortcut(shortcuts.begin_recording()),
            Action::FinishRecord(key) => {
                Reply::Shortcut(shortcuts.finish_recording(key.spec(), &mut engine.config))
            }
            Action::CancelRecord => Reply::Shortcut(shortcuts.cancel_recording()),
            Action::SetEnabled(enabled) => {
                Reply::Shortcut(shortcuts.set_enabled(enabled, &mut engine.config))
            }
            Action::Reset => Reply::Shortcut(shortcuts.reset_to_default(&mut engine.config)),
        };
        assert_eq!(reply, model.reply);
        desktop
    }

    fn check_invariants(desktop: &Self, model: &Model) {
        let engine = &desktop.engine;
        assert_eq!(engine.is_busy(), model.active.is_some());
        assert_eq!(engine.active, model.active);
        assert_eq!(engine.next_op_id, model.opens.len() as u64 + 1);
        assert_eq!(engine.picker.opens, model.opens);
        assert_eq!(engine.picker.closes, model.closes);
        assert_eq!(engine.notifier.0, model.notices);
        assert_eq!(engine.board.revision().unwrap(), model.revision);
        assert_eq!(
            engine.board.current_payload().unwrap().plain,
            model.clipboard.text()
        );
        assert_eq!(engine.config.last_target, model.last_target);

        let shortcuts = &desktop.shortcuts;
        assert_eq!(shortcuts.is_recording(), model.recording);
        assert_eq!(
            shortcuts.backend().current(),
            model.registered.map(Key::spec).as_ref()
        );
        assert_eq!(shortcuts.backend().events, model.binding_events);
        assert_eq!(engine.config.shortcut, model.configured.spec());
        assert_eq!(engine.config.shortcut_enabled, model.enabled);
    }
}

prop_state_machine! {
    #![proptest_config(ProptestConfig::with_cases(512))]
    #[test]
    fn desktop_matches_clipboard_and_shortcut_model(sequential 1..80 => Desktop);
}
