use std::{
    collections::HashMap,
    env,
    fs::File,
    io::{ErrorKind, Read},
    os::{
        fd::{AsFd, OwnedFd},
        unix::net::UnixStream,
    },
    path::PathBuf,
    sync::Arc,
    time::{Duration, Instant},
};

use rustix::{
    event::{PollFd, PollFlags, Timespec, poll},
    fs::{OFlags, fcntl_getfl, fcntl_setfl},
    pipe::{PipeFlags, pipe_with},
};
use wayland_client::{
    Connection, Dispatch, EventQueue, Proxy, QueueHandle,
    backend::{ObjectId, WaylandError},
    protocol::{wl_callback, wl_registry, wl_seat},
};
use wayland_protocols::ext::data_control::v1::client::{
    ext_data_control_device_v1 as ext_device, ext_data_control_manager_v1 as ext_manager,
    ext_data_control_offer_v1 as ext_offer, ext_data_control_source_v1 as ext_source,
};
use wayland_protocols_wlr::data_control::v1::client::{
    zwlr_data_control_device_v1 as wlr_device, zwlr_data_control_manager_v1 as wlr_manager,
    zwlr_data_control_offer_v1 as wlr_offer, zwlr_data_control_source_v1 as wlr_source,
};

use super::{Formats, MAX_READ_BYTES, PLAIN, POLL_INTERVAL, Selection, unavailable};
use crate::{ClipboardSnapshot, Revision, WriteError, WrittenClipboard, require_revision};

const TIMEOUT: Duration = Duration::from_secs(2);
const MAX_PENDING_SENDS: usize = 32;

enum Device {
    Ext(
        ext_manager::ExtDataControlManagerV1,
        ext_device::ExtDataControlDeviceV1,
    ),
    Wlr(
        wlr_manager::ZwlrDataControlManagerV1,
        wlr_device::ZwlrDataControlDeviceV1,
    ),
}

enum Offer {
    Ext(ext_offer::ExtDataControlOfferV1),
    Wlr(wlr_offer::ZwlrDataControlOfferV1),
}

impl Offer {
    fn id(&self) -> ObjectId {
        match self {
            Self::Ext(offer) => offer.id(),
            Self::Wlr(offer) => offer.id(),
        }
    }

    fn receive(&self, mime: String, fd: &OwnedFd) {
        match self {
            Self::Ext(offer) => offer.receive(mime, fd.as_fd()),
            Self::Wlr(offer) => offer.receive(mime, fd.as_fd()),
        }
    }

    fn destroy(self) {
        match self {
            Self::Ext(offer) => offer.destroy(),
            Self::Wlr(offer) => offer.destroy(),
        }
    }
}

#[derive(Debug)]
struct SourceData {
    mime: Vec<(&'static str, Arc<[u8]>)>,
}

struct PendingSend {
    fd: OwnedFd,
    bytes: Arc<[u8]>,
    offset: usize,
    deadline: Instant,
}

#[derive(Default)]
struct State {
    globals: Vec<(u32, String)>,
    requested_sync: u64,
    completed_sync: u64,
    finished: bool,
    revision: u64,
    selection: Option<Offer>,
    formats: HashMap<ObjectId, Vec<String>>,
    source: Option<ObjectId>,
    outbound: Vec<PendingSend>,
}

impl State {
    fn selection(&mut self, offer: Option<Offer>) {
        if let Some(previous) = self.selection.take() {
            self.discard(previous);
        }
        self.selection = offer;
        self.revision += 1;
    }

    fn discard(&mut self, offer: Offer) {
        self.formats.remove(&offer.id());
        offer.destroy();
    }

    fn send_data(&mut self, data: &SourceData, mime: &str, fd: OwnedFd) {
        if self.outbound.len() < MAX_PENDING_SENDS
            && let Some((_, bytes)) = data.mime.iter().find(|(name, _)| *name == mime)
        {
            self.outbound.push(PendingSend {
                fd,
                bytes: bytes.clone(),
                offset: 0,
                deadline: Instant::now() + TIMEOUT,
            });
        }
    }

