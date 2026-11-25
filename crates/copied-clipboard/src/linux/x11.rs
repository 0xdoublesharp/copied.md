use std::{
    sync::Arc,
    thread,
    time::{Duration, Instant},
};

use x11rb::{
    connection::{Connection, RequestConnection},
    protocol::{
        Event,
        xfixes::{ConnectionExt as _, SelectionEventMask},
        xproto::{
            self, Atom, AtomEnum, ChangeWindowAttributesAux, ConnectionExt as _, CreateWindowAux,
            EventMask, PropMode, Property, SelectionNotifyEvent, SelectionRequestEvent, Window,
            WindowClass,
        },
    },
    rust_connection::RustConnection,
    wrapper::ConnectionExt as _,
};

use super::{Formats, MAX_READ_BYTES, PLAIN, POLL_INTERVAL, ReadFormat, Selection, unavailable};
use crate::{Revision, WriteError, WrittenClipboard, require_revision};

const TRANSFER_TIMEOUT: Duration = Duration::from_secs(2);

x11rb::atom_manager! {
    Atoms: AtomCookies {
        CLIPBOARD,
        TARGETS,
        TIMESTAMP,
        INCR,
        UTF8_STRING,
        COPIED_SELECTION,
        HTML: b"text/html",
    }
}

struct OwnedFormats {
    plain: String,
    formats: Vec<(Atom, Arc<[u8]>)>,
}

struct Outgoing {
    requestor: Window,
    property: Atom,
    target: Atom,
    bytes: Arc<[u8]>,
    offset: usize,
    last_activity: Instant,
}

pub(super) struct X11 {
    connection: RustConnection,
    atoms: Atoms,
    root: Window,
    owner_window: Window,
    revision: Revision,
    owner: Window,
    selection_timestamp: u32,
    owned: Option<OwnedFormats>,
    outgoing: Vec<Outgoing>,
}

impl X11 {
    pub(super) fn connect() -> Result<Self, WriteError> {
        let (connection, screen) = x11rb::connect(None).map_err(unavailable)?;
        let root = connection.setup().roots[screen].root;
        connection
            .xfixes_query_version(2, 0)
            .map_err(unavailable)?
            .reply()
            .map_err(unavailable)?;
        let atoms = Atoms::new(&connection)
            .map_err(unavailable)?
            .reply()
            .map_err(unavailable)?;
        let owner_window = new_window(&connection, root)?;
        connection
            .xfixes_select_selection_input(
                owner_window,
                atoms.CLIPBOARD,
                SelectionEventMask::SET_SELECTION_OWNER
                    | SelectionEventMask::SELECTION_WINDOW_DESTROY
                    | SelectionEventMask::SELECTION_CLIENT_CLOSE,
            )
            .map_err(unavailable)?
            .check()
            .map_err(unavailable)?;
        let owner = connection
            .get_selection_owner(atoms.CLIPBOARD)
            .map_err(unavailable)?
            .reply()
            .map_err(unavailable)?
            .owner;
        Ok(Self {
            connection,
            atoms,
            root,
            owner_window,
            revision: Revision(0),
            owner,
            selection_timestamp: 0,
            owned: None,
            outgoing: Vec::new(),
        })
    }

    fn process(&mut self, event: &Event) {
        match event {
            Event::XfixesSelectionNotify(event) if event.selection == self.atoms.CLIPBOARD => {
                self.revision.0 += 1;
                self.owner = event.owner;
                self.selection_timestamp = event.selection_timestamp;
                if self.owner != self.owner_window {
                    self.owned = None;
                }
            }
            Event::SelectionRequest(event) => self.serve(*event),
            Event::PropertyNotify(event) if event.state == Property::DELETE => {
                self.advance_transfer(event.window, event.atom);
            }
            Event::DestroyNotify(event) => self
                .outgoing
                .retain(|transfer| transfer.requestor != event.window),
            _ => {}
        }
    }

    fn max_chunk(&self) -> usize {
        // Reserve request headers and pad to the X11 four-byte request alignment.
        self.connection
            .maximum_request_bytes()
            .saturating_sub(32)
            .min(256 * 1024)
            & !3
    }

