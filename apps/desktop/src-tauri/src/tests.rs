use std::{
    path::PathBuf,
    sync::{Arc, Mutex},
};

use copied_clipboard::{Board, FakeBoard, WriteError};
use copied_config::{
    ConfigPatch, ConfigV2, ShortcutCopyMode, ShortcutSpec, default_shortcut, load_config,
};
use copied_core::{
    ConversionOptions, CopyTarget, Diagnostic, DiagnosticCode, DiagnosticSeverity,
    MarkdownConversion, TARGETS, convert_markdown,
};
use copied_engine::{CopyEngine, EngineError, OpId, PickerOpenRequest};
use copied_platform::{
    ActiveApp, FakeActiveAppProvider, FakeLoginItemBackend, LoginItemStatus, PlatformCapabilities,
    PlatformError, ShortcutBackend, ShortcutCapability,
};
use serde_json::json;

use super::*;
use crate::picker::{Panel, WindowFactory};

const APP: &str = "mac:bundle:com.example.Editor";
const SOURCE: &str = "**copied.md**";

#[derive(Debug, Default)]
struct WindowCalls {
    webview_create_count: usize,
    panels: Vec<(Panel, Option<OpId>)>,
    hides: usize,
    fail_next_show: bool,
}

#[derive(Clone, Default)]
struct CountingFactory(Arc<Mutex<WindowCalls>>);

impl WindowFactory for CountingFactory {
    type Window = ();

    fn create(&mut self) -> tauri::Result<Self::Window> {
        self.0.lock().unwrap().webview_create_count += 1;
        Ok(())
    }

    fn show(
        &mut self,
        _: &Self::Window,
        panel: Panel,
        request: Option<&PickerOpenRequest>,
    ) -> tauri::Result<()> {
        let mut calls = self.0.lock().unwrap();
        if std::mem::take(&mut calls.fail_next_show) {
            return Err(tauri::Error::WindowNotFound);
        }
        calls
            .panels
            .push((panel, request.map(|request| request.op_id)));
        Ok(())
    }

    fn hide(&mut self, _: &Self::Window) -> tauri::Result<()> {
        self.0.lock().unwrap().hides += 1;
        Ok(())
    }
}

#[derive(Default)]
struct ShortcutBackendWithConflict(Option<ShortcutSpec>);

impl ShortcutBackend for ShortcutBackendWithConflict {
    fn current(&self) -> Option<&ShortcutSpec> {
        self.0.as_ref()
    }

    fn bind(&mut self, shortcut: &ShortcutSpec) -> Result<(), PlatformError> {
        if shortcut.code == "KeyX" {
            return Err(PlatformError::ShortcutInUse(shortcut.format_portable()));
        }
        self.0 = Some(shortcut.clone());
        Ok(())
    }

    fn unbind(&mut self) {
        self.0 = None;
    }
}

fn desktop(
    board: FakeBoard,
    path: PathBuf,
    config: ConfigV2,
    factory: CountingFactory,
) -> Desktop<CountingFactory> {
    Desktop::new(
        CopyEngine::new(
            DesktopBoard::from_board(board),
            DesktopPicker::new(factory),
            DesktopNotifier::default(),
            config,
        ),
        path,
        DesktopShortcut::from_backend(ShortcutBackendWithConflict::default()),
        PlatformCapabilities {
            background_clipboard: true,
            global_shortcuts: ShortcutCapability::Direct,
            active_app_detection: true,
            launch_at_login: true,
        },
        Some(Box::new(FakeActiveAppProvider {
            active_app: Some(ActiveApp {
                id: APP.into(),
                name: "Editor".into(),
            }),
        })),
        Some(Box::new(FakeLoginItemBackend::default())),
    )
}

fn warning_conversion(
    markdown: &str,
    target: CopyTarget,
    options: &ConversionOptions,
) -> MarkdownConversion {
    let mut conversion = convert_markdown(markdown, target, options);
    conversion.diagnostics.push(Diagnostic {
        code: DiagnosticCode::FormattingOmitted,
        severity: DiagnosticSeverity::Warning,
        message: "Some formatting was omitted.".into(),
    });
    conversion
}

