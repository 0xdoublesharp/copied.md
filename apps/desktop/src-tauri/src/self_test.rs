use copied_clipboard::FakeBoard;
use copied_config::{ConfigPatch, ConfigV2, ShortcutCopyMode, load_config, save_config};
use copied_core::CopyTarget;
use copied_engine::{CopyEngine, CopyOutcome, PickerHost};

use crate::{DesktopError, desktop::DesktopNotifier};

struct NoPicker;

impl PickerHost for NoPicker {
    fn open(&mut self, _: copied_engine::PickerOpenRequest) {
        panic!("copy-only opened a picker");
    }

    fn close(&mut self, _: copied_engine::OpId) {
        panic!("copy-only closed a picker");
    }
}

pub fn self_test() -> Result<(), DesktopError> {
    isolated_checks()?;
    #[cfg(target_os = "macos")]
    check_macos_hotkey()?;
    Ok(())
}

pub(crate) fn isolated_checks() -> Result<(), DesktopError> {
    #[cfg(target_os = "macos")]
    copied_clipboard::macos::self_test()?;
    #[cfg(not(target_os = "macos"))]
    isolated_clipboard_test()?;

    let folder = tempfile::tempdir()?;
    let path = folder.path().join("config.json");
    let mut config = ConfigV2::default();
    config.apply_patch(ConfigPatch {
        shortcut_mode: Some(ShortcutCopyMode::CopyOnly),
        picker_columns: Some(5),
        ..ConfigPatch::default()
    })?;
    save_config(&path, &config)?;
    let read_back = load_config(&path)?;
    check(read_back == config, "configuration round-trip")?;

    let mut engine = CopyEngine::new(
        FakeBoard::with_text("# copied.md"),
        NoPicker,
        DesktopNotifier::default(),
        read_back,
    );
    let outcome = engine.start_copy(engine.mode(), None)?;
    check(
        matches!(outcome, CopyOutcome::Copied(copy) if copy.target == CopyTarget::RichText),
        "engine copy",
    )?;
    check(
        engine.board.current_payload().is_some(),
        "engine clipboard write",
    )
}

fn check(condition: bool, name: &'static str) -> Result<(), DesktopError> {
    if condition {
        Ok(())
    } else {
        Err(DesktopError::SelfTest(name))
    }
}

#[cfg(not(target_os = "macos"))]
fn isolated_clipboard_test() -> Result<(), DesktopError> {
    use copied_clipboard::{Board, WriteError};
    use copied_core::ClipboardPayload;

    let mut board = FakeBoard::with_text("original");
    let original = board.read_text()?;
    let payload = ClipboardPayload {
        plain: "copied.md".into(),
        html: Some("<strong>copied.md</strong>".into()),
        ..ClipboardPayload::default()
    };
    board.write(&payload, Some(original.revision))?;
    check(
        board.current_payload() == Some(&payload),
        "clipboard round-trip",
    )?;
    check(
        board.write(&payload, Some(original.revision)) == Err(WriteError::Stale),
        "stale clipboard write",
    )
}

#[cfg(target_os = "macos")]
fn check_macos_hotkey() -> Result<(), DesktopError> {
    use copied_config::ShortcutSpec;
    use copied_platform::{ShortcutBackend, macos::MacHotKey};
    use objc2::MainThreadMarker;
    use objc2_app_kit::{NSApplication, NSApplicationActivationPolicy};

    let main = MainThreadMarker::new().ok_or(DesktopError::SelfTest("macOS main thread"))?;
    let app = NSApplication::sharedApplication(main);
    app.setActivationPolicy(NSApplicationActivationPolicy::Prohibited);
    let mut hotkey = MacHotKey::new(|| {})?;
    for code in ["F18", "F19"] {
        let spec = ShortcutSpec {
            code: code.into(),
            ctrl: true,
            alt: true,
            shift: true,
            meta: true,
        };
        hotkey.bind(&spec)?;
        check(hotkey.current() == Some(&spec), "macOS hotkey registration")?;
    }
    hotkey.unbind();
    check(hotkey.current().is_none(), "macOS hotkey release")
}
