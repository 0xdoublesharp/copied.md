use copied_config::{ConfigV2, load_config};
use copied_core::TARGETS;
use serde::{Deserialize, Serialize};
use std::process::{Command, Stdio};

use crate::cli::{Failure, Services};

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct PickerTarget {
    id: String,
    label: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    enabled_by_default: Option<bool>,
}

#[derive(Debug, Deserialize, Serialize)]
pub(crate) struct Choice {
    #[serde(skip_serializing_if = "Option::is_none")]
    target: Option<String>,
}

pub(crate) type Chooser<'a> =
    dyn FnMut(&[PickerTarget], Option<&str>, bool) -> Result<Option<String>, Failure> + 'a;

pub(crate) fn pick(services: &mut Services<'_>) -> Result<Option<String>, Failure> {
    let targets: Vec<_> = TARGETS
        .iter()
        .map(|target| PickerTarget {
            id: target.id.id().into(),
            label: target.label.to_string(),
            enabled_by_default: target.enabled_by_default,
        })
        .collect();
    choose(services, &targets, true)
}

pub(crate) fn choose_supplied(
    services: &mut Services<'_>,
    targets: &[PickerTarget],
) -> Result<Choice, Failure> {
    Ok(Choice {
        target: choose(services, targets, false)?,
    })
}

fn choose(
    services: &mut Services<'_>,
    targets: &[PickerTarget],
    use_configured_helper: bool,
) -> Result<Option<String>, Failure> {
    let config = services
        .config_path
        .as_deref()
        .map_or_else(|| Ok(ConfigV2::default()), load_config)
        .map_err(|error| Failure::system("config-failed", error))?;
    let mut targets = targets.to_vec();
    for target in &mut targets {
        if let Some(visible) = copied_core::resolve_target(&target.id)
            .and_then(|target| config.format_visibility.get(&target))
        {
            target.enabled_by_default = Some(*visible);
        }
    }
    (services.choose)(
        &targets,
        config.last_target.map(copied_core::CopyTarget::id),
        use_configured_helper,
    )
}

pub(crate) fn system_choose(
    targets: &[PickerTarget],
    preferred: Option<&str>,
    use_configured_helper: bool,
) -> Result<Option<String>, Failure> {
    if use_configured_helper
        && let Some(helper) =
            std::env::var_os("COPIED_MD_PASTEBOARD").filter(|path| !path.is_empty())
    {
        return helper_choose(Command::new(helper), targets);
    }
    choose_visible(targets, preferred, native_choose)
}

fn choose_visible(
    targets: &[PickerTarget],
    preferred: Option<&str>,
    choose: impl FnOnce(&[&PickerTarget], usize) -> Result<Option<String>, Failure>,
) -> Result<Option<String>, Failure> {
    if targets.is_empty() {
        return Err(Failure::new(
            1,
            "clipboard-failed",
            "There are no available formats.",
        ));
    }

    let mut visible: Vec<_> = targets
        .iter()
        .filter(|target| target.enabled_by_default != Some(false))
        .collect();
    if visible.is_empty() {
        visible.extend(targets);
    }
    let selected = visible
        .iter()
        .position(|target| Some(target.id.as_str()) == preferred)
        .unwrap_or(0);
    choose(&visible, selected)
}

fn helper_choose(
    mut command: Command,
    targets: &[PickerTarget],
) -> Result<Option<String>, Failure> {
    let mut child = command
        .arg("choose")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|_| {
            Failure::new(
                1,
                "clipboard-helper-unavailable",
                "Install copied-md-pasteboard or set COPIED_MD_PASTEBOARD.",
            )
        })?;
    let mut input = child.stdin.take().expect("helper stdin is piped");
    let sent = serde_json::to_writer(&mut input, targets);
    drop(input);
    let output = child.wait_with_output()?;
    if !output.status.success() {
        let message = String::from_utf8_lossy(&output.stderr);
        if output.status.code() == Some(3) {
            return Err(stale_helper_error(message.trim()));
        }
        return Err(Failure::new(
            1,
            "clipboard-failed",
            if message.trim().is_empty() {
                "The clipboard operation failed."
            } else {
                message.trim()
            },
        ));
    }
    sent.map_err(|error| Failure::system("clipboard-failed", error))?;
    serde_json::from_slice::<Choice>(&output.stdout)
        .map(|choice| choice.target)
        .map_err(|_| {
            Failure::new(
                1,
                "clipboard-helper-response",
                "The clipboard helper returned an invalid response.",
            )
        })
}