    fn serve(&mut self, request: SelectionRequestEvent) {
        let property = if request.property == x11rb::NONE {
            request.target
        } else {
            request.property
        };
        let response =
            if request.selection == self.atoms.CLIPBOARD && request.owner == self.owner_window {
                self.set_request_property(request, property)
            } else {
                false
            };
        let event = SelectionNotifyEvent {
            response_type: xproto::SELECTION_NOTIFY_EVENT,
            sequence: 0,
            time: request.time,
            requestor: request.requestor,
            selection: request.selection,
            target: request.target,
            property: if response { property } else { x11rb::NONE },
        };
        if let Ok(cookie) =
            self.connection
                .send_event(false, request.requestor, EventMask::NO_EVENT, event)
        {
            cookie.ignore_error();
        }
        let _ = self.connection.flush();
    }

    fn set_request_property(&mut self, request: SelectionRequestEvent, property: Atom) -> bool {
        let Some(owned) = &self.owned else {
            return false;
        };
        let window = request.requestor;
        let connection = &self.connection;
        let result = if request.target == self.atoms.TARGETS {
            let mut targets = vec![self.atoms.TARGETS, self.atoms.TIMESTAMP];
            targets.extend(owned.formats.iter().map(|(atom, _)| *atom));
            connection.change_property32(
                PropMode::REPLACE,
                window,
                property,
                AtomEnum::ATOM,
                &targets,
            )
        } else if request.target == self.atoms.TIMESTAMP {
            connection.change_property32(
                PropMode::REPLACE,
                window,
                property,
                AtomEnum::INTEGER,
                &[self.selection_timestamp],
            )
        } else if let Some((_, bytes)) = owned
            .formats
            .iter()
            .find(|(atom, _)| *atom == request.target)
        {
            if bytes.len() > self.max_chunk() {
                let Ok(length) = u32::try_from(bytes.len()) else {
                    return false;
                };
                if self
                    .outgoing
                    .iter()
                    .any(|transfer| transfer.requestor == window && transfer.property == property)
                {
                    return false;
                }
                if !connection
                    .change_window_attributes(
                        window,
                        &ChangeWindowAttributesAux::new()
                            .event_mask(EventMask::PROPERTY_CHANGE | EventMask::STRUCTURE_NOTIFY),
                    )
                    .is_ok_and(|cookie| cookie.check().is_ok())
                {
                    return false;
                }
                let header = connection.change_property32(
                    PropMode::REPLACE,
                    window,
                    property,
                    self.atoms.INCR,
                    &[length],
                );
                if header.is_ok_and(|cookie| cookie.check().is_ok()) {
                    self.outgoing.push(Outgoing {
                        requestor: window,
                        property,
                        target: request.target,
                        bytes: bytes.clone(),
                        offset: 0,
                        last_activity: Instant::now(),
                    });
                    return true;
                }
                self.release_requestor(window);
                return false;
            }
            connection.change_property8(PropMode::REPLACE, window, property, request.target, bytes)
        } else {
            return false;
        };
        result.is_ok_and(|cookie| cookie.check().is_ok())
    }

    fn advance_transfer(&mut self, window: Window, property: Atom) {
        let Some(index) = self
            .outgoing
            .iter()
            .position(|transfer| transfer.requestor == window && transfer.property == property)
        else {
            return;
        };
        let max_chunk = self.max_chunk();
        let transfer = &mut self.outgoing[index];
        let end = transfer.bytes.len().min(transfer.offset + max_chunk);
        let chunk = &transfer.bytes[transfer.offset..end];
        let sent = self
            .connection
            .change_property8(PropMode::REPLACE, window, property, transfer.target, chunk)
            .is_ok_and(|cookie| cookie.check().is_ok());
        if chunk.is_empty() || !sent {
            self.outgoing.swap_remove(index);
            self.release_requestor(window);
        } else {
            transfer.offset = end;
            transfer.last_activity = Instant::now();
        }
    }

    fn release_requestor(&self, window: Window) {
        if !self
            .outgoing
            .iter()
            .any(|transfer| transfer.requestor == window)
            && let Ok(cookie) = self.connection.change_window_attributes(
                window,
                &ChangeWindowAttributesAux::new().event_mask(EventMask::NO_EVENT),
            )
        {
            cookie.ignore_error();
        }
    }

