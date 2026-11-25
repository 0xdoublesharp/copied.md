//! Fuzzes RTF text escaping.
//!
//! `unescape_rtf` must never panic on arbitrary input, and anything it accepts must
//! survive another escape/unescape cycle. `escape_rtf` must emit ASCII with balanced
//! (in fact, fully escaped) braces and `unescape_rtf` must recover the original exactly.
#![no_main]

use arbitrary::Arbitrary;
use copied_codec::rtf::{escape_rtf, unescape_rtf};
use copied_fuzz::{assert_balanced_rtf, from_alphabet};
use libfuzzer_sys::fuzz_target;

#[derive(Arbitrary, Debug)]
enum Input {
    Unescape(String),
    /// Escape-heavy alphabet so `\uN?` and brace paths are reached quickly.
    UnescapeDense(Vec<u8>),
    RoundTrip(String),
}

fn check_round_trip(value: &str) {
    let escaped = escape_rtf(value);
    assert!(
        escaped.is_ascii(),
        "non-ASCII escape of {value:?}: {escaped:?}"
    );
    assert_balanced_rtf(&escaped);
    assert_eq!(unescape_rtf(&escaped).as_deref(), Ok(value), "{escaped:?}");
}

fn check_unescape(value: &str) {
    if let Ok(decoded) = unescape_rtf(value) {
        assert_eq!(unescape_rtf(value).as_deref(), Ok(decoded.as_str()));
        check_round_trip(&decoded);
    }
}

fuzz_target!(|input: Input| match input {
    Input::Unescape(value) => check_unescape(&value),
    Input::UnescapeDense(bytes) => check_unescape(&from_alphabet(&bytes, b"\\u-?0123456789{}a ")),
    Input::RoundTrip(value) => check_round_trip(&value),
});
