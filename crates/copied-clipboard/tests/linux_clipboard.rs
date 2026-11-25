#![cfg(target_os = "linux")]

use copied_clipboard::{
    ClipboardSnapshot, WriteError,
    linux::{Session, desktop_session},
    system_board,
};
use copied_core::ClipboardPayload;

#[test]
#[ignore = "run only against a disposable X11 or Wayland session"]
fn cross_client_clipboard_round_trip_and_stale_guard() {
    assert_ne!(desktop_session(), Session::Headless);
    let mut producer = system_board().unwrap();
    let mut consumer = system_board().unwrap();
    let original = ClipboardPayload {
        // Large enough to exercise X11 INCR and Wayland pipe streaming.
        plain: "Hello, 🌍\n".repeat(28_000),
        html: Some("<b>Hello, 🌍</b>".into()),
        rtf: Some(r"{\rtf1 Hello}".into()),
        ..ClipboardPayload::default()
    };
    let first = producer.write(&original, None).unwrap();
    assert_eq!(first.revision, producer.read_text().unwrap().revision);
    assert_eq!(first.types, ["text/plain", "text/html", "text/rtf"]);
    let captured = consumer.read_text().unwrap();
    assert_eq!(captured.text, original.plain);
    for (board, revision) in [(&producer, first.revision), (&consumer, captured.revision)] {
        assert_eq!(
            board.read_html().unwrap(),
            original
                .html
                .clone()
                .map(|text| ClipboardSnapshot { text, revision })
        );
    }
    let external = ClipboardPayload {
        plain: "new external text".into(),
        ..ClipboardPayload::default()
    };
    producer.write(&external, Some(first.revision)).unwrap();
    assert_eq!(
        consumer.write(&original, Some(captured.revision)),
        Err(WriteError::Stale)
    );
    assert_eq!(consumer.read_text().unwrap().text, external.plain);
    assert_eq!(producer.read_html(), Ok(None));
    assert_eq!(consumer.read_html(), Ok(None));
}