    fn barrier(&mut self) -> Result<(), WriteError> {
        self.connection
            .get_selection_owner(self.atoms.CLIPBOARD)
            .map_err(unavailable)?
            .reply()
            .map_err(unavailable)?;
        self.poll()
    }

    fn wait_for(
        &mut self,
        expected: Revision,
        deadline: Instant,
        predicate: impl Fn(&Event) -> bool,
    ) -> Result<Event, WriteError> {
        while Instant::now() < deadline {
            if let Some(event) = self.connection.poll_for_event().map_err(unavailable)? {
                if predicate(&event) {
                    return Ok(event);
                }
                self.process(&event);
                require_revision(Some(expected), self.revision)?;
            } else {
                thread::sleep(POLL_INTERVAL);
            }
        }
        Err(unavailable("X11 clipboard owner did not respond"))
    }

    fn read_target(
        &mut self,
        window: Window,
        target: Atom,
        revision: Revision,
        deadline: Instant,
    ) -> Result<Option<Vec<u8>>, WriteError> {
        let property = self.atoms.COPIED_SELECTION;
        let selection = self.atoms.CLIPBOARD;
        self.connection
            .convert_selection(window, selection, target, property, x11rb::CURRENT_TIME)
            .map_err(unavailable)?
            .check()
            .map_err(unavailable)?;
        let notify = self.wait_for(revision, deadline, |event| matches!(
            event, Event::SelectionNotify(event) if event.requestor == window && event.selection == selection && event.target == target
        ))?;
        if matches!(notify, Event::SelectionNotify(event) if event.property != property) {
            return Ok(None);
        }
        let mut reply = self.read_property(window, property)?;
        if reply.type_ != self.atoms.INCR {
            return Ok((reply.format == 8).then_some(reply.value));
        }
        let mut bytes = Vec::new();
        loop {
            self.wait_for(revision, deadline, |event| matches!(
                event, Event::PropertyNotify(event) if event.window == window && event.atom == property && event.state == Property::NEW_VALUE
            ))?;
            reply = self.read_property(window, property)?;
            if reply.format != 8
                || reply.type_ == x11rb::NONE
                || bytes.len() + reply.value.len() > MAX_READ_BYTES
            {
                return Err(unavailable("invalid or oversized X11 clipboard transfer"));
            }
            if reply.value.is_empty() {
                return Ok(Some(bytes));
            }
            bytes.extend(reply.value);
        }
    }

    fn read_property(
        &self,
        window: Window,
        property: Atom,
    ) -> Result<xproto::GetPropertyReply, WriteError> {
        let reply = self
            .connection
            .get_property(true, window, property, AtomEnum::ANY, 0, 16 * 1024 * 1024)
            .map_err(unavailable)?
            .reply()
            .map_err(unavailable)?;
        if reply.bytes_after != 0 {
            return Err(unavailable("X11 clipboard text exceeds the read limit"));
        }
        Ok(reply)
    }

    fn read_external(
        &mut self,
        targets: &[Atom],
        revision: Revision,
    ) -> Result<Option<String>, WriteError> {
        let window = new_window(&self.connection, self.root)?;
        let deadline = Instant::now() + TRANSFER_TIMEOUT;
        let result = (|| {
            for &target in targets {
                if let Some(bytes) = self.read_target(window, target, revision, deadline)? {
                    return if target == u32::from(AtomEnum::STRING) {
                        Ok(Some(bytes.into_iter().map(char::from).collect()))
                    } else {
                        String::from_utf8(bytes).map(Some).map_err(unavailable)
                    };
                }
            }
            Ok(None)
        })();
        if let Ok(cookie) = self.connection.destroy_window(window) {
            cookie.ignore_error();
        }
        result
    }

    fn intern(&self, name: &str) -> Result<Atom, WriteError> {
        self.connection
            .intern_atom(false, name.as_bytes())
            .map_err(unavailable)?
            .reply()
            .map(|reply| reply.atom)
            .map_err(unavailable)
    }

