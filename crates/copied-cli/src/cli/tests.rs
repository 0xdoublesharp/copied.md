use super::*;
use copied_clipboard::{ClipboardSnapshot, FakeBoard, Revision, WrittenClipboard};
use copied_config::{ConfigV2, load_config, save_config};
use copied_core::{DiagnosticCode, DiagnosticSeverity};
use serde_json::{Value, json};
use std::{
    cell::Cell,
    io::{BufReader, Cursor},
    sync::{Arc, Mutex},
};

#[derive(Debug)]
struct Output {
    status: u8,
    stdout: Vec<u8>,
    stderr: Vec<u8>,
}

impl Output {
    fn json(&self) -> Value {
        assert_eq!(self.status, 0, "{self:?}");
        assert!(self.stderr.is_empty(), "{self:?}");
        serde_json::from_slice(&self.stdout).unwrap()
    }

    fn check_error(&self, status: u8, code: &str) {
        assert_eq!(self.status, status, "{self:?}");
        assert!(self.stdout.is_empty(), "{self:?}");
        let envelope: Value = serde_json::from_slice(&self.stderr).unwrap();
        let message = envelope["error"]["message"].as_str().unwrap();
        assert_ne!(message, "");
        assert_eq!(envelope, json!({"error":{"code":code,"message":message}}));
    }
}

struct Harness {
    board: Arc<Mutex<FakeBoard>>,
    opens: Cell<usize>,
    open_error: Option<WriteError>,
    pick: Option<&'static str>,
    picks: Cell<usize>,
    config_path: Option<PathBuf>,
}

impl Harness {
    fn new() -> Self {
        Self {
            board: Arc::new(Mutex::new(FakeBoard::with_text("**Source**"))),
            opens: Cell::new(0),
            open_error: None,
            pick: None,
            picks: Cell::new(0),
            config_path: None,
        }
    }

    fn execute(&self, args: &[&str], input: &str) -> Output {
        self.run(
            "copied-md",
            args,
            BufReader::new(Chunks(input.as_bytes())),
            false,
        )
    }

    fn run(&self, exe: &str, args: &[&str], input: impl BufRead, interactive: bool) -> Output {
        let mut stdout = Vec::new();
        let mut stderr = Vec::new();
        let arguments = std::iter::once(exe)
            .chain(args.iter().copied())
            .map(OsString::from)
            .collect();
        let mut open_board = || -> Result<Box<dyn Board>, WriteError> {
            self.opens.set(self.opens.get() + 1);
            if let Some(error) = &self.open_error {
                return Err(error.clone());
            }
            Ok(Box::new(SharedBoard(self.board.clone())))
        };
        let mut choose = |_: &[picker::PickerTarget], _: Option<&str>, _: bool| {
            self.picks.set(self.picks.get() + 1);
            Ok(self.pick.map(str::to_owned))
        };
        let mut services = Services {
            open_board: &mut open_board,
            choose: &mut choose,
            config_path: self.config_path.clone(),
            #[cfg(target_os = "linux")]
            linux_helper: false,
        };
        let status = super::run(
            arguments,
            &mut Streams {
                input,
                output: &mut stdout,
                error: &mut stderr,
                interactive,
            },
            &mut services,
        );
        Output {
            status,
            stdout,
            stderr,
        }
    }
}

struct Chunks<'a>(&'a [u8]);

impl io::Read for Chunks<'_> {
    fn read(&mut self, output: &mut [u8]) -> io::Result<usize> {
        let max = output.len().min(3);
        self.0.read(&mut output[..max])
    }
}

struct SharedBoard(Arc<Mutex<FakeBoard>>);

impl Board for SharedBoard {
    fn revision(&self) -> Result<Revision, WriteError> {
        self.0.lock().unwrap().revision()
    }

    fn read_text(&self) -> Result<ClipboardSnapshot, WriteError> {
        self.0.lock().unwrap().read_text()
    }

    fn write(
        &mut self,
        payload: &ClipboardPayload,
        expected: Option<Revision>,
    ) -> Result<WrittenClipboard, WriteError> {
        self.0.lock().unwrap().write(payload, expected)
    }
}

fn diagnostics(result: &MarkdownConversion) -> Vec<u8> {
    let mut expected = Vec::new();
    for diagnostic in &result.diagnostics {
        writeln!(expected, "{}", diagnostic.message).unwrap();
    }
    expected
}

