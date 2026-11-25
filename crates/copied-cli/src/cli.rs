use clap::{Parser, Subcommand, ValueEnum, error::ErrorKind};
use copied_clipboard::{Board, WriteError};
use copied_core::{
    ClipboardPayload, ConversionOptions, CopyTarget, Diagnostic, MarkdownConversion, TARGETS,
    convert_markdown, resolve_target,
};
use serde::Serialize;
use std::{
    ffi::{OsStr, OsString},
    fmt::Display,
    fs,
    io::{self, BufRead, Write},
    path::{Path, PathBuf},
};

use crate::{mcp, pasteboard_compat, picker};

#[derive(Debug, Parser)]
#[command(
    name = "copied-md",
    about = "Convert Markdown for copying into other apps.",
    after_help = "Convert (the default command) and copy read stdin unless --file is supplied.\n--source-url resolves relative links. --unicode opts into decorative styling.",
    args_override_self = true
)]
#[expect(
    clippy::struct_excessive_bools,
    reason = "the independent flags mirror the CLI"
)]
struct Arguments {
    #[command(subcommand)]
    command: Option<Command>,
    #[arg(long, global = true, value_name = "TARGET")]
    to: Option<String>,
    #[arg(long, global = true, value_name = "PATH")]
    file: Option<PathBuf>,
    #[arg(long, global = true, value_enum)]
    format: Option<Format>,
    #[arg(long, global = true, value_name = "URL")]
    source_url: Option<String>,
    #[arg(long, global = true)]
    unicode: bool,
    #[arg(long, global = true)]
    json: bool,
    #[arg(long, global = true)]
    clipboard: bool,
    #[arg(long, global = true)]
    pick: bool,
}

#[derive(Debug, Subcommand)]
enum Command {
    /// List copy targets.
    Targets,
    /// Convert Markdown and print the result.
    Convert,
    /// Convert Markdown and write to the system clipboard.
    Copy,
    /// Read, write, or test the clipboard using the pasteboard helper protocol.
    Clipboard {
        #[command(subcommand)]
        action: pasteboard_compat::Action,
    },
    /// Serve the Model Context Protocol over stdin and stdout.
    Mcp,
}

#[derive(Clone, Copy, Debug, ValueEnum)]
enum Format {
    Plain,
    Html,
    Rtf,
    Signal,
}

impl Format {
    fn select(self, payload: &ClipboardPayload) -> Result<&str, Failure> {
        let (value, name) = match self {
            Self::Plain => return Ok(&payload.plain),
            Self::Html => (payload.html.as_deref(), "HTML"),
            Self::Rtf => (payload.rtf.as_deref(), "RTF"),
            Self::Signal => (payload.signal.as_deref(), "Signal"),
        };
        value.ok_or_else(|| {
            Failure::new(
                2,
                "representation-unavailable",
                format!("This target does not provide {name}."),
            )
        })
    }
}

#[derive(Debug, Serialize)]
pub(crate) struct Failure {
    #[serde(skip)]
    status: u8,
    code: &'static str,
    message: String,
}

impl Failure {
    pub(crate) fn new(status: u8, code: &'static str, message: impl Into<String>) -> Self {
        Self {
            status,
            code,
            message: message.into(),
        }
    }

    fn usage(message: impl Into<String>) -> Self {
        Self::new(2, "usage", message)
    }

    pub(crate) fn system(code: &'static str, error: impl Display) -> Self {
        Self::new(1, code, error.to_string())
    }

    fn empty_output() -> Self {
        Self::new(2, "empty-output", "There is no text to copy.")
    }
}

impl From<io::Error> for Failure {
    fn from(error: io::Error) -> Self {
        Self::system("command-failed", error)
    }
}

impl From<WriteError> for Failure {
    fn from(error: WriteError) -> Self {
        let (status, code) = match error {
            WriteError::Stale => (3, "clipboard-changed"),
            WriteError::Empty => (1, "empty-output"),
            WriteError::Unavailable(_) => (1, "clipboard-unavailable"),
            WriteError::Rejected(_) => (1, "clipboard-failed"),
        };
        Self::new(status, code, error.to_string())
    }
}

pub(crate) fn target_id(value: &str) -> Result<CopyTarget, Failure> {
    resolve_target(value)
        .ok_or_else(|| Failure::new(2, "unknown-target", format!("Unknown copy target {value}.")))
}

