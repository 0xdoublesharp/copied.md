use copied_core::ClipboardPayload;
use objc2::{
    rc::{Retained, autoreleasepool},
    runtime::ProtocolObject,
};
use objc2_app_kit::{
    NSPasteboard, NSPasteboardItem, NSPasteboardTypeHTML, NSPasteboardTypeRTF,
    NSPasteboardTypeString, NSPasteboardWriting,
};
use objc2_foundation::{NSArray, NSData, NSString};

use crate::{
    Board, ClipboardSnapshot, Revision, WriteError, WrittenClipboard,
    bundle::{Bundle, prepare_bundle},
    require_revision, stable_snapshot,
};

const CHROMIUM_CUSTOM_DATA: &str = "org.chromium.web-custom-data";

/// The general pasteboard or an owned, temporary named pasteboard.
#[derive(Debug)]
pub struct MacPasteboard {
    // Keep the name instead of sending an Objective-C object across threads.
    unique_name: Option<String>,
}

impl MacPasteboard {
    pub fn general() -> Self {
        Self { unique_name: None }
    }

    pub fn unique() -> Self {
        autoreleasepool(|_| Self {
            unique_name: Some(NSPasteboard::pasteboardWithUniqueName().name().to_string()),
        })
    }

    /// Reads a native representation from this pasteboard.
    pub fn data_for_type(&self, flavor: &str) -> Option<Vec<u8>> {
        autoreleasepool(|_| {
            self.pasteboard()
                .dataForType(&NSString::from_str(flavor))
                .map(|data| data.to_vec())
        })
    }

    fn pasteboard(&self) -> Retained<NSPasteboard> {
        self.unique_name
            .as_deref()
            .map_or_else(NSPasteboard::generalPasteboard, |name| {
                NSPasteboard::pasteboardWithName(&NSString::from_str(name))
            })
    }
}

impl Drop for MacPasteboard {
    #[allow(unsafe_code)]
    fn drop(&mut self) {
        if self.unique_name.is_some() {
            autoreleasepool(|_| {
                let board = self.pasteboard();
                // SAFETY: releaseGlobally is a void method and this is an owned unique pasteboard.
                let (): () = unsafe { objc2::msg_send![&*board, releaseGlobally] };
            });
        }
    }
}

#[allow(unsafe_code)]
fn prepare_item(bundle: &Bundle<'_>) -> Result<Retained<NSPasteboardItem>, WriteError> {
    // SAFETY: AppKit supplies these immutable pasteboard type strings.
    let (plain, html, rtf) = unsafe {
        (
            NSPasteboardTypeString,
            NSPasteboardTypeHTML,
            NSPasteboardTypeRTF,
        )
    };
    let item = NSPasteboardItem::new();
    if !item.setString_forType(&NSString::from_str(bundle.plain), plain)
        || bundle
            .html
            .is_some_and(|value| !item.setString_forType(&NSString::from_str(value), html))
        || bundle
            .rtf
            .is_some_and(|value| !item.setData_forType(&NSData::with_bytes(value.as_bytes()), rtf))
        || bundle.signal_pickle.as_deref().is_some_and(|data| {
            !item.setData_forType(
                &NSData::with_bytes(data),
                &NSString::from_str(CHROMIUM_CUSTOM_DATA),
            )
        })
    {
        return Err(WriteError::Rejected(
            "macOS could not prepare the clipboard flavors".into(),
        ));
    }
    Ok(item)
}

fn revision(count: isize) -> Result<Revision, WriteError> {
    u64::try_from(count)
        .map(Revision)
        .map_err(|_| WriteError::Unavailable("invalid macOS pasteboard change count".into()))
}

impl Board for MacPasteboard {
    fn revision(&self) -> Result<Revision, WriteError> {
        autoreleasepool(|_| revision(self.pasteboard().changeCount()))
    }

    #[allow(unsafe_code)]
    fn read_text(&self) -> Result<ClipboardSnapshot, WriteError> {
        autoreleasepool(|_| {
            let board = self.pasteboard();
            stable_snapshot(
                || revision(board.changeCount()),
                || {
                    // SAFETY: AppKit supplies this immutable pasteboard type string.
                    let plain = unsafe { NSPasteboardTypeString };
                    Ok(board.stringForType(plain).map(|text| text.to_string()))
                },
            )
        })
    }

    fn write(
        &mut self,
        payload: &ClipboardPayload,
        expected: Option<Revision>,
    ) -> Result<WrittenClipboard, WriteError> {
        let bundle = prepare_bundle(payload)?;
        let types = bundle.types();
        autoreleasepool(|_| {
            let item = prepare_item(&bundle)?;
            let item = ProtocolObject::<dyn NSPasteboardWriting>::from_retained(item);
            let items = NSArray::from_retained_slice(&[item]);
            let board = self.pasteboard();
            if expected.is_some() {
                require_revision(expected, revision(board.changeCount())?)?;
            }
            let prepared = revision(board.clearContents())?;
            require_revision(Some(prepared), revision(board.changeCount())?)?;
            if !board.writeObjects(&items) {
                return Err(WriteError::Rejected(
                    "macOS rejected the clipboard write".into(),
                ));
            }
            Ok(WrittenClipboard {
                types,
                revision: prepared,
            })
        })
    }
}

/// Exercises real pasteboard writes on a temporary board.
pub fn self_test() -> Result<WrittenClipboard, WriteError> {
    check_pasteboard(&mut MacPasteboard::unique(), |board| {
        board.data_for_type("public.html")
    })
}