#[test]
fn help_and_target_catalog_have_both_output_modes() {
    let cli = Harness::new();
    let help = cli.execute(&["-h"], "");
    assert_eq!(help.status, 0);
    let help = String::from_utf8(help.stdout).unwrap();
    for name in ["targets", "convert", "copy", "clipboard", "mcp"] {
        assert!(help.contains(name));
    }
    let plain = cli.execute(&["targets"], "");
    assert_eq!(plain.status, 0);
    let lines: Vec<_> = std::str::from_utf8(&plain.stdout)
        .unwrap()
        .lines()
        .collect();
    assert_eq!(lines.len(), TARGETS.len());
    assert!(plain.stdout.ends_with(b"\n"));
    for (line, target) in lines.iter().zip(TARGETS) {
        assert_eq!(*line, format!("{}\t{}", target.id, target.label));
    }
    assert_eq!(
        cli.execute(&["targets", "--json"], "").json(),
        serde_json::to_value(TARGETS).unwrap()
    );
    assert_eq!(cli.opens.get(), 0);
}

#[test]
fn stdin_is_default_and_preserves_utf8_across_reads() {
    let cli = Harness::new();
    for (args, input, target) in [
        (&[][..], "[Notes](./notes)", CopyTarget::RichText),
        (
            &["convert", "--to", "plain-text"][..],
            "**😀**",
            CopyTarget::PlainText,
        ),
        (
            &["--to", "plain-text"][..],
            "**Bold**",
            CopyTarget::PlainText,
        ),
        (&[][..], "invalid�utf8", CopyTarget::RichText),
    ] {
        let expected = convert_markdown(input, target, &ConversionOptions::default());
        let output = if input.contains('�') {
            cli.run("copied-md", args, Cursor::new(b"invalid\xffutf8"), false)
        } else {
            cli.execute(args, input)
        };
        assert_eq!(output.status, 0);
        assert_eq!(output.stdout, expected.representations.plain.as_bytes());
        assert_eq!(output.stderr, diagnostics(&expected));
    }
}

#[test]
fn file_conversion_selects_the_requested_representation_and_forwards_options() {
    let cli = Harness::new();
    let markdown = "[Notes](./notes)";
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("document.md");
    fs::write(&path, markdown).unwrap();
    let url = "https://example.org/posts/a";
    for (format, target, select) in [
        ("html", CopyTarget::RichText, Format::Html),
        ("rtf", CopyTarget::Telegram, Format::Rtf),
        ("signal", CopyTarget::Signal, Format::Signal),
        ("plain", CopyTarget::Telegram, Format::Plain),
    ] {
        let expected = convert_markdown(
            markdown,
            target,
            &ConversionOptions {
                source_url: Some(url.into()),
                decorative_unicode: true,
            },
        );
        let args = [
            "convert",
            "--file",
            path.to_str().unwrap(),
            "--to",
            target.id(),
            "--source-url",
            url,
            "--unicode",
            "--format",
            format,
        ];
        let output = cli.execute(&args, "ignored");
        if let Ok(expected_output) = select.select(&expected.representations) {
            assert_eq!(output.status, 0);
            assert_eq!(output.stdout, expected_output.as_bytes());
            assert_eq!(output.stderr, diagnostics(&expected));
        } else {
            let mut args = args.to_vec();
            args.push("--json");
            assert_eq!(
                cli.execute(&args, "").json(),
                serde_json::to_value(expected).unwrap()
            );
            assert_eq!(output.status, 2);
            assert_eq!(output.stdout, b"");
        }
    }
}

#[test]
fn format_selection_routes_each_representation() {
    let formats = ClipboardPayload {
        plain: "Plain".into(),
        html: Some("<p>HTML</p>".into()),
        rtf: Some("{\\rtf1 RTF}".into()),
        signal: Some("<strong>Signal</strong>".into()),
    };
    for (format, expected) in [
        (Format::Plain, "Plain"),
        (Format::Html, "<p>HTML</p>"),
        (Format::Rtf, "{\\rtf1 RTF}"),
        (Format::Signal, "<strong>Signal</strong>"),
    ] {
        assert_eq!(format.select(&formats).unwrap(), expected);
    }
}

