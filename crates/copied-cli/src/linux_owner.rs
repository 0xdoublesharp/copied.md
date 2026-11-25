//! Keep the Linux selection owner available after a successful one-shot command.

use copied_clipboard::{Board, ClipboardSnapshot, Revision, WriteError, WrittenClipboard};
use copied_core::ClipboardPayload;
use serde::{Deserialize, Serialize, de::DeserializeOwned};
use std::{
    cell::RefCell,
    fs::File,
    io::{self, BufRead, BufReader, BufWriter, Read, Write},
    os::{
        linux::net::SocketAddrExt as _,
        unix::{
            net::{SocketAddr, UnixListener, UnixStream},
            process::CommandExt as _,
        },
    },
    process::{Child, ChildStdin, ChildStdout, Command, Stdio},
    thread,
    time::{Duration, Instant},
};

use crate::cli::write_json;

pub(crate) const WORKER_ARG: &str = "--internal-linux-clipboard-owner";

#[derive(Serialize, Deserialize)]
enum Request {
    Revision,
    Read,
    ReadGuarded,
    Write(ClipboardPayload, Option<Revision>),
}

#[derive(Serialize, Deserialize)]
enum RemoteError {
    Stale,
    Empty,
    Rejected(String),
    Unavailable(String),
}

impl From<WriteError> for RemoteError {
    fn from(error: WriteError) -> Self {
        match error {
            WriteError::Stale => Self::Stale,
            WriteError::Empty => Self::Empty,
            WriteError::Rejected(message) => Self::Rejected(message),
            WriteError::Unavailable(message) => Self::Unavailable(message),
        }
    }
}

impl From<RemoteError> for WriteError {
    fn from(error: RemoteError) -> Self {
        match error {
            RemoteError::Stale => Self::Stale,
            RemoteError::Empty => Self::Empty,
            RemoteError::Rejected(message) => Self::Rejected(message),
            RemoteError::Unavailable(message) => Self::Unavailable(message),
        }
    }
}

struct Pipes {
    input: BufReader<ChildStdout>,
    output: BufWriter<ChildStdin>,
}

struct Owner {
    child: Option<Child>,
    pipes: Option<RefCell<Pipes>>,
    retained: bool,
}

pub(crate) fn connect() -> Result<Box<dyn Board>, WriteError> {
    Ok(Box::new(connect_worker()?))
}

fn connect_worker() -> Result<Owner, WriteError> {
    let mut child = Command::new(std::env::current_exe().map_err(unavailable)?)
        .arg(WORKER_ARG)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .process_group(0)
        .spawn()
        .map_err(unavailable)?;
    let mut input = BufReader::new(child.stdout.take().expect("worker stdout is piped"));
    let output = BufWriter::new(child.stdin.take().expect("worker stdin is piped"));
    let ready = receive::<()>(&mut input);
    let owner = Owner {
        child: Some(child),
        pipes: Some(RefCell::new(Pipes { input, output })),
        retained: false,
    };
    ready?;
    Ok(owner)
}

impl Owner {
    fn send<T: DeserializeOwned>(&self, request: &Request) -> Result<T, WriteError> {
        let mut pipes = self
            .pipes
            .as_ref()
            .expect("worker connection is open")
            .borrow_mut();
        write_json(&mut pipes.output, request).map_err(unavailable)?;
        pipes.output.flush().map_err(unavailable)?;
        receive(&mut pipes.input)
    }
}

impl Drop for Owner {
    fn drop(&mut self) {
        // EOF tells the worker to retain a successful selection until it is replaced.
        drop(self.pipes.take());
        if let Some(mut child) = self.child.take() {
            if self.retained {
                let _ = thread::Builder::new()
                    .name("copied-clipboard-reaper".into())
                    .spawn(move || {
                        let _ = child.wait();
                    });
            } else {
                let _ = child.wait();
            }
        }
    }
}

