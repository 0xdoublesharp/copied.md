use std::cell::Cell;

use super::*;

fn payload(plain: &str) -> ClipboardPayload {
    ClipboardPayload {
        plain: plain.into(),
        ..ClipboardPayload::default()
    }
}

fn external(board: &mut FakeBoard) {
    board.external_copy("New external text");
}

#[test]
fn clipboard_snapshot_detects_external_copy_during_read() {
    let revision = Cell::new(Revision(0));
    let read = || {
        revision.set(Revision(1));
        Ok(Some("original".into()))
    };
    assert_eq!(
        crate::stable_snapshot(|| Ok(revision.get()), read),
        Err(WriteError::Stale)
    );
}

#[test]
fn copy_before_clear_preserves_the_new_owner() {
    let mut board = FakeBoard::with_text("original");
    let revision = board.read_text().unwrap().revision;
    board.on_before_clear = Some(external);
    assert_eq!(
        board.write(&payload("converted"), Some(revision)),
        Err(WriteError::Stale)
    );
    assert_eq!(board.read_text().unwrap().text, "New external text");
    assert_eq!(board.revision().unwrap(), Revision(1));
    assert_eq!(board.write_calls(), 0);
}

#[test]
fn copy_after_clear_preserves_the_new_owner_even_without_expected_revision() {
    for expected in [None, Some(Revision(0))] {
        let mut board = FakeBoard::with_text("original");
        board.on_after_clear = Some(external);
        assert_eq!(
            board.write(&payload("converted"), expected),
            Err(WriteError::Stale)
        );
        assert_eq!(board.read_text().unwrap().text, "New external text");
        assert_eq!(board.revision().unwrap(), Revision(2));
        assert_eq!(board.write_calls(), 0);
    }
}

#[test]
fn prepares_all_flavors_before_clearing() {
    for item in [
        ClipboardPayload {
            html: Some("<b>Example</b>".into()),
            ..payload("Example")
        },
        ClipboardPayload {
            rtf: Some(r"{\rtf1 Example}".into()),
            ..payload("Example")
        },
        ClipboardPayload {
            signal: Some("<b>Example</b>".into()),
            ..payload("Example")
        },
    ] {
        let mut board = FakeBoard::with_text("original");
        board.fail_prepare = true;
        board.on_before_clear = Some(|_| panic!("preparation should precede clearing"));
        assert!(matches!(
            board.write(&item, Some(Revision(0))),
            Err(WriteError::Rejected(_))
        ));
        assert_eq!(board.read_text().unwrap().text, "original");
        assert_eq!(board.revision().unwrap(), Revision(0));
        assert_eq!(board.write_calls(), 0);
    }
}

#[test]
fn reports_rejection_after_clear() {
    let mut board = FakeBoard::with_text("original");
    board.fail_write = true;
    assert!(matches!(
        board.write(&payload("converted"), None),
        Err(WriteError::Rejected(_))
    ));
    assert_eq!(board.revision().unwrap(), Revision(1));
    assert_eq!(board.read_text(), Err(WriteError::Empty));
    assert_eq!(board.write_calls(), 1);
}

#[test]
fn omits_empty_flavors_and_rejects_stale_or_empty_writes() {
    let mut board = FakeBoard::default();
    assert_eq!(board.read_text(), Err(WriteError::Empty));
    let values = [
        "**Ready**",
        "👩🏽‍💻 🛰️",
        "e\u{301}\nsecond line",
        "tabs\t0",
        "end \\ { }",
    ];
    for (index, value) in values.into_iter().enumerate() {
        let expected = board.revision().unwrap();
        let rich = index % 2 == 0;
        let item = ClipboardPayload {
            html: Some(if rich { "<strong>Ready</strong>" } else { "" }.into()),
            rtf: Some(if rich { r"{\rtf1 Ready}" } else { "" }.into()),
            signal: Some(if rich { "<strong>Ready</strong>" } else { "" }.into()),
            ..payload(value)
        };
        let written = board.write(&item, Some(expected)).unwrap();
        let snapshot = board.read_text().unwrap();
        assert_eq!(snapshot.text, value);
        assert_ne!(snapshot.revision, expected);
        assert_eq!(snapshot.revision, written.revision);
        let mut types = vec!["text/plain"];
        if rich {
            types.extend(["text/html", "text/rtf", "text/signal"]);
            assert_eq!(board.current_payload(), Some(&item));
        } else {
            assert_eq!(board.current_payload(), Some(&payload(value)));
        }
        assert_eq!(written.types, types);
        assert_eq!(
            board.write(&payload("stale"), Some(expected)),
            Err(WriteError::Stale)
        );
        for empty in [" \n\t", ""] {
            let item = ClipboardPayload {
                html: Some("<b>Ready</b>".into()),
                ..payload(empty)
            };
            assert_eq!(board.write(&item, None), Err(WriteError::Empty));
        }
        assert_eq!(board.read_text(), Ok(snapshot));
        assert_eq!(board.write_calls(), index + 1);
    }
}