    fn send_pending(&mut self) {
        self.outbound.retain_mut(|send| {
            // Send in bounded batches to leave time for clipboard commands and events.
            for _ in 0..64 {
                if send.offset == send.bytes.len() || Instant::now() >= send.deadline {
                    return false;
                }
                let mut fd = [PollFd::new(&send.fd, PollFlags::OUT)];
                if poll(&mut fd, Some(&Timespec::default())).unwrap_or_default() == 0 {
                    return true;
                }
                if !fd[0].revents().contains(PollFlags::OUT) {
                    return false;
                }
                // Linux guarantees that a writable pipe accepts at least PIPE_BUF bytes.
                let end = send.bytes.len().min(send.offset + 4096);
                match rustix::io::write(&send.fd, &send.bytes[send.offset..end]) {
                    Ok(count) => {
                        send.offset += count;
                        send.deadline = Instant::now() + TIMEOUT;
                    }
                    Err(rustix::io::Errno::INTR | rustix::io::Errno::AGAIN) => return true,
                    Err(_) => return false,
                }
            }
            true
        });
    }
}

impl Dispatch<wl_registry::WlRegistry, ()> for State {
    fn event(
        state: &mut Self,
        _: &wl_registry::WlRegistry,
        event: wl_registry::Event,
        (): &(),
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
        match event {
            wl_registry::Event::Global {
                name, interface, ..
            } => state.globals.push((name, interface)),
            wl_registry::Event::GlobalRemove { name } => {
                state.globals.retain(|(id, _)| *id != name);
            }
            _ => {}
        }
    }
}

impl Dispatch<wl_callback::WlCallback, u64> for State {
    fn event(
        state: &mut Self,
        _: &wl_callback::WlCallback,
        _: wl_callback::Event,
        serial: &u64,
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
        state.completed_sync = *serial;
    }
}

wayland_client::delegate_noop!(State: ignore wl_seat::WlSeat);
wayland_client::delegate_noop!(State: ignore ext_manager::ExtDataControlManagerV1);
wayland_client::delegate_noop!(State: ignore wlr_manager::ZwlrDataControlManagerV1);

macro_rules! data_control_dispatch {
    ($variant:ident, $device:ident::$Device:ident, $offer:ident::$Offer:ident, $source:ident::$Source:ident) => {
        impl Dispatch<$device::$Device, ()> for State {
            fn event(state: &mut Self, device: &$device::$Device, event: $device::Event, (): &(), _: &Connection, _: &QueueHandle<Self>) {
                match event {
                    $device::Event::Selection { id } => state.selection(id.map(Offer::$variant)),
                    $device::Event::PrimarySelection { id: Some(offer) } => state.discard(Offer::$variant(offer)),
                    $device::Event::Finished => { state.finished = true; device.destroy(); }
                    _ => {}
                }
            }

            wayland_client::event_created_child!(State, $device::$Device, [
                $device::EVT_DATA_OFFER_OPCODE => ($offer::$Offer, ())
            ]);
        }

        impl Dispatch<$offer::$Offer, ()> for State {
            fn event(state: &mut Self, offer: &$offer::$Offer, event: $offer::Event, (): &(), _: &Connection, _: &QueueHandle<Self>) {
                if let $offer::Event::Offer { mime_type } = event {
                    state.formats.entry(offer.id()).or_default().push(mime_type);
                }
            }
        }

        impl Dispatch<$source::$Source, SourceData> for State {
            fn event(state: &mut Self, source: &$source::$Source, event: $source::Event, data: &SourceData, _: &Connection, _: &QueueHandle<Self>) {
                match event {
                    $source::Event::Send { mime_type, fd } => state.send_data(data, &mime_type, fd),
                    $source::Event::Cancelled => {
                        if state.source.as_ref() == Some(&source.id()) {
                            state.source = None;
                        }
                        source.destroy();
                    }
                    _ => {}
                }
            }
        }
    };
}

data_control_dispatch!(
    Ext,
    ext_device::ExtDataControlDeviceV1,
    ext_offer::ExtDataControlOfferV1,
    ext_source::ExtDataControlSourceV1
);
data_control_dispatch!(
    Wlr,
    wlr_device::ZwlrDataControlDeviceV1,
    wlr_offer::ZwlrDataControlOfferV1,
    wlr_source::ZwlrDataControlSourceV1
);

pub(super) struct Wayland {
    connection: Connection,
    queue: EventQueue<State>,
    state: State,
    device: Device,
}

impl Wayland {
    pub(super) fn connect() -> Result<Self, WriteError> {
        let display = env::var_os("WAYLAND_DISPLAY")
            .filter(|name| !name.is_empty())
            .map_or_else(|| PathBuf::from("wayland-0"), PathBuf::from);
        let path = if display.is_absolute() {
            display
        } else {
            env::var_os("XDG_RUNTIME_DIR")
                .map(PathBuf::from)
                .filter(|path| path.is_absolute())
                .ok_or_else(|| unavailable("Wayland has no runtime socket directory"))?
                .join(display)
        };
        let socket = UnixStream::connect(path).map_err(unavailable)?;
        let connection = Connection::from_socket(socket).map_err(unavailable)?;
        let mut queue = connection.new_event_queue();
        let mut state = State::default();
        let handle = queue.handle();
        let registry = connection.display().get_registry(&handle, ());
        barrier(&connection, &mut queue, &mut state)?;

        let global = |interface: &str| {
            state
                .globals
                .iter()
                .find(|(_, name)| name == interface)
                .map(|(id, _)| *id)
        };
        let seat = global("wl_seat").ok_or_else(|| unavailable("Wayland has no clipboard seat"))?;
        let seat = registry.bind::<wl_seat::WlSeat, _, _>(seat, 1, &handle, ());
        let device = if let Some(name) = global("ext_data_control_manager_v1") {
            let manager =
                registry.bind::<ext_manager::ExtDataControlManagerV1, _, _>(name, 1, &handle, ());
            let device = manager.get_data_device(&seat, &handle, ());
            Device::Ext(manager, device)
        } else if let Some(name) = global("zwlr_data_control_manager_v1") {
            let manager =
                registry.bind::<wlr_manager::ZwlrDataControlManagerV1, _, _>(name, 1, &handle, ());
            let device = manager.get_data_device(&seat, &handle, ());
            Device::Wlr(manager, device)
        } else {
            return Err(unavailable(
                "Wayland compositor does not expose a clipboard data-control protocol",
            ));
        };
        barrier(&connection, &mut queue, &mut state)?;
        if state.finished || state.revision == 0 {
            return Err(unavailable(
                "Wayland compositor did not grant background clipboard access",
            ));
        }
        Ok(Self {
            connection,
            queue,
            state,
            device,
        })
    }