impl Board for Owner {
    fn revision(&self) -> Result<Revision, WriteError> {
        self.send(&Request::Revision)
    }

    fn read_text(&self) -> Result<ClipboardSnapshot, WriteError> {
        self.send(&Request::Read)
    }

    fn write(
        &mut self,
        payload: &ClipboardPayload,
        expected: Option<Revision>,
    ) -> Result<WrittenClipboard, WriteError> {
        let written = self.send(&Request::Write(payload.clone(), expected))?;
        self.retained = true;
        Ok(written)
    }
}

fn receive<T: DeserializeOwned>(input: &mut impl BufRead) -> Result<T, WriteError> {
    let mut line = Vec::new();
    input.read_until(b'\n', &mut line).map_err(unavailable)?;
    serde_json::from_slice::<Result<T, RemoteError>>(&line)
        .map_err(unavailable)?
        .map_err(Into::into)
}

fn respond<T: Serialize>(
    output: &mut impl Write,
    response: Result<T, WriteError>,
) -> io::Result<()> {
    write_json(output, &response.map_err(RemoteError::from))?;
    output.flush()
}

const POLL_INTERVAL: Duration = Duration::from_millis(100);
const GUARD_TIMEOUT: Duration = Duration::from_mins(5);
const IO_TIMEOUT: Duration = Duration::from_secs(5);

/// A helper read must keep the original Board connection alive. Its changeCount
/// is a single-use handle to that connection, because Linux revisions are local
/// to the selection listener and can restart at the same number in each process.
pub(crate) fn read_guarded() -> Result<ClipboardSnapshot, WriteError> {
    let mut worker = connect_worker()?;
    let snapshot = worker.send(&Request::ReadGuarded)?;
    worker.retained = true;
    Ok(snapshot)
}

pub(crate) fn write_guarded(
    payload: &ClipboardPayload,
    token: Revision,
) -> Result<WrittenClipboard, WriteError> {
    let address = guard_address(token).map_err(unavailable)?;
    let mut connection = UnixStream::connect_addr(&address).map_err(guarded_io_error)?;
    if !same_user(&connection) {
        return Err(WriteError::Stale);
    }
    connection
        .set_read_timeout(Some(IO_TIMEOUT))
        .map_err(unavailable)?;
    connection
        .set_write_timeout(Some(IO_TIMEOUT))
        .map_err(unavailable)?;
    {
        let mut output = BufWriter::new(&mut connection);
        write_json(&mut output, payload).map_err(guarded_io_error)?;
        output.flush().map_err(guarded_io_error)?;
    }
    connection
        .shutdown(std::net::Shutdown::Write)
        .map_err(guarded_io_error)?;
    let mut response = Vec::new();
    if BufReader::new(connection)
        .read_until(b'\n', &mut response)
        .map_err(guarded_io_error)?
        == 0
    {
        return Err(WriteError::Stale);
    }
    serde_json::from_slice::<Result<WrittenClipboard, RemoteError>>(&response)
        .map_err(unavailable)?
        .map_err(Into::into)
}

fn guarded_io_error(error: io::Error) -> WriteError {
    match error.kind() {
        io::ErrorKind::ConnectionRefused
        | io::ErrorKind::ConnectionReset
        | io::ErrorKind::BrokenPipe
        | io::ErrorKind::NotConnected => WriteError::Stale,
        _ => unavailable(error),
    }
}

fn guard_address(token: Revision) -> io::Result<SocketAddr> {
    SocketAddr::from_abstract_name(format!("copied-md-clipboard-{:x}", token.0))
}

fn same_user(connection: &UnixStream) -> bool {
    rustix::net::sockopt::socket_peercred(connection)
        .is_ok_and(|peer| peer.uid == rustix::process::geteuid())
}

enum Retained {
    Selection(Revision),
    Guard(UnixListener, Revision),
}

