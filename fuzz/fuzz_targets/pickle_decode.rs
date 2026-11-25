//! Fuzzes the Chromium Pickle custom-data codec.
//!
//! Raw bytes must decode or fail without panicking or allocating from untrusted counts,
//! and anything that decodes must re-encode to the same bytes (decoding is canonical).
//! Arbitrary `(mime, value)` pairs must round-trip through encode and decode.
#![no_main]

use arbitrary::Arbitrary;
use copied_codec::pickle::{
    decode_chromium_custom_data, encode_chromium_custom_data, encode_signal_pickle,
};
use libfuzzer_sys::fuzz_target;

#[derive(Arbitrary, Debug)]
enum Input {
    Raw(Vec<u8>),
    Entries(Vec<(String, String)>),
    Signal(String),
}

fn encode(entries: &[(String, String)]) -> Vec<u8> {
    let borrowed: Vec<(&str, &str)> = entries
        .iter()
        .map(|(mime, value)| (mime.as_str(), value.as_str()))
        .collect();
    encode_chromium_custom_data(&borrowed).expect("small inputs encode")
}

fuzz_target!(|input: Input| match input {
    Input::Raw(bytes) => {
        if let Ok(entries) = decode_chromium_custom_data(&bytes) {
            assert_eq!(
                encode(&entries),
                bytes,
                "decode accepted a non-canonical encoding"
            );
        }
    }
    Input::Entries(entries) => {
        let bytes = encode(&entries);
        assert_eq!(bytes.len() % 4, 0);
        assert_eq!(
            u32::from_le_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]) as usize,
            bytes.len() - 4
        );
        assert_eq!(
            decode_chromium_custom_data(&bytes).expect("round trip"),
            entries
        );
    }
    Input::Signal(html) => {
        let bytes = encode_signal_pickle(&html).expect("small inputs encode");
        assert_eq!(
            decode_chromium_custom_data(&bytes).expect("round trip"),
            vec![("text/signal".to_owned(), html)]
        );
    }
});