fn check_pasteboard<B: Board>(
    board: &mut B,
    html: impl FnOnce(&B) -> Option<Vec<u8>>,
) -> Result<WrittenClipboard, WriteError> {
    const HTML: &str = "<h1>Example</h1>";
    let initial = board.revision()?;
    let original = ClipboardPayload {
        plain: "# Example".into(),
        html: Some(HTML.into()),
        ..ClipboardPayload::default()
    };
    let written = board.write(&original, Some(initial))?;
    let captured = board.read_text()?;
    if written.types != ["text/plain", "text/html"]
        || captured.text != original.plain
        || captured.revision != written.revision
        || html(board).as_deref() != Some(HTML.as_bytes())
    {
        return Err(WriteError::Rejected(
            "pasteboard types did not round-trip".into(),
        ));
    }
    let replacement = ClipboardPayload {
        plain: "new".into(),
        ..ClipboardPayload::default()
    };
    board.write(&replacement, Some(captured.revision))?;
    match board.write(&original, Some(initial)) {
        Err(WriteError::Stale) => {}
        Err(error) => return Err(error),
        Ok(_) => {
            return Err(WriteError::Rejected(
                "stale clipboard write was accepted".into(),
            ));
        }
    }
    if board.read_text()?.text != replacement.plain {
        return Err(WriteError::Rejected(
            "stale clipboard write changed the pasteboard".into(),
        ));
    }
    Ok(written)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn native_pasteboard_and_stale_guard() {
        let mut board = MacPasteboard::unique();
        assert_eq!(board.read_text(), Err(WriteError::Empty));
        let old = board.revision().unwrap();
        let payload = ClipboardPayload {
            plain: "e\u{301}\n👩🏽‍💻".into(),
            html: Some("<p>Example</p>".into()),
            rtf: Some(r"{\rtf1 Example}".into()),
            ..ClipboardPayload::default()
        };
        let written = board.write(&payload, Some(old)).unwrap();
        assert_eq!(written.types, ["text/plain", "text/html", "text/rtf"]);
        assert_eq!(
            board.read_text().unwrap(),
            ClipboardSnapshot {
                text: payload.plain,
                revision: written.revision
            }
        );
        autoreleasepool(|_| {
            let native = board.pasteboard();
            assert_eq!(
                native
                    .stringForType(&NSString::from_str("public.html"))
                    .unwrap()
                    .to_string(),
                "<p>Example</p>"
            );
            assert_eq!(
                native
                    .dataForType(&NSString::from_str("public.rtf"))
                    .unwrap()
                    .to_vec(),
                br"{\rtf1 Example}"
            );
        });
        let plain = ClipboardPayload {
            plain: "plain".into(),
            html: Some(String::new()),
            rtf: Some(String::new()),
            signal: Some(String::new()),
        };
        assert_eq!(board.write(&plain, Some(old)), Err(WriteError::Stale));
        let next = board.write(&plain, Some(written.revision)).unwrap();
        assert_eq!(next.types, ["text/plain"]);
        autoreleasepool(|_| {
            let native = board.pasteboard();
            for flavor in ["public.html", "public.rtf", CHROMIUM_CUSTOM_DATA] {
                assert!(native.dataForType(&NSString::from_str(flavor)).is_none());
            }
        });
        assert_eq!(board.read_text().unwrap().text, "plain");
    }

    #[test]
    fn isolated_self_test() {
        assert_eq!(self_test().unwrap().types, ["text/plain", "text/html"]);
    }

    #[test]
    fn self_test_reports_missing_rich_data() {
        let mut board = crate::FakeBoard::default();
        assert_eq!(
            check_pasteboard(&mut board, |_| None),
            Err(WriteError::Rejected(
                "pasteboard types did not round-trip".into()
            ))
        );
    }

    struct BrokenBoard {
        board: crate::FakeBoard,
        accept_stale: bool,
    }

    impl Board for BrokenBoard {
        fn revision(&self) -> Result<Revision, WriteError> {
            self.board.revision()
        }

        fn read_text(&self) -> Result<ClipboardSnapshot, WriteError> {
            self.board.read_text()
        }

        fn write(
            &mut self,
            payload: &ClipboardPayload,
            expected: Option<Revision>,
        ) -> Result<WrittenClipboard, WriteError> {
            if self.accept_stale {
                return self.board.write(payload, None);
            }
            let written = self.board.write(payload, expected)?;
            if self.board.write_calls() == 2 {
                self.board.external_copy("wrong");
            }
            Ok(written)
        }
    }

    #[test]
    fn self_test_reports_an_accepted_stale_write_or_corrupt_replacement() {
        for (accept_stale, message) in [
            (true, "stale clipboard write was accepted"),
            (false, "stale clipboard write changed the pasteboard"),
        ] {
            let mut board = BrokenBoard {
                board: crate::FakeBoard::default(),
                accept_stale,
            };
            let result = check_pasteboard(&mut board, |board| {
                Some(
                    board
                        .board
                        .current_payload()?
                        .html
                        .as_ref()?
                        .as_bytes()
                        .to_vec(),
                )
            });
            assert_eq!(result, Err(WriteError::Rejected(message.into())));
        }
    }

    #[test]
    fn general_uses_the_shared_pasteboard() {
        assert_eq!(
            MacPasteboard::general().pasteboard().name(),
            NSPasteboard::generalPasteboard().name()
        );
        assert!(crate::system_board().is_ok());
    }
}