fn make_guard(board: &dyn Board) -> Result<(ClipboardSnapshot, Retained), WriteError> {
    let snapshot = board.read_text()?;
    let mut random = [0; 8];
    File::open("/dev/urandom")
        .and_then(|mut file| file.read_exact(&mut random))
        .map_err(unavailable)?;
    // Preserve exact JS-number round trips through existing helper clients.
    let token = Revision(u64::from_ne_bytes(random) & ((1 << 53) - 1));
    let listener = UnixListener::bind_addr(&guard_address(token).map_err(unavailable)?)
        .map_err(unavailable)?;
    listener.set_nonblocking(true).map_err(unavailable)?;
    let guard = Retained::Guard(listener, snapshot.revision);
    Ok((
        ClipboardSnapshot {
            revision: token,
            ..snapshot
        },
        guard,
    ))
}

pub(crate) fn serve(mut input: impl BufRead, mut output: impl Write) -> io::Result<()> {
    let mut board = match copied_clipboard::system_board() {
        Ok(board) => board,
        Err(error) => return respond::<()>(&mut output, Err(error)),
    };
    respond(&mut output, Ok(()))?;
    let retained = serve_requests(&mut input, &mut output, &mut *board)?;
    let owned = match retained {
        Some(Retained::Guard(listener, revision)) => {
            wait_for_guard(&listener, revision, &mut *board)?
        }
        Some(Retained::Selection(revision)) => Some(revision),
        None => None,
    };
    if let Some(revision) = owned {
        while board.revision().ok() == Some(revision) {
            thread::sleep(POLL_INTERVAL);
        }
    }
    Ok(())
}

fn wait_for_guard(
    listener: &UnixListener,
    revision: Revision,
    board: &mut dyn Board,
) -> io::Result<Option<Revision>> {
    let deadline = Instant::now() + GUARD_TIMEOUT;
    while Instant::now() < deadline && board.revision().ok() == Some(revision) {
        match listener.accept() {
            Ok((mut connection, _)) if same_user(&connection) => {
                connection.set_read_timeout(Some(IO_TIMEOUT))?;
                connection.set_write_timeout(Some(IO_TIMEOUT))?;
                let payload = serde_json::from_reader(BufReader::new(&connection));
                if let Ok(payload) = payload {
                    let result = board.write(&payload, Some(revision));
                    let written = result.as_ref().ok().map(|written| written.revision);
                    // Keep ownership even when the requester disconnects before reading.
                    let _ = respond(&mut BufWriter::new(&mut connection), result);
                    if written.is_some() {
                        return Ok(written);
                    }
                }
            }
            Ok(_) => {}
            Err(error) if error.kind() == io::ErrorKind::WouldBlock => thread::sleep(POLL_INTERVAL),
            Err(error) => return Err(error),
        }
    }
    Ok(None)
}

fn serve_requests(
    input: &mut impl BufRead,
    output: &mut impl Write,
    board: &mut dyn Board,
) -> io::Result<Option<Retained>> {
    let mut retained = None;
    let mut line = Vec::new();
    while input.read_until(b'\n', &mut line)? != 0 {
        match serde_json::from_slice(&line)? {
            Request::Revision => respond(output, board.revision())?,
            Request::Read => respond(output, board.read_text())?,
            Request::ReadGuarded => {
                let result = make_guard(board).map(|(snapshot, guard)| {
                    retained = Some(guard);
                    snapshot
                });
                respond(output, result)?;
            }
            Request::Write(payload, expected) => {
                let result = board.write(&payload, expected);
                if let Ok(written) = &result {
                    retained = Some(Retained::Selection(written.revision));
                }
                // A successful write must survive even when the client closes its pipe.
                if let Err(error) = respond(output, result) {
                    return if retained.is_some() {
                        Ok(retained)
                    } else {
                        Err(error)
                    };
                }
            }
        }
        line.clear();
    }
    Ok(retained)
}

fn unavailable(error: impl std::fmt::Display) -> WriteError {
    WriteError::Unavailable(error.to_string())
}
