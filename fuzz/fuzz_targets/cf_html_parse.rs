//! Fuzzes the Windows `CF_HTML` envelope codec.
//!
//! Arbitrary payloads must parse or fail without panicking, and accepted offsets must be
//! ordered, on character boundaries and framed by the fragment markers. Arbitrary
//! fragments must round-trip through `encode_cf_html` and `parse_cf_html` exactly.
//! A structured branch builds well-formed headers with fuzzed offsets and extra keys so
//! the fuzzer reaches the offset checks instead of stalling on the `Version:` line.
#![no_main]

use arbitrary::Arbitrary;
use copied_codec::cf_html::{HEADER_LEN, encode_cf_html, parse_cf_html};
use libfuzzer_sys::fuzz_target;

const START_MARKER: &str = "<!--StartFragment-->";
const END_MARKER: &str = "<!--EndFragment-->";

#[derive(Arbitrary, Debug)]
struct Field {
    /// 0..=3 selects a known offset key; anything else is an extra key.
    slot: u8,
    value: u32,
    digits: u8,
    extra_key: String,
    crlf: bool,
}

#[derive(Arbitrary, Debug)]
enum Input {
    Raw(String),
    Fragment(String),
    Structured {
        fields: Vec<Field>,
        body: String,
        corrupt_at: Option<(u16, u8)>,
    },
}

fn check_accepted(payload: &str) {
    let Ok(offsets) = parse_cf_html(payload) else {
        return;
    };
    assert!(offsets.start_html <= offsets.start_fragment);
    assert!(offsets.start_fragment <= offsets.end_fragment);
    assert!(offsets.end_fragment <= offsets.end_html);
    assert!(offsets.end_html <= payload.len());
    for offset in [
        offsets.start_html,
        offsets.start_fragment,
        offsets.end_fragment,
        offsets.end_html,
    ] {
        assert!(payload.is_char_boundary(offset));
    }
    assert!(payload[offsets.start_html..offsets.start_fragment].ends_with(START_MARKER));
    assert!(payload[offsets.end_fragment..offsets.end_html].starts_with(END_MARKER));
    assert_eq!(parse_cf_html(payload), Ok(offsets), "non-deterministic");
}

fn structured(fields: &[Field], body: &str) -> String {
    const KEYS: [&str; 4] = ["StartHTML", "EndHTML", "StartFragment", "EndFragment"];
    let mut payload = String::from("Version:0.9\r\n");
    for field in fields.iter().take(8) {
        let key = match KEYS.get(usize::from(field.slot)) {
            Some(key) => (*key).to_owned(),
            None => field.extra_key.clone(),
        };
        let width = usize::from(field.digits % 12);
        payload.push_str(&format!("{key}:{:0width$}", field.value));
        payload.push_str(if field.crlf { "\r\n" } else { "\n" });
    }
    payload.push_str(body);
    payload
}

fuzz_target!(|input: Input| match input {
    Input::Raw(payload) => check_accepted(&payload),
    Input::Fragment(fragment) => {
        let payload = encode_cf_html(&fragment);
        let offsets = parse_cf_html(&payload).expect("encoded payload parses");
        assert_eq!(offsets.start_html, HEADER_LEN);
        assert_eq!(offsets.end_html, payload.len());
        assert_eq!(
            &payload[offsets.start_fragment..offsets.end_fragment],
            fragment
        );
        check_accepted(&payload);
    }
    Input::Structured {
        fields,
        body,
        corrupt_at,
    } => {
        let mut payload = structured(&fields, &body);
        check_accepted(&payload);
        if let Some((at, byte)) = corrupt_at {
            let at = usize::from(at) % (payload.len() + 1);
            if payload.is_char_boundary(at) {
                payload.insert(at, char::from(byte & 0x7f));
                check_accepted(&payload);
            }
        }
    }
});