#[test]
fn copy_only_success_warning_and_error_never_create_a_webview() {
    let startup: tauri::Config = serde_json::from_str(include_str!("../tauri.conf.json")).unwrap();
    assert!(startup.app.windows.is_empty());
    assert!(!startup.app.macos_private_api);

    let dir = tempfile::tempdir().unwrap();
    let factory = CountingFactory::default();
    let config = ConfigV2 {
        shortcut_mode: ShortcutCopyMode::CopyOnly,
        ..ConfigV2::default()
    };

    let mut success = desktop(
        FakeBoard::with_text(SOURCE),
        dir.path().join("success.json"),
        config.clone(),
        factory.clone(),
    );
    let revision = success.engine.board.revision().unwrap();
    success.start(None).unwrap();
    assert_ne!(success.engine.board.revision().unwrap(), revision);
    assert!(matches!(
        success.engine.notifier.0.as_slice(),
        [Feedback::Copied(CopyTarget::RichText)]
    ));
    assert!(success.state().current_picker.is_none());

    let mut warning = desktop(
        FakeBoard::with_text(SOURCE),
        dir.path().join("warning.json"),
        config.clone(),
        factory.clone(),
    );
    warning.engine = warning.engine.with_converter(warning_conversion);
    warning.start(None).unwrap();
    assert!(matches!(
        warning.engine.notifier.0.as_slice(),
        [Feedback::Copied(CopyTarget::RichText), Feedback::Warning(_, message)] if message == "Some formatting was omitted."
    ));
    assert!(warning.state().current_picker.is_none());

    let mut error = desktop(
        FakeBoard::with_text(" "),
        dir.path().join("error.json"),
        config,
        factory.clone(),
    );
    assert!(matches!(
        error.start(None),
        Err(DesktopError::Engine(EngineError::Clipboard(
            WriteError::Empty
        )))
    ));
    assert!(matches!(
        error.engine.notifier.0.as_slice(),
        [Feedback::Error(_)]
    ));
    assert!(error.state().current_picker.is_none());

    let windows = factory.0.lock().unwrap();
    assert_eq!(windows.webview_create_count, 0);
    assert!(windows.panels.is_empty());
    assert_eq!(windows.hides, 0);
}

#[test]
fn picker_reuses_one_window_and_ignores_stale_choices_and_dismissals() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("config.json");
    let factory = CountingFactory::default();
    let mut desktop = desktop(
        FakeBoard::with_text(SOURCE),
        path.clone(),
        ConfigV2::default(),
        factory.clone(),
    );

    assert_eq!(factory.0.lock().unwrap().webview_create_count, 0);
    assert!(desktop.state().current_picker.is_none());
    desktop.start(Some(ShortcutCopyMode::Picker)).unwrap();
    let first = desktop.state().current_picker.unwrap().op_id;
    desktop.dismiss(first).unwrap();
    assert!(desktop.state().current_picker.is_none());
    desktop.start(Some(ShortcutCopyMode::Picker)).unwrap();
    let second = desktop.state().current_picker.unwrap().op_id;
    assert!(second > first);

    let revision = desktop.engine.board.revision().unwrap();
    desktop.choose(first, Some(CopyTarget::PlainText)).unwrap();
    desktop.dismiss(first).unwrap();
    assert_eq!(desktop.engine.board.revision().unwrap(), revision);
    assert_eq!(desktop.state().current_picker.unwrap().op_id, second);

    desktop.choose(second, Some(CopyTarget::PlainText)).unwrap();
    assert_ne!(desktop.engine.board.revision().unwrap(), revision);
    assert_eq!(
        load_config(&path).unwrap().last_target,
        Some(CopyTarget::PlainText)
    );
    assert!(desktop.state().current_picker.is_none());
    let windows = factory.0.lock().unwrap();
    assert_eq!(windows.webview_create_count, 1);
    assert_eq!(
        windows.panels,
        [(Panel::Picker, Some(first)), (Panel::Picker, Some(second))]
    );
    assert_eq!(windows.hides, 2);
}

#[test]
fn failed_picker_open_releases_the_engine_session() {
    let dir = tempfile::tempdir().unwrap();
    let factory = CountingFactory::default();
    factory.0.lock().unwrap().fail_next_show = true;
    let mut desktop = desktop(
        FakeBoard::with_text(SOURCE),
        dir.path().join("config.json"),
        ConfigV2::default(),
        factory.clone(),
    );
    assert!(matches!(
        desktop.start(Some(ShortcutCopyMode::Picker)),
        Err(DesktopError::Native(_))
    ));
    assert!(!desktop.engine.is_busy());
    assert!(desktop.state().current_picker.is_none());
    desktop.start(Some(ShortcutCopyMode::Picker)).unwrap();
    assert!(desktop.state().current_picker.is_some());
    assert_eq!(factory.0.lock().unwrap().webview_create_count, 1);
}

#[test]
fn settings_save_pin_mode_and_columns_atomically() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("config.json");
    let mut desktop = desktop(
        FakeBoard::with_text(SOURCE),
        path.clone(),
        ConfigV2::default(),
        CountingFactory::default(),
    );
    let config = desktop
        .update_config(ConfigPatch {
            picker_columns: Some(5),
            shortcut_mode: Some(ShortcutCopyMode::Picker),
            ..ConfigPatch::default()
        })
        .unwrap();
    assert_eq!(load_config(&path).unwrap(), config);
    assert_eq!(desktop.state().active_app.unwrap().id, APP);
    assert_eq!(desktop.state().targets.len(), 15);
    assert_eq!(
        desktop.toggle_login(true).unwrap(),
        LoginItemStatus::Enabled
    );
    desktop
        .set_pin(APP.into(), Some(CopyTarget::PlainText))
        .unwrap();
    desktop.start(None).unwrap();
    let request = desktop.state().current_picker.unwrap();
    assert_eq!(request.columns, 5);
    assert_eq!(request.selected, CopyTarget::PlainText);
    desktop.dismiss(request.op_id).unwrap();
    let config = desktop.set_pin(APP.into(), None).unwrap();
    assert!(config.pinned_formats.is_empty());

    assert!(
        desktop
            .update_config(ConfigPatch {
                format_visibility: Some(TARGETS.iter().map(|target| (target.id, false)).collect()),
                ..ConfigPatch::default()
            })
            .is_err()
    );
    assert_eq!(load_config(&path).unwrap(), config);
}

