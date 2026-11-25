use std::{
    env,
    ffi::OsStr,
    sync::{
        Arc,
        mpsc::{self, RecvTimeoutError, Sender},
    },
    thread,
    time::Duration,
};

use copied_core::ClipboardPayload;

use crate::{
    Board, ClipboardSnapshot, Revision, WriteError, WrittenClipboard, bundle::prepare_bundle,
    require_revision,
};

mod wayland;
mod x11;

const POLL_INTERVAL: Duration = Duration::from_millis(10);
const REQUEST_TIMEOUT: Duration = Duration::from_secs(4);
const MAX_READ_BYTES: usize = 64 * 1024 * 1024;
const PLAIN: &str = "text/plain;charset=utf-8";
const HTML: &str = "text/html";
const SIGNAL: &str = "chromium/x-web-custom-data";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Session {
    Wayland,
    X11,
    Headless,
}

pub fn desktop_session() -> Session {
    session_from(
        env::var_os("WAYLAND_DISPLAY").as_deref(),
        env::var_os("WAYLAND_SOCKET").as_deref(),
        env::var_os("XDG_SESSION_TYPE").as_deref(),
        env::var_os("DISPLAY").as_deref(),
    )
}

fn session_from(
    wayland: Option<&OsStr>,
    socket: Option<&OsStr>,
    kind: Option<&OsStr>,
    x11: Option<&OsStr>,
) -> Session {
    if wayland.is_some_and(|value| !value.is_empty())
        || socket.is_some_and(|value| !value.is_empty())
        || kind == Some(OsStr::new("wayland"))
    {
        Session::Wayland
    } else if x11.is_some_and(|value| !value.is_empty()) {
        Session::X11
    } else {
        Session::Headless
    }
}

pub fn background_clipboard_available() -> bool {
    LinuxClipboard::connect().is_ok()
}

#[derive(Debug)]
struct Formats {
    plain: String,
    mime: Vec<(&'static str, Arc<[u8]>)>,
    types: Vec<String>,
}

impl Formats {
    fn new(payload: &ClipboardPayload) -> Result<Self, WriteError> {
        let bundle = prepare_bundle(payload)?;
        let types = bundle.types();
        let plain: Arc<[u8]> = Arc::from(bundle.plain.as_bytes());
        let mut mime = vec![(PLAIN, plain.clone()), ("text/plain", plain)];
        for (name, bytes) in [(HTML, bundle.html), ("text/rtf", bundle.rtf)] {
            if let Some(bytes) = bytes {
                mime.push((name, Arc::from(bytes.as_bytes())));
            }
        }
        if let Some(pickle) = bundle.signal_pickle {
            mime.push((SIGNAL, Arc::from(pickle)));
        }
        Ok(Self {
            plain: bundle.plain.into(),
            mime,
            types,
        })
    }
}

enum Command {
    Revision(Sender<Result<Revision, WriteError>>),
    Read(
        ReadFormat,
        Sender<Result<Option<ClipboardSnapshot>, WriteError>>,
    ),
    Write(
        Formats,
        Option<Revision>,
        Sender<Result<WrittenClipboard, WriteError>>,
    ),
}

#[derive(Clone, Copy)]
enum ReadFormat {
    Plain,
    Html,
}

trait Selection {
    fn poll(&mut self) -> Result<(), WriteError>;
    fn revision(&mut self) -> Result<Revision, WriteError>;
    fn read_format(
        &mut self,
        format: ReadFormat,
        revision: Revision,
    ) -> Result<Option<String>, WriteError>;

    fn read(&mut self, format: ReadFormat) -> Result<Option<ClipboardSnapshot>, WriteError> {
        let revision = self.revision()?;
        let text = self.read_format(format, revision);
        require_revision(Some(revision), self.revision()?)?;
        Ok(text?.map(|text| ClipboardSnapshot { text, revision }))
    }