#[cfg(target_os = "macos")]
fn native_choose(targets: &[&PickerTarget], selected: usize) -> Result<Option<String>, Failure> {
    mac_dialog(Command::new("/usr/bin/osascript"), targets, selected)
}

#[cfg(target_os = "macos")]
fn mac_dialog(
    mut command: Command,
    targets: &[&PickerTarget],
    selected: usize,
) -> Result<Option<String>, Failure> {
    const SCRIPT: &str = r#"on run argv
        set choices to items 2 thru -1 of argv
        set selectedItem to item (item 1 of argv as integer) of choices
        set resultList to choose from list choices with title "Copy Markdown As…" default items {selectedItem} OK button name "Copy"
        if resultList is false then return ""
        return item 1 of resultList
    end run"#;
    let choices: Vec<_> = targets
        .iter()
        .enumerate()
        .map(|(index, target)| format!("{}. {}", index + 1, target.label))
        .collect();
    let output = command
        .args(["-e", SCRIPT, "--", &(selected + 1).to_string()])
        .args(&choices)
        .output()
        .map_err(|error| Failure::system("clipboard-picker-unavailable", error))?;
    if !output.status.success() {
        return Err(Failure::system(
            "clipboard-failed",
            String::from_utf8_lossy(&output.stderr),
        ));
    }
    let label = String::from_utf8_lossy(&output.stdout);
    Ok(choices
        .iter()
        .position(|choice| choice == label.trim_end_matches(['\r', '\n']))
        .map(|index| targets[index].id.clone()))
}

#[cfg(not(target_os = "macos"))]
fn native_choose(targets: &[&PickerTarget], selected: usize) -> Result<Option<String>, Failure> {
    use std::{
        fs::File,
        io::{BufRead as _, BufReader, Write as _},
    };

    #[cfg(windows)]
    let (input, output) = ("CONIN$", "CONOUT$");
    #[cfg(not(windows))]
    let (input, output) = ("/dev/tty", "/dev/tty");
    let unavailable = |_| {
        Failure::new(
            1,
            "clipboard-picker-unavailable",
            "Open a terminal to pick a target or use --to.",
        )
    };
    let mut input = BufReader::new(File::open(input).map_err(unavailable)?);
    let mut output = File::options()
        .write(true)
        .open(output)
        .map_err(unavailable)?;
    for (index, target) in targets.iter().enumerate() {
        writeln!(output, "{}. {}", index + 1, target.label)?;
    }
    write!(
        output,
        "Choose a format [{}] or q to cancel: ",
        selected + 1
    )?;
    output.flush()?;
    let mut choice = String::new();
    if input.read_line(&mut choice)? == 0 || choice.trim() == "q" {
        return Ok(None);
    }
    let index = if choice.trim().is_empty() {
        Some(selected)
    } else {
        choice
            .trim()
            .parse::<usize>()
            .ok()
            .and_then(|index| index.checked_sub(1))
    };
    index
        .and_then(|index| targets.get(index))
        .map(|target| Some(target.id.clone()))
        .ok_or_else(|| Failure::new(2, "usage", "Choose a number from the format list."))
}

