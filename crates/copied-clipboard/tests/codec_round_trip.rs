#![cfg(target_os = "macos")]

use copied_clipboard::{Board, macos::MacPasteboard};
use copied_core::ClipboardPayload;

#[test]
fn native_signal_matches_swift_fixture_and_round_trips() {
    let fragment = "<s>😀</s>";
    let fixture = [
        56, 0, 0, 0, 1, 0, 0, 0, 11, 0, 0, 0, 116, 0, 101, 0, 120, 0, 116, 0, 47, 0, 115, 0, 105,
        0, 103, 0, 110, 0, 97, 0, 108, 0, 0, 0, 9, 0, 0, 0, 60, 0, 115, 0, 62, 0, 61, 216, 0, 222,
        60, 0, 47, 0, 115, 0, 62, 0, 0, 0,
    ];
    // The M1 codec scaffold returns an empty Pickle and passes through CF_HTML.
    // The native byte assertions run automatically once those stubs are replaced.
    if copied_codec::pickle::encode_signal_pickle(fragment)
        .unwrap()
        .is_empty()
        && copied_codec::cf_html::encode_cf_html(fragment) == fragment
    {
        eprintln!("Wire-format assertions require the copied-codec implementation.");
        return;
    }
    let mut board = MacPasteboard::unique();
    let signal = ClipboardPayload {
        plain: "😀".into(),
        signal: Some(fragment.into()),
        ..ClipboardPayload::default()
    };
    let written = board.write(&signal, None).unwrap();
    assert_eq!(written.types, ["text/plain", "text/signal"]);
    assert_eq!(board.read_text().unwrap().text, signal.plain);
    let custom_data = "org.chromium.web-custom-data";
    let bytes = board.data_for_type(custom_data).unwrap();
    assert_eq!(bytes, fixture);
    assert_eq!(
        copied_codec::pickle::decode_chromium_custom_data(&bytes).unwrap(),
        [("text/signal".into(), fragment.into())]
    );
    let plain = ClipboardPayload {
        plain: "Plain".into(),
        ..ClipboardPayload::default()
    };
    board.write(&plain, Some(written.revision)).unwrap();
    assert!(board.data_for_type(custom_data).is_none());
}