pub(crate) fn write_json(output: &mut impl Write, value: &impl Serialize) -> io::Result<()> {
    serde_json::to_writer(&mut *output, value)?;
    output.write_all(b"\n")
}

pub(crate) struct Services<'a> {
    pub(crate) open_board: &'a mut dyn FnMut() -> Result<Box<dyn Board>, WriteError>,
    pub(crate) choose: &'a mut picker::Chooser<'a>,
    pub(crate) config_path: Option<PathBuf>,
    #[cfg(target_os = "linux")]
    pub(crate) linux_helper: bool,
}

struct Streams<I, O, E> {
    input: I,
    output: O,
    error: E,
    interactive: bool,
}

/// Runs the CLI with process services and caller-provided streams. The first argument is argv[0].
pub fn run_system(
    args: impl IntoIterator<Item = OsString>,
    input: impl BufRead,
    output: impl Write,
    error: impl Write,
    interactive: bool,
) -> u8 {
    let args: Vec<_> = args.into_iter().collect();
    #[cfg(target_os = "linux")]
    if linux_worker_requested(&args) {
        return u8::from(crate::linux_owner::serve(input, output).is_err());
    }

    #[cfg(target_os = "linux")]
    let mut open_board = crate::linux_owner::connect;
    #[cfg(not(target_os = "linux"))]
    let mut open_board = copied_clipboard::system_board;
    let mut services = Services {
        open_board: &mut open_board,
        choose: &mut picker::system_choose,
        config_path: copied_config::default_config_path(),
        #[cfg(target_os = "linux")]
        linux_helper: true,
    };
    run(
        args,
        &mut Streams {
            input,
            output,
            error,
            interactive,
        },
        &mut services,
    )
}

#[cfg(target_os = "linux")]
fn linux_worker_requested(args: &[OsString]) -> bool {
    args.len() == 2 && args[1] == crate::linux_owner::WORKER_ARG
}

fn run(
    args: Vec<OsString>,
    streams: &mut Streams<impl BufRead, impl Write, impl Write>,
    services: &mut Services<'_>,
) -> u8 {
    let json = args.iter().skip(1).any(|arg| arg == "--json");
    let result = dispatch(args, streams, services).and_then(|()| {
        streams.output.flush()?;
        streams.error.flush()?;
        Ok(())
    });
    match result {
        Ok(()) => 0,
        Err(error) => {
            if json {
                #[derive(Serialize)]
                struct Envelope<'a> {
                    error: &'a Failure,
                }
                let _ = write_json(&mut streams.error, &Envelope { error: &error });
            } else {
                let _ = writeln!(streams.error, "{}", error.message.trim_end());
            }
            let _ = streams.error.flush();
            error.status
        }
    }
}

fn dispatch(
    mut args: Vec<OsString>,
    streams: &mut Streams<impl BufRead, impl Write, impl Write>,
    services: &mut Services<'_>,
) -> Result<(), Failure> {
    if args.first().is_some_and(|arg| pasteboard_alias(arg)) {
        args.insert(1, "clipboard".into());
    }
    let parsed = match Arguments::try_parse_from(args) {
        Ok(args) => args,
        Err(error) if error.kind() == ErrorKind::DisplayHelp => {
            return Ok(write!(streams.output, "{error}")?);
        }
        Err(error) => {
            // Node parseArgs reported unknown options and missing option values as runtime errors.
            let message = error.to_string();
            return Err(
                if error.kind() == ErrorKind::UnknownArgument
                    && error
                        .get(clap::error::ContextKind::InvalidArg)
                        .is_some_and(|arg| arg.to_string().starts_with('-'))
                    || error.kind() == ErrorKind::InvalidValue
                        && error
                            .get(clap::error::ContextKind::InvalidValue)
                            .is_some_and(|value| value.to_string().is_empty())
                {
                    Failure::system("command-failed", message)
                } else {
                    Failure::usage(message)
                },
            );
        }
    };
    match parsed.command.as_ref().unwrap_or(&Command::Convert) {
        Command::Targets => {
            if parsed.json {
                write_json(&mut streams.output, &TARGETS)?;
            } else {
                for target in TARGETS {
                    writeln!(streams.output, "{}\t{}", target.id, target.label)?;
                }
            }
        }
        Command::Convert | Command::Copy => convert(&parsed, streams, services)?,
        Command::Clipboard { action } => {
            if parsed.has_conversion_flags() {
                return Err(Failure::usage(
                    "The clipboard command takes no conversion options.",
                ));
            }
            pasteboard_compat::run(*action, &mut streams.input, &mut streams.output, services)?;
        }
        Command::Mcp => {
            if parsed.has_conversion_flags() || parsed.json {
                return Err(Failure::usage("The MCP command takes no options."));
            }
            mcp::serve(&mut streams.input, &mut streams.output)?;
        }
    }
    Ok(())
}