fn stale_helper_error(message: &str) -> Failure {
    if message.is_empty() {
        copied_clipboard::WriteError::Stale.into()
    } else {
        Failure::new(3, "clipboard-changed", message)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn targets() -> Vec<PickerTarget> {
        serde_json::from_str(
            r#"[{"id":"sms","label":"SMS"},
                {"id":"teams","label":"Teams","enabledByDefault":false},
                {"id":"telegram","label":"Telegram"}]"#,
        )
        .unwrap()
    }

    #[test]
    fn the_picker_honors_visible_and_remembered_targets() {
        let targets = targets();
        let selected =
            |visible: &[&PickerTarget], selected: usize| Ok(Some(visible[selected].id.clone()));
        assert_eq!(
            choose_visible(&targets, Some("telegram"), selected)
                .unwrap()
                .as_deref(),
            Some("telegram")
        );
        assert_eq!(
            choose_visible(&targets, Some("teams"), selected)
                .unwrap()
                .as_deref(),
            Some("sms")
        );
        assert_eq!(
            choose_visible(&targets[1..2], None, selected)
                .unwrap()
                .as_deref(),
            Some("teams")
        );
        assert_eq!(
            serde_json::to_value(choose_visible(&[], None, selected).unwrap_err()).unwrap()["message"],
            "There are no available formats."
        );
    }

    #[test]
    fn config_visibility_overrides_the_supplied_catalog() {
        use copied_core::CopyTarget;
        use std::collections::BTreeMap;

        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("config.json");
        copied_config::save_config(
            &path,
            &ConfigV2 {
                last_target: Some(CopyTarget::Teams),
                format_visibility: BTreeMap::from([
                    (CopyTarget::Teams, true),
                    (CopyTarget::Sms, false),
                ]),
                ..ConfigV2::default()
            },
        )
        .unwrap();
        let mut choose =
            |targets: &[PickerTarget], preferred: Option<&str>, use_configured_helper: bool| {
                assert!(!use_configured_helper);
                choose_visible(targets, preferred, |visible, selected| {
                    assert_eq!(
                        visible
                            .iter()
                            .map(|target| target.id.as_str())
                            .collect::<Vec<_>>(),
                        ["teams", "telegram"]
                    );
                    Ok(Some(visible[selected].id.clone()))
                })
            };
        let mut services = Services {
            config_path: Some(path),
            open_board: &mut || unreachable!("the picker is read-only"),
            choose: &mut choose,
            #[cfg(target_os = "linux")]
            linux_helper: false,
        };
        assert_eq!(
            choose_supplied(&mut services, &targets())
                .unwrap()
                .target
                .as_deref(),
            Some("teams")
        );
    }

    #[cfg(unix)]
    fn shell(script: &str) -> Command {
        let mut command = Command::new("/bin/sh");
        command.args(["-c", script]);
        command
    }

    #[cfg(unix)]
    #[test]
    fn external_helper_choice_and_cancel_use_the_legacy_envelope() {
        let targets = targets();
        let choice = helper_choose(
            shell(r#"test "$0" = choose && test -n "$(cat)" && printf '{"target":"telegram"}'"#),
            &targets,
        )
        .unwrap();
        assert_eq!(choice.as_deref(), Some("telegram"));
        assert_eq!(
            helper_choose(shell("cat >/dev/null && printf '{}'"), &targets).unwrap(),
            None,
        );
    }

    #[cfg(unix)]
    #[test]
    fn external_helper_spawn_status_and_response_errors_are_distinct() {
        let targets = targets();
        for (command, code, message) in [
            (
                Command::new("/no-such-copied-md-pasteboard"),
                "clipboard-helper-unavailable",
                "Install copied-md-pasteboard or set COPIED_MD_PASTEBOARD.",
            ),
            (
                shell("cat >/dev/null && printf stale >&2 && exit 3"),
                "clipboard-changed",
                "stale",
            ),
            (
                shell("cat >/dev/null && exit 3"),
                "clipboard-changed",
                "The clipboard changed. Copy the Markdown again.",
            ),
            (
                shell("cat >/dev/null && exit 1"),
                "clipboard-failed",
                "The clipboard operation failed.",
            ),
            (
                shell("cat >/dev/null && printf failed >&2 && exit 1"),
                "clipboard-failed",
                "failed",
            ),
            (
                shell("cat >/dev/null && printf broken"),
                "clipboard-helper-response",
                "The clipboard helper returned an invalid response.",
            ),
        ] {
            let error = helper_choose(command, &targets).unwrap_err();
            assert_eq!(
                serde_json::to_value(error).unwrap(),
                serde_json::json!({"code":code,"message":message}),
            );
        }
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn mac_dialog_translates_choice_cancel_and_process_errors() {
        let targets = targets();
        let options = [&targets[0], &targets[2]];
        assert_eq!(
            mac_dialog(shell("printf '%s\\n' \"$5\""), &options, 1)
                .unwrap()
                .as_deref(),
            Some("telegram")
        );
        assert_eq!(mac_dialog(shell("true"), &options, 1).unwrap(), None);
        assert!(mac_dialog(shell("exit 1"), &options, 1).is_err());
        assert!(mac_dialog(Command::new("/no-such-osascript"), &options, 1).is_err());
    }
}