#[test]
fn emits_the_entire_schema_without_diagnostics_on_stderr_in_json_mode() {
    let cli = Harness::new();
    let markdown = "**Bold**";
    let json = cli
        .execute(
            &["convert", "--to", "slack", "--format", "html", "--json"],
            markdown,
        )
        .json();
    assert_eq!(json["schemaVersion"], 1);
    assert_eq!(json["target"], "slack-paste");
    assert_eq!(json["metrics"]["sourceCodeUnits"], 8);
    assert_eq!(
        json,
        serde_json::to_value(convert_markdown(
            markdown,
            CopyTarget::SlackPaste,
            &ConversionOptions::default()
        ))
        .unwrap()
    );
}

#[test]
fn rejects_invalid_arguments_before_opening_the_clipboard() {
    let cli = Harness::new();
    for (args, status, code) in [
        (&["convert", "document.md"][..], 2, "usage"),
        (&["unknown"][..], 2, "usage"),
        (&["mcp", "--to", "sms"][..], 2, "usage"),
        (&["mcp"][..], 2, "usage"),
        (&["clipboard", "read", "--clipboard"][..], 2, "usage"),
        (&["convert", "--clipboard"][..], 2, "usage"),
        (&["convert", "--pick"][..], 2, "usage"),
        (
            &["copy", "--file", "document.md", "--clipboard"][..],
            2,
            "usage",
        ),
        (&["copy", "--to", "sms", "--pick"][..], 2, "usage"),
        (&["convert", "--format", "xml"][..], 2, "usage"),
        (&["convert", "--to", "missing"][..], 2, "unknown-target"),
        (&["--unknown"][..], 1, "command-failed"),
    ] {
        let mut args = args.to_vec();
        args.push("--json");
        cli.execute(&args, "").check_error(status, code);
    }
    cli.execute(&["--json", "--file"], "")
        .check_error(1, "command-failed");
    assert_eq!(cli.opens.get(), 0);
}

#[test]
fn reports_missing_input_unreadable_files_and_missing_representation() {
    let cli = Harness::new();
    let missing = cli.run("copied-md", &["convert", "--json"], Cursor::new([]), true);
    missing.check_error(2, "missing-input");
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("missing.md");
    cli.execute(&["convert", "--file", path.to_str().unwrap(), "--json"], "")
        .check_error(1, "read-failed");
    let unavailable = cli.execute(
        &["convert", "--to", "plain-text", "--format", "html"],
        "plain",
    );
    assert_eq!(unavailable.status, 2);
    assert_eq!(unavailable.stdout, b"");
    assert_eq!(unavailable.stderr, b"This target does not provide HTML.\n");
}

#[test]
fn opens_mcp_with_no_args_and_surfaces_transport_errors() {
    let cli = Harness::new();
    let output = cli.execute(&["mcp"], "");
    assert_eq!(output.status, 0);
    assert!(output.stdout.is_empty() && output.stderr.is_empty());
    let output = cli.run("copied-md", &["mcp"], BufReader::new(Broken), false);
    assert_eq!(output.status, 1);
    assert_eq!(output.stderr, b"broken input\n");
    cli.run(
        "copied-md",
        &["convert", "--json"],
        BufReader::new(Broken),
        false,
    )
    .check_error(1, "read-failed");
}

struct Broken;

impl io::Read for Broken {
    fn read(&mut self, _: &mut [u8]) -> io::Result<usize> {
        Err(io::Error::other("broken input"))
    }
}

#[test]
fn copy_picker_can_cancel_or_supply_an_invalid_target() {
    let mut cli = Harness::new();
    let cancelled = cli.execute(&["copy", "--pick"], "hello");
    assert_eq!(cancelled.status, 0);
    assert!(cancelled.stdout.is_empty() && cancelled.stderr.is_empty());
    cli.pick = Some("invalid");
    cli.execute(&["copy", "--pick", "--json"], "hello")
        .check_error(2, "unknown-target");
    assert_eq!(cli.picks.get(), 2);
    assert_eq!(cli.opens.get(), 0);
}