impl Arguments {
    fn has_conversion_flags(&self) -> bool {
        self.to.is_some()
            || self.file.is_some()
            || self.format.is_some()
            || self.source_url.is_some()
            || self.unicode
            || self.clipboard
            || self.pick
    }
}

fn pasteboard_alias(executable: &OsStr) -> bool {
    Path::new(executable)
        .file_name()
        .and_then(OsStr::to_str)
        .is_some_and(|name| matches!(name, "copied-md-pasteboard" | "copied-md-pasteboard.exe"))
}

fn convert(
    args: &Arguments,
    streams: &mut Streams<impl BufRead, impl Write, impl Write>,
    services: &mut Services<'_>,
) -> Result<(), Failure> {
    let copy = matches!(args.command, Some(Command::Copy));
    if !copy && (args.clipboard || args.pick) {
        return Err(Failure::usage(
            "--clipboard and --pick require the copy command.",
        ));
    }
    if args.clipboard && args.file.is_some() {
        return Err(Failure::usage("Choose either --file or --clipboard."));
    }
    if args.pick && args.to.is_some() {
        return Err(Failure::usage("Choose either --to or --pick."));
    }
    let mut target = target_id(args.to.as_deref().unwrap_or("rich-text"))?;
    let mut board = None;
    let (markdown, expected) = if args.clipboard {
        let clipboard = (services.open_board)()?;
        let snapshot = clipboard
            .read_text()
            .map_err(pasteboard_compat::read_error)?;
        board = Some(clipboard);
        (snapshot.text, Some(snapshot.revision))
    } else {
        (
            read_markdown(
                args.file.as_deref(),
                &mut streams.input,
                streams.interactive,
            )?,
            None,
        )
    };
    if args.pick {
        let Some(choice) = picker::pick(services)? else {
            return Ok(());
        };
        target = target_id(&choice)?;
    }
    let conversion = convert_markdown(
        &markdown,
        target,
        &ConversionOptions {
            source_url: args.source_url.clone(),
            decorative_unicode: args.unicode,
        },
    );

    if copy {
        if markdown.trim().is_empty() || conversion.representations.plain.trim().is_empty() {
            return Err(Failure::empty_output());
        }
        let mut board = match board {
            Some(board) => board,
            None => (services.open_board)()?,
        };
        let written = board.write(&conversion.representations, expected)?;
        pasteboard_compat::remember_target(services.config_path.as_deref(), target)?;
        if args.json {
            #[derive(Serialize)]
            struct Copied<'a> {
                #[serde(flatten)]
                conversion: &'a MarkdownConversion,
                clipboard: pasteboard_compat::Written,
            }
            return Ok(write_json(
                &mut streams.output,
                &Copied {
                    conversion: &conversion,
                    clipboard: written.into(),
                },
            )?);
        }
    } else if args.json {
        return Ok(write_json(&mut streams.output, &conversion)?);
    } else {
        let plain = args
            .format
            .unwrap_or(Format::Plain)
            .select(&conversion.representations)?;
        write!(streams.output, "{plain}")?;
    }
    write_diagnostics(&mut streams.error, &conversion.diagnostics)
}

fn read_markdown(
    file: Option<&Path>,
    mut input: impl BufRead,
    interactive: bool,
) -> Result<String, Failure> {
    let bytes = if let Some(path) = file {
        fs::read(path).map_err(|error| Failure::system("read-failed", error))?
    } else {
        if interactive {
            return Err(Failure::new(
                2,
                "missing-input",
                "Pipe Markdown into stdin or pass --file.",
            ));
        }
        let mut bytes = Vec::new();
        input
            .read_to_end(&mut bytes)
            .map_err(|error| Failure::system("read-failed", error))?;
        bytes
    };
    // Like Node's UTF-8 file and pipe decoding, replace malformed bytes.
    Ok(String::from_utf8_lossy_owned(bytes))
}

fn write_diagnostics(mut error: impl Write, diagnostics: &[Diagnostic]) -> Result<(), Failure> {
    for diagnostic in diagnostics {
        writeln!(error, "{}", diagnostic.message)?;
    }
    Ok(())
}

#[cfg(test)]
mod tests;