    fn write(
        &mut self,
        formats: Formats,
        expected: Option<Revision>,
    ) -> Result<WrittenClipboard, WriteError>;
}

/// Owns a clipboard connection and serves selection requests on a background worker.
/// Keep the board alive for as long as its published formats should be pasteable.
#[derive(Debug)]
pub struct LinuxClipboard {
    commands: Sender<Command>,
}

impl LinuxClipboard {
    pub fn connect() -> Result<Self, WriteError> {
        let session = desktop_session();
        if session == Session::Headless {
            return Err(unavailable("no Wayland or X11 display"));
        }

        let (commands, requests) = mpsc::channel();
        let (ready, response) = mpsc::channel();
        thread::Builder::new()
            .name("copied-linux-clipboard".into())
            .spawn(move || {
                let backend: Result<Box<dyn Selection>, _> = match session {
                    Session::Wayland => wayland::Wayland::connect()
                        .map(|board| Box::new(board) as Box<dyn Selection>),
                    Session::X11 => {
                        x11::X11::connect().map(|board| Box::new(board) as Box<dyn Selection>)
                    }
                    Session::Headless => unreachable!(),
                };
                let mut board = match backend {
                    Ok(board) if ready.send(Ok(())).is_ok() => board,
                    Ok(_) => return,
                    Err(error) => {
                        let _ = ready.send(Err(error));
                        return;
                    }
                };
                loop {
                    if board.poll().is_err() {
                        break;
                    }
                    match requests.recv_timeout(POLL_INTERVAL) {
                        Ok(Command::Revision(reply)) => {
                            let _ = reply.send(board.revision());
                        }
                        Ok(Command::Read(format, reply)) => {
                            let _ = reply.send(board.read(format));
                        }
                        Ok(Command::Write(formats, expected, reply)) => {
                            let _ = reply.send(board.write(formats, expected));
                        }
                        Err(RecvTimeoutError::Timeout) => {}
                        Err(RecvTimeoutError::Disconnected) => break,
                    }
                }
            })
            .map_err(unavailable)?;
        response
            .recv_timeout(REQUEST_TIMEOUT)
            .map_err(unavailable)??;
        Ok(Self { commands })
    }

    fn request<T>(
        &self,
        command: impl FnOnce(Sender<Result<T, WriteError>>) -> Command,
    ) -> Result<T, WriteError> {
        let (reply, response) = mpsc::channel();
        self.commands.send(command(reply)).map_err(unavailable)?;
        response.recv().map_err(unavailable)?
    }
}

impl Board for LinuxClipboard {
    fn revision(&self) -> Result<Revision, WriteError> {
        self.request(Command::Revision)
    }

    fn read_text(&self) -> Result<ClipboardSnapshot, WriteError> {
        self.request(|reply| Command::Read(ReadFormat::Plain, reply))?
            .ok_or(WriteError::Empty)
    }

    fn read_html(&self) -> Result<Option<ClipboardSnapshot>, WriteError> {
        self.request(|reply| Command::Read(ReadFormat::Html, reply))
    }

    fn write(
        &mut self,
        payload: &ClipboardPayload,
        expected: Option<Revision>,
    ) -> Result<WrittenClipboard, WriteError> {
        let formats = Formats::new(payload)?;
        self.request(|reply| Command::Write(formats, expected, reply))
    }
}

fn unavailable(error: impl std::fmt::Display) -> WriteError {
    WriteError::Unavailable(error.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn clipboard_session_prefers_wayland_to_xwayland() {
        assert_eq!(
            session_from(
                Some(OsStr::new("wayland-0")),
                None,
                None,
                Some(OsStr::new(":0"))
            ),
            Session::Wayland
        );
        assert_eq!(
            session_from(
                None,
                None,
                Some(OsStr::new("wayland")),
                Some(OsStr::new(":0"))
            ),
            Session::Wayland
        );
        assert_eq!(
            session_from(Some(OsStr::new("")), None, None, Some(OsStr::new(":0"))),
            Session::X11
        );
        assert_eq!(session_from(None, None, None, None), Session::Headless);
    }

    #[test]
    fn linux_formats_include_chromium_custom_data() {
        let payload = ClipboardPayload {
            plain: "example".into(),
            html: Some("<p>Example</p>".into()),
            rtf: Some(String::new()),
            signal: Some("<p>Example</p>".into()),
        };
        let formats = Formats::new(&payload).unwrap();
        let names = formats
            .mime
            .iter()
            .map(|(name, _)| *name)
            .collect::<Vec<_>>();
        assert_eq!(names, [PLAIN, "text/plain", "text/html", SIGNAL]);
        assert_eq!(formats.types, ["text/plain", "text/html", "text/signal"]);
    }
}