#[test]
fn copy_picker_writes_the_conversion_and_remembers_a_successful_target() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("config.json");
    let mut cli = Harness::new();
    cli.pick = Some("telegram");
    cli.config_path = Some(path.clone());
    let expected = convert_markdown(
        "**Source**",
        CopyTarget::Telegram,
        &ConversionOptions::default(),
    );
    let mut output = cli
        .execute(&["copy", "--clipboard", "--pick", "--json"], "")
        .json();
    let receipt = output.as_object_mut().unwrap().remove("clipboard").unwrap();
    let mut board = cli.board.lock().unwrap();
    assert_eq!(board.current_payload(), Some(&expected.representations));
    assert_eq!(board.write_calls(), 1);
    let mut expected_board = FakeBoard::default();
    let types = expected_board
        .write(&expected.representations, None)
        .unwrap()
        .types;
    assert_eq!(receipt, json!({"types":types}));
    assert_eq!(output, serde_json::to_value(expected).unwrap());
    assert_eq!(
        load_config(&path).unwrap().last_target,
        Some(CopyTarget::Telegram)
    );

    board.on_before_clear = Some(|board| {
        board.external_copy("newer");
    });
    drop(board);
    cli.execute(&["copy", "--clipboard", "--to", "sms", "--json"], "")
        .check_error(3, "clipboard-changed");
    let board = cli.board.lock().unwrap();
    assert_eq!(board.read_text().unwrap().text, "newer");
    assert_eq!(board.write_calls(), 1);
    assert_eq!(
        load_config(&path).unwrap().last_target,
        Some(CopyTarget::Telegram)
    );
}

#[test]
fn copy_rejects_empty_input_or_rendered_whitespace_and_prints_diagnostics() {
    let cli = Harness::new();
    cli.execute(&["copy", "--json"], "  \n\t")
        .check_error(2, "empty-output");
    assert_eq!(cli.opens.get(), 0);
    let markdown = "[x]: https://example.org";
    let expected = convert_markdown(
        markdown,
        CopyTarget::RichText,
        &ConversionOptions::default(),
    );
    let output = cli.execute(&["copy", "--json"], markdown);
    if expected.representations.plain.trim().is_empty() {
        output.check_error(2, "empty-output");
        assert_eq!(cli.opens.get(), 0);
    } else {
        assert_eq!(
            output.json()["representations"]["text/plain"],
            expected.representations.plain
        );
    }

    let markdown = "[Link](./relative)";
    let expected = convert_markdown(
        markdown,
        CopyTarget::PlainText,
        &ConversionOptions::default(),
    );
    let output = cli.execute(&["copy", "--to", "plain-text"], markdown);
    assert_eq!(output.status, 0);
    assert_eq!(output.stdout, b"");
    assert_eq!(output.stderr, diagnostics(&expected));
    let mut stderr = Vec::new();
    write_diagnostics(
        &mut stderr,
        &[Diagnostic {
            code: DiagnosticCode::RelativeUrl,
            severity: DiagnosticSeverity::Warning,
            message: "Use the original page URL.".into(),
        }],
    )
    .unwrap();
    assert_eq!(stderr, b"Use the original page URL.\n");
}

#[test]
fn copy_maps_backend_failure_codes_and_never_touches_the_config_on_error() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("config.json");
    let mut cli = Harness::new();
    cli.config_path = Some(path.clone());
    cli.open_error = Some(WriteError::Unavailable("no display".into()));
    cli.execute(&["copy", "--json"], "hello")
        .check_error(1, "clipboard-unavailable");
    cli.open_error = None;
    cli.board.lock().unwrap().fail_write = true;
    cli.execute(&["copy", "--json"], "hello")
        .check_error(1, "clipboard-failed");
    cli.execute(&["clipboard", "read", "--json"], "")
        .check_error(1, "empty-output");
    assert!(!path.exists());
}