    fn claim(
        &mut self,
        owned: OwnedFormats,
        expected: Option<Revision>,
    ) -> Result<Revision, WriteError> {
        self.barrier()?;
        require_revision(expected, self.revision)?;
        let previous = self.revision;
        self.connection
            .set_selection_owner(self.owner_window, self.atoms.CLIPBOARD, x11rb::CURRENT_TIME)
            .map_err(unavailable)?
            .check()
            .map_err(unavailable)?;
        self.owned = Some(owned);
        self.barrier()?;
        if self.owner != self.owner_window || self.revision == previous {
            return Err(WriteError::Rejected(
                "X11 rejected clipboard ownership".into(),
            ));
        }
        Ok(self.revision)
    }
}

fn new_window(connection: &RustConnection, root: Window) -> Result<Window, WriteError> {
    let window = connection.generate_id().map_err(unavailable)?;
    connection
        .create_window(
            x11rb::COPY_DEPTH_FROM_PARENT,
            window,
            root,
            0,
            0,
            1,
            1,
            0,
            WindowClass::INPUT_OUTPUT,
            x11rb::COPY_FROM_PARENT,
            &CreateWindowAux::new().event_mask(EventMask::PROPERTY_CHANGE),
        )
        .map_err(unavailable)?
        .check()
        .map_err(unavailable)?;
    Ok(window)
}

impl Selection for X11 {
    fn poll(&mut self) -> Result<(), WriteError> {
        while let Some(event) = self.connection.poll_for_event().map_err(unavailable)? {
            self.process(&event);
        }
        let mut expired = Vec::new();
        self.outgoing.retain(|transfer| {
            if transfer.last_activity.elapsed() >= TRANSFER_TIMEOUT {
                expired.push(transfer.requestor);
                false
            } else {
                true
            }
        });
        for window in expired {
            self.release_requestor(window);
        }
        self.connection.flush().map_err(unavailable)
    }

    fn revision(&mut self) -> Result<Revision, WriteError> {
        self.barrier()?;
        Ok(self.revision)
    }

    fn read_format(
        &mut self,
        format: ReadFormat,
        revision: Revision,
    ) -> Result<Option<String>, WriteError> {
        if let Some(data) = &self.owned {
            return match format {
                ReadFormat::Plain => Ok(Some(data.plain.clone())),
                ReadFormat::Html => data
                    .formats
                    .iter()
                    .find(|(atom, _)| *atom == self.atoms.HTML)
                    .map(|(_, bytes)| String::from_utf8(bytes.to_vec()).map_err(unavailable))
                    .transpose(),
            };
        }
        if self.owner == x11rb::NONE {
            return Ok(None);
        }
        match format {
            ReadFormat::Plain => self.read_external(
                &[
                    self.atoms.UTF8_STRING,
                    self.intern(PLAIN)?,
                    self.intern("text/plain")?,
                    u32::from(AtomEnum::STRING),
                ],
                revision,
            ),
            ReadFormat::Html => self.read_external(&[self.atoms.HTML], revision),
        }
    }

    fn write(
        &mut self,
        formats: Formats,
        expected: Option<Revision>,
    ) -> Result<WrittenClipboard, WriteError> {
        let Formats { plain, mime, types } = formats;
        let mut native = mime
            .into_iter()
            .map(|(name, data)| Ok((self.intern(name)?, data)))
            .collect::<Result<Vec<_>, WriteError>>()?;
        native.push((self.atoms.UTF8_STRING, Arc::from(plain.as_bytes())));
        if let Some(latin1) = plain
            .chars()
            .map(|character| u8::try_from(u32::from(character)).ok())
            .collect::<Option<Vec<_>>>()
        {
            native.push((u32::from(AtomEnum::STRING), Arc::from(latin1)));
        }
        // Serialize the final owner/revision check and acquisition with other X11 clients.
        self.connection
            .grab_server()
            .map_err(unavailable)?
            .check()
            .map_err(unavailable)?;
        let result = self.claim(
            OwnedFormats {
                plain,
                formats: native,
            },
            expected,
        );
        self.connection
            .ungrab_server()
            .map_err(unavailable)?
            .check()
            .map_err(unavailable)?;
        Ok(WrittenClipboard {
            types,
            revision: result?,
        })
    }
}
