use copied_clipboard::{ClipboardSnapshot, Revision, WriteError, WrittenClipboard};
use copied_config::{load_config, save_config};
use copied_core::{ClipboardPayload, CopyTarget};
use serde::{Deserialize, Serialize};
use std::{
    io::{BufRead, Write},
    path::Path,
};

use crate::{
    cli::{Failure, Services, target_id, write_json},
    picker::{self, PickerTarget},
};

#[derive(Clone, Copy, Debug, clap::Subcommand)]
pub(crate) enum Action {
    /// Print the current clipboard text and its change count.
    Read,
    /// Write a representations bundle from stdin.
    Write,
    /// Test a round trip and stale-write rejection on an isolated clipboard.
    SelfTest,
    /// Open a destination picker for a JSON array of targets from stdin.
    Choose,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct WriteRequest {
    representations: ClipboardPayload,
    expected_change_count: Option<Revision>,
    target: Option<String>,
}

#[derive(Debug, Serialize)]
pub(crate) struct Written {
    types: Vec<String>,
}

impl From<WrittenClipboard> for Written {
    fn from(value: WrittenClipboard) -> Self {
        Self { types: value.types }
    }
}

pub(crate) fn run(
    action: Action,
    input: &mut impl BufRead,
    output: &mut impl Write,
    services: &mut Services<'_>,
) -> Result<(), Failure> {
    match action {
        Action::Read => {
            #[derive(Serialize)]
            #[serde(rename_all = "camelCase")]
            struct Snapshot {
                text: String,
                change_count: Revision,
            }
            let snapshot = read_snapshot(services).map_err(read_error)?;
            write_json(
                output,
                &Snapshot {
                    text: snapshot.text,
                    change_count: snapshot.revision,
                },
            )?;
        }
        Action::Write => {
            let request: WriteRequest = serde_json::from_reader(input).map_err(invalid_request)?;
            let target = request.target.as_deref().map(target_id).transpose()?;
            let written = write_request(services, &request)?;
            if let Some(target) = target {
                remember_target(services.config_path.as_deref(), target)?;
            }
            write_json(output, &Written::from(written))?;
        }
        Action::SelfTest => write_json(output, &Written::from(self_test()?))?,
        Action::Choose => {
            let targets: Vec<PickerTarget> =
                serde_json::from_reader(input).map_err(invalid_request)?;
            write_json(output, &picker::choose_supplied(services, &targets)?)?;
        }
    }
    Ok(())
}

fn read_snapshot(services: &mut Services<'_>) -> Result<ClipboardSnapshot, WriteError> {
    #[cfg(target_os = "linux")]
    if services.linux_helper {
        return crate::linux_owner::read_guarded();
    }
    (services.open_board)()?.read_text()
}

fn write_request(
    services: &mut Services<'_>,
    request: &WriteRequest,
) -> Result<WrittenClipboard, WriteError> {
    #[cfg(target_os = "linux")]
    if services.linux_helper
        && let Some(token) = request.expected_change_count
    {
        return crate::linux_owner::write_guarded(&request.representations, token);
    }
    (services.open_board)()?.write(&request.representations, request.expected_change_count)
}

fn invalid_request(error: serde_json::Error) -> Failure {
    Failure::system("invalid-clipboard-request", error)
}

pub(crate) fn read_error(error: WriteError) -> Failure {
    if error == WriteError::Empty {
        return Failure::new(1, "empty-output", "The clipboard has no text.");
    }
    error.into()
}

pub(crate) fn remember_target(path: Option<&Path>, target: CopyTarget) -> Result<(), Failure> {
    let Some(path) = path else {
        return Ok(());
    };
    let mut config = load_config(path).map_err(|error| Failure::system("config-failed", error))?;
    if config.last_target != Some(target) {
        config.last_target = Some(target);
        save_config(path, &config).map_err(|error| Failure::system("config-failed", error))?;
    }
    Ok(())
}

#[cfg(target_os = "macos")]
fn self_test() -> Result<WrittenClipboard, WriteError> {
    copied_clipboard::macos::self_test()
}

#[cfg(not(target_os = "macos"))]
fn self_test() -> Result<WrittenClipboard, WriteError> {
    fake_self_test()
}

#[cfg(any(not(target_os = "macos"), test))]
pub(crate) fn fake_self_test() -> Result<WrittenClipboard, WriteError> {
    use copied_clipboard::{Board as _, FakeBoard};

    let mut board = FakeBoard::default();
    let payload = ClipboardPayload {
        plain: "# Example".into(),
        html: Some("<h1>Example</h1>".into()),
        ..ClipboardPayload::default()
    };
    let written = board.write(&payload, Some(board.revision()?))?;
    let snapshot = board.read_text()?;
    if snapshot.text != payload.plain
        || snapshot.revision != written.revision
        || board.current_payload() != Some(&payload)
        || written.types != ["text/plain", "text/html"]
    {
        return Err(WriteError::Rejected(
            "Pasteboard types did not round-trip.".into(),
        ));
    }
    board.external_copy("new");
    if board.write(&payload, Some(snapshot.revision)) != Err(WriteError::Stale)
        || board.read_text()?.text != "new"
    {
        return Err(WriteError::Rejected(
            "A stale clipboard write was accepted.".into(),
        ));
    }
    Ok(written)
}