#[test]
fn clipboard_helper_reads_writes_and_does_not_remember_stale_target() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("config.json");
    let initial = ConfigV2 {
        picker_columns: 5,
        ..ConfigV2::default()
    };
    save_config(&path, &initial).unwrap();
    let mut cli = Harness::new();
    cli.config_path = Some(path.clone());
    let before = cli.execute(&["clipboard", "read"], "").json();
    assert_eq!(before, json!({"text":"**Source**","changeCount":0}));
    let message = r#"{"representations":{"text/plain":"new","text/html":"<p>new</p>","text/rtf":"rtf","text/signal":"<b>new</b>"},"expectedChangeCount":0,"target":"slack"}"#;
    let written = cli
        .run(
            "/bin/copied-md-pasteboard.exe",
            &["write"],
            Cursor::new(message),
            false,
        )
        .json();
    assert_eq!(
        written,
        json!({"types":["text/plain","text/html","text/rtf","text/signal"]})
    );
    assert_eq!(
        cli.execute(&["clipboard", "read"], "").json(),
        json!({"text":"new","changeCount":1})
    );
    let request =
        r#"{"representations":{"text/plain":"stale"},"expectedChangeCount":0,"target":"sms"}"#;
    cli.execute(&["clipboard", "write", "--json"], request)
        .check_error(3, "clipboard-changed");
    assert_eq!(
        load_config(&path).unwrap(),
        ConfigV2 {
            last_target: Some(CopyTarget::SlackPaste),
            ..initial
        }
    );
    let no_target = r#"{"representations":{"text/plain":"unconditional","text/html":"","text/rtf":"","text/signal":""}}"#;
    assert_eq!(
        cli.execute(&["clipboard", "write"], no_target).json(),
        json!({"types":["text/plain"]})
    );
    assert_eq!(
        load_config(&path).unwrap().last_target,
        Some(CopyTarget::SlackPaste)
    );
    let repeated_target = r#"{"representations":{"text/plain":"again"},"target":"slack"}"#;
    assert_eq!(
        cli.execute(&["clipboard", "write"], repeated_target).json(),
        json!({"types":["text/plain"]})
    );
}

#[test]
fn clipboard_helper_validates_requests_before_writing() {
    let cli = Harness::new();
    for invalid in [
        "broken",
        "{}",
        r#"{"representations":{}}"#,
        r#"{"representations":{"text/plain":"text"},"expectedChangeCount":-1}"#,
    ] {
        cli.execute(&["clipboard", "write", "--json"], invalid)
            .check_error(1, "invalid-clipboard-request");
    }
    cli.execute(
        &["clipboard", "write", "--json"],
        r#"{"representations":{"text/plain":"text"},"target":"invalid"}"#,
    )
    .check_error(2, "unknown-target");
    cli.execute(&["clipboard", "choose", "--json"], "{}")
        .check_error(1, "invalid-clipboard-request");
    assert_eq!(cli.opens.get(), 0);
    cli.execute(
        &["clipboard", "write", "--json"],
        r#"{"representations":{"text/plain":" "}}"#,
    )
    .check_error(1, "empty-output");
}

#[test]
fn clipboard_choose_matches_helper_json_for_cancel_and_selection() {
    let mut cli = Harness::new();
    let input = r#"[{"id":"sms","label":"SMS"}]"#;
    assert_eq!(
        cli.execute(&["clipboard", "choose"], input).json(),
        json!({})
    );
    cli.pick = Some("sms");
    assert_eq!(
        cli.execute(&["clipboard", "choose"], input).json(),
        json!({"target":"sms"})
    );
    assert_eq!(cli.opens.get(), 0);
}

#[test]
fn clipboard_self_test_is_isolated() {
    let cli = Harness::new();
    let result = cli.execute(&["clipboard", "self-test"], "");
    assert_eq!(result.json(), json!({"types":["text/plain","text/html"]}));
    assert_eq!(cli.opens.get(), 0);
    assert_eq!(
        pasteboard_compat::fake_self_test().unwrap().types,
        ["text/plain", "text/html"]
    );
}

#[test]
fn clipboard_config_errors_surface_and_the_copy_still_commits() {
    let cli = Harness::new();
    let directory = tempfile::tempdir().unwrap();
    let file = directory.path().join("config.json");
    fs::write(&file, "broken").unwrap();
    let mut cli = Harness {
        config_path: Some(file),
        ..cli
    };
    let unavailable = Harness {
        open_error: Some(WriteError::Unavailable("no desktop".into())),
        ..Harness::new()
    };
    unavailable
        .execute(&["clipboard", "read", "--json"], "")
        .check_error(1, "clipboard-unavailable");

    let request = r#"{"representations":{"text/plain":"new"},"target":"rich-text"}"#;
    cli.execute(&["clipboard", "write", "--json"], request)
        .check_error(1, "config-failed");
    assert_eq!(cli.board.lock().unwrap().read_text().unwrap().text, "new");
    cli.pick = Some("rich-text");
    cli.execute(&["copy", "--pick", "--json"], "hello")
        .check_error(1, "config-failed");
}
