use std::io::Write;
use std::process::{Command, Output, Stdio};

fn binary(args: &[&str], input: &str) -> Output {
    invoke(
        Command::new(env!("CARGO_BIN_EXE_copied-md")).args(args),
        input,
    )
}

fn invoke(command: &mut Command, input: &str) -> Output {
    let mut child = command
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    child
        .stdin
        .take()
        .unwrap()
        .write_all(input.as_bytes())
        .unwrap();
    child.wait_with_output().unwrap()
}

#[test]
fn default_command_converts_piped_markdown() {
    let output = binary(&[], "hello");
    assert!(output.status.success());
    assert_eq!(output.stdout, b"hello");
    assert_eq!(output.stderr, b"");
}

#[test]
fn errors_use_stderr_and_json_envelope() {
    let output = binary(&["convert", "--to", "missing", "--json"], "hello");
    assert_eq!(output.status.code(), Some(2));
    assert_eq!(output.stdout, b"");
    let error: serde_json::Value = serde_json::from_slice(&output.stderr).unwrap();
    assert_eq!(error["error"]["code"], "unknown-target");
}

#[test]
fn mcp_stdio_carries_only_protocol_responses() {
    let output = binary(
        &["mcp"],
        "{\"jsonrpc\":\"2.0\",\"method\":\"notifications/initialized\"}\n{\"jsonrpc\":\"2.0\",\"id\":7,\"method\":\"ping\"}\n",
    );
    assert!(output.status.success());
    assert_eq!(output.stderr, b"");
    let response: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(
        response,
        serde_json::json!({"jsonrpc":"2.0","id":7,"result":{}})
    );
}

#[cfg(unix)]
#[test]
fn pasteboard_alias_runs_the_isolated_self_test() {
    use std::os::unix::process::CommandExt as _;

    for name in ["copied-md-pasteboard", "copied-md-pasteboard.exe"] {
        let mut command = Command::new(env!("CARGO_BIN_EXE_copied-md"));
        let output = invoke(command.arg0(name).arg("self-test"), "");
        assert!(output.status.success(), "{:?}", output.stderr);
        assert_eq!(output.stderr, b"");
        let written: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
        assert_eq!(
            written,
            serde_json::json!({"types":["text/plain","text/html"]})
        );
    }
}

#[cfg(any(target_os = "macos", target_os = "linux", target_os = "windows"))]
#[test]
fn impossible_clipboard_revision_exits_three() {
    let output = binary(
        &["clipboard", "write", "--json"],
        r#"{"representations":{"text/plain":"stale"},"expectedChangeCount":18446744073709551615}"#,
    );
    assert_eq!(output.status.code(), Some(3), "{:?}", output.stderr);
    assert_eq!(output.stdout, b"");
    let error: serde_json::Value = serde_json::from_slice(&output.stderr).unwrap();
    assert_eq!(error["error"]["code"], "clipboard-changed");
}

#[cfg(target_os = "linux")]
#[test]
#[ignore = "requires a private X11 or Wayland display"]
fn linux_process_keeps_the_selection_and_guards_helper_tokens() {
    use serde_json::{Value, json};

    let config = tempfile::tempdir().unwrap();
    let run = |args: &[&str], input: &str| {
        invoke(
            Command::new(env!("CARGO_BIN_EXE_copied-md"))
                .args(args)
                .env("XDG_CONFIG_HOME", config.path()),
            input,
        )
    };
    let seed = |text: &str| {
        let request = json!({"representations":{"text/plain":text}});
        let output = run(&["clipboard", "write"], &request.to_string());
        assert!(output.status.success(), "{:?}", output.stderr);
    };
    let snapshot = || {
        let output = run(&["clipboard", "read"], "");
        assert!(output.status.success(), "{:?}", output.stderr);
        serde_json::from_slice::<Value>(&output.stdout).unwrap()
    };

    seed("original");
    let before = snapshot();
    assert_eq!(before["text"], "original");
    let write = |text: &str, revision: &Value| {
        let request = json!({
            "representations":{"text/plain":text,"text/html":"<p>Rich</p>"},
            "expectedChangeCount": revision,
        });
        run(&["clipboard", "write", "--json"], &request.to_string())
    };
    let output = write("guarded", &before["changeCount"]);
    assert!(output.status.success(), "{:?}", output.stderr);
    assert_eq!(
        serde_json::from_slice::<Value>(&output.stdout).unwrap(),
        json!({"types":["text/plain","text/html"]}),
    );
    let guarded = snapshot();
    assert_eq!(guarded["text"], "guarded");
    assert_eq!(
        write("retry", &before["changeCount"]).status.code(),
        Some(3)
    );

    seed("changed");
    let rejected = write("stale", &guarded["changeCount"]);
    assert_eq!(rejected.status.code(), Some(3), "{:?}", rejected.stderr);
    assert_eq!(snapshot()["text"], "changed");

    let output = run(&["copy", "--clipboard", "--to", "plain-text"], "");
    assert!(output.status.success(), "{:?}", output.stderr);
    assert_eq!(snapshot()["text"], "changed");
}

#[test]
fn helper_choose_rejects_an_empty_catalog_without_opening_a_dialog() {
    let temp = tempfile::tempdir().unwrap();
    let output = invoke(
        Command::new(env!("CARGO_BIN_EXE_copied-md"))
            .env("HOME", temp.path())
            .env("XDG_CONFIG_HOME", temp.path())
            .args(["clipboard", "choose", "--json"]),
        "[]",
    );
    assert_eq!(output.status.code(), Some(1));
    let error: serde_json::Value = serde_json::from_slice(&output.stderr).unwrap();
    assert_eq!(error["error"]["message"], "There are no available formats.");
}

#[cfg(unix)]
#[test]
fn copy_pick_respects_a_configured_helper() {
    use std::os::unix::fs::PermissionsExt as _;

    let temp = tempfile::tempdir().unwrap();
    let helper = temp.path().join("configured-helper");
    std::fs::write(
        &helper,
        r#"#!/bin/sh
test "$1" = choose && cat >/dev/null && printf '{}'
"#,
    )
    .unwrap();
    std::fs::set_permissions(&helper, std::fs::Permissions::from_mode(0o700)).unwrap();
    let output = invoke(
        Command::new(env!("CARGO_BIN_EXE_copied-md"))
            .env("HOME", temp.path())
            .env("XDG_CONFIG_HOME", temp.path())
            .env("COPIED_MD_PASTEBOARD", helper)
            .args(["copy", "--pick"]),
        "hello",
    );
    assert!(output.status.success(), "{:?}", output.stderr);
    assert_eq!(output.stdout, b"");
    assert_eq!(output.stderr, b"");
}