    fn check_device(&self) -> Result<(), WriteError> {
        if self.state.finished {
            return Err(unavailable("Wayland compositor withdrew clipboard access"));
        }
        Ok(())
    }

    fn receive(&mut self, revision: Revision) -> Result<String, WriteError> {
        let offer = self.state.selection.as_ref().ok_or(WriteError::Empty)?;
        let types = self
            .state
            .formats
            .get(&offer.id())
            .ok_or(WriteError::Empty)?;
        let mime = [PLAIN, "text/plain", "UTF8_STRING"]
            .into_iter()
            .find(|mime| types.iter().any(|available| available == mime))
            .ok_or(WriteError::Empty)?;
        let (read, write) = pipe_with(PipeFlags::CLOEXEC).map_err(unavailable)?;
        let flags = fcntl_getfl(&read).map_err(unavailable)?;
        fcntl_setfl(&read, flags | OFlags::NONBLOCK).map_err(unavailable)?;
        offer.receive(mime.into(), &write);
        drop(write);
        let mut pipe = File::from(read);
        let mut bytes = Vec::new();
        let mut chunk = [0_u8; 8192];
        let deadline = Instant::now() + TIMEOUT;
        loop {
            self.poll()?;
            require_revision(Some(revision), Revision(self.state.revision))?;
            match pipe.read(&mut chunk) {
                Ok(0) => return String::from_utf8(bytes).map_err(unavailable),
                Ok(count) if bytes.len() + count <= MAX_READ_BYTES => {
                    bytes.extend_from_slice(&chunk[..count]);
                }
                Ok(_) => return Err(unavailable("Wayland clipboard text exceeds the read limit")),
                Err(error) if error.kind() == ErrorKind::WouldBlock => {
                    let mut input = [PollFd::new(&pipe, PollFlags::IN)];
                    wait(&mut input, POLL_INTERVAL)?;
                }
                Err(error) if error.kind() == ErrorKind::Interrupted => {}
                Err(error) => return Err(unavailable(error)),
            }
            if Instant::now() >= deadline {
                return Err(unavailable("Wayland clipboard owner did not respond"));
            }
        }
    }
}

fn wait(fds: &mut [PollFd<'_>], duration: Duration) -> Result<usize, WriteError> {
    let timeout = Timespec {
        tv_sec: i64::try_from(duration.as_secs()).unwrap_or(i64::MAX),
        tv_nsec: duration.subsec_nanos().into(),
    };
    match poll(fds, Some(&timeout)) {
        Ok(ready) => Ok(ready),
        Err(rustix::io::Errno::INTR) => Ok(0),
        Err(error) => Err(unavailable(error)),
    }
}

fn service(
    connection: &Connection,
    queue: &mut EventQueue<State>,
    state: &mut State,
    timeout: Duration,
) -> Result<(), WriteError> {
    queue.dispatch_pending(state).map_err(unavailable)?;
    state.send_pending();
    match connection.flush() {
        Ok(()) => {}
        Err(WaylandError::Io(error)) if error.kind() == ErrorKind::WouldBlock => {}
        Err(error) => return Err(unavailable(error)),
    }
    if let Some(guard) = queue.prepare_read() {
        let mut input = [PollFd::from_borrowed_fd(
            guard.connection_fd(),
            PollFlags::IN,
        )];
        if wait(&mut input, timeout)? != 0 {
            match guard.read() {
                Ok(_) => {}
                Err(WaylandError::Io(error)) if error.kind() == ErrorKind::WouldBlock => {}
                Err(error) => return Err(unavailable(error)),
            }
        }
    }
    queue.dispatch_pending(state).map_err(unavailable)?;
    Ok(())
}

fn barrier(
    connection: &Connection,
    queue: &mut EventQueue<State>,
    state: &mut State,
) -> Result<(), WriteError> {
    state.requested_sync += 1;
    connection
        .display()
        .sync(&queue.handle(), state.requested_sync);
    let deadline = Instant::now() + TIMEOUT;
    while state.completed_sync != state.requested_sync {
        if Instant::now() >= deadline {
            return Err(unavailable("Wayland clipboard sync timed out"));
        }
        service(connection, queue, state, POLL_INTERVAL)?;
    }
    Ok(())
}

impl Selection for Wayland {
    fn poll(&mut self) -> Result<(), WriteError> {
        service(
            &self.connection,
            &mut self.queue,
            &mut self.state,
            Duration::ZERO,
        )?;
        self.check_device()
    }

    fn revision(&mut self) -> Result<Revision, WriteError> {
        barrier(&self.connection, &mut self.queue, &mut self.state)?;
        self.check_device()?;
        Ok(Revision(self.state.revision))
    }

    fn read(&mut self) -> Result<ClipboardSnapshot, WriteError> {
        let revision = self.revision()?;
        let text = self.receive(revision);
        require_revision(Some(revision), self.revision()?)?;
        Ok(ClipboardSnapshot {
            text: text?,
            revision,
        })
    }

    fn write(
        &mut self,
        formats: Formats,
        expected: Option<Revision>,
    ) -> Result<WrittenClipboard, WriteError> {
        let revision = self.revision()?;
        require_revision(expected, revision)?;
        let handle = self.queue.handle();
        let Formats { mime, types, .. } = formats;
        let offer_names = mime.iter().map(|(name, _)| *name).collect::<Vec<_>>();
        let id = match &self.device {
            Device::Ext(manager, device) => {
                let source = manager.create_data_source(&handle, SourceData { mime });
                for name in offer_names {
                    source.offer(name.into());
                }
                device.set_selection(Some(&source));
                source.id()
            }
            Device::Wlr(manager, device) => {
                let source = manager.create_data_source(&handle, SourceData { mime });
                for name in offer_names {
                    source.offer(name.into());
                }
                device.set_selection(Some(&source));
                source.id()
            }
        };
        self.state.source = Some(id);
        // Data control has no server-side compare-and-swap. Observe before the request
        // and use the resulting selection event as the receipt.
        let updated = self.revision()?;
        if self.state.source.is_none() {
            return Err(WriteError::Stale);
        }
        if updated == revision {
            return Err(unavailable(
                "Wayland compositor did not acknowledge clipboard ownership",
            ));
        }
        Ok(WrittenClipboard {
            types,
            revision: updated,
        })
    }
}