#[test]
fn shortcut_recording_cancel_conflict_and_save_failure_restore_the_previous_binding() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("config.json");
    let mut desktop = desktop(
        FakeBoard::with_text(SOURCE),
        path.clone(),
        ConfigV2::default(),
        CountingFactory::default(),
    );
    desktop.show_panel(Panel::Shortcut).unwrap();
    let original = default_shortcut();
    let replacement = ShortcutSpec {
        code: "KeyM".into(),
        ..original.clone()
    };

    desktop.begin_recording().unwrap();
    assert!(!desktop.state().shortcut_registered);
    desktop.cancel_recording().unwrap();
    assert!(desktop.state().shortcut_registered);

    desktop.begin_recording().unwrap();
    let conflict = ShortcutSpec {
        code: "KeyX".into(),
        ..original.clone()
    };
    assert!(desktop.commit_recording(conflict).is_err());
    assert!(desktop.state().shortcut_registered);
    assert_eq!(desktop.state().config.shortcut, original);

    // A directory at the destination makes the atomic persist fail after binding.
    std::fs::create_dir(&path).unwrap();
    desktop.begin_recording().unwrap();
    assert!(desktop.commit_recording(replacement.clone()).is_err());
    assert!(desktop.state().shortcut_registered);
    assert_eq!(desktop.state().config.shortcut, original);
    std::fs::remove_dir(&path).unwrap();

    desktop.begin_recording().unwrap();
    let saved = desktop.commit_recording(replacement).unwrap();
    assert_eq!(load_config(&path).unwrap(), saved);
    assert!(desktop.state().shortcut_registered);
    assert!(!desktop.toggle_shortcut(false).unwrap().shortcut_enabled);
    assert!(!desktop.state().shortcut_registered);
    assert_eq!(desktop.reset_shortcut().unwrap().shortcut, original);
}

#[test]
fn closing_settings_restores_the_shortcut_and_rejects_late_recording() {
    let dir = tempfile::tempdir().unwrap();
    let mut desktop = desktop(
        FakeBoard::with_text(SOURCE),
        dir.path().join("config.json"),
        ConfigV2::default(),
        CountingFactory::default(),
    );
    desktop.show_panel(Panel::Shortcut).unwrap();
    desktop.begin_recording().unwrap();
    assert!(!desktop.state().shortcut_registered);
    desktop.close_panel().unwrap();
    assert!(desktop.state().shortcut_registered);
    assert!(desktop.begin_recording().is_err());
    assert!(desktop.state().shortcut_registered);
}

#[test]
fn preview_ipc_decodes_camel_case_parameters_and_returns_conversion() {
    let app = tauri::test::mock_builder()
        .invoke_handler(tauri::generate_handler![commands::preview_markdown])
        .build(tauri::test::mock_context(tauri::test::noop_assets()))
        .unwrap();
    let window = tauri::WebviewWindowBuilder::new(&app, "picker", Default::default())
        .build()
        .unwrap();
    let payload = json!({
        "markdown": SOURCE,
        "target": "plain-text",
        "sourceUrl": "https://example.com/",
        "decorativeUnicode": true
    });
    let reply = tauri::test::get_ipc_response(
        &window,
        tauri::webview::InvokeRequest {
            cmd: "preview_markdown".into(),
            callback: tauri::ipc::CallbackFn(0),
            error: tauri::ipc::CallbackFn(1),
            url: if cfg!(windows) {
                "http://tauri.localhost"
            } else {
                "tauri://localhost"
            }
            .parse()
            .unwrap(),
            body: tauri::ipc::InvokeBody::Json(payload),
            headers: Default::default(),
            invoke_key: tauri::test::INVOKE_KEY.to_string(),
        },
    )
    .unwrap()
    .deserialize::<MarkdownConversion>()
    .unwrap();
    assert_eq!(
        reply,
        convert_markdown(
            SOURCE,
            CopyTarget::PlainText,
            &ConversionOptions {
                source_url: Some("https://example.com/".into()),
                decorative_unicode: true,
            }
        )
    );
}

#[test]
fn isolated_self_test_passes() {
    crate::self_test::isolated_checks().unwrap();
}
