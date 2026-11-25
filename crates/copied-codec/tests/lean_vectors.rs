//! Checks the Rust codecs against vectors computed by the proven Lean models.
//!
//! `formal/lean/vectors.json` is generated with `cd formal/lean && lake exe vectors > vectors.json`
//! and checked in, so this test does not need a Lean toolchain.

use copied_codec::cf_html::{encode_cf_html, parse_cf_html};
use copied_codec::pickle::{decode_chromium_custom_data, encode_chromium_custom_data};
use copied_codec::rtf::{escape_rtf, unescape_rtf};
use serde_json::Value;

const VECTORS: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../formal/lean/vectors.json"
));

fn vectors() -> Value {
    serde_json::from_str(VECTORS).expect("vectors.json is valid JSON")
}

fn cases<'a>(root: &'a Value, key: &str) -> &'a [Value] {
    let list = root[key]
        .as_array()
        .unwrap_or_else(|| panic!("missing {key}"));
    assert!(!list.is_empty(), "{key} has no vectors");
    list
}

fn text<'a>(case: &'a Value, key: &str) -> &'a str {
    case[key]
        .as_str()
        .unwrap_or_else(|| panic!("{key} is not a string in {case}"))
}

fn usize_field(case: &Value, key: &str) -> usize {
    usize::try_from(case[key].as_u64().unwrap()).unwrap()
}

fn hex(value: &str) -> Vec<u8> {
    assert_eq!(value.len() % 2, 0);
    (0..value.len())
        .step_by(2)
        .map(|i| u8::from_str_radix(&value[i..i + 2], 16).unwrap())
        .collect()
}

fn pairs(value: &Value) -> Vec<(String, String)> {
    value
        .as_array()
        .unwrap()
        .iter()
        .map(|pair| {
            (
                pair[0].as_str().unwrap().to_owned(),
                pair[1].as_str().unwrap().to_owned(),
            )
        })
        .collect()
}

#[test]
fn schema_version() {
    assert_eq!(vectors()["schemaVersion"], 1);
}

#[test]
fn pickle_encode_matches_lean() {
    let root = vectors();
    for case in cases(&root, "pickle") {
        let entries = pairs(&case["entries"]);
        let borrowed: Vec<(&str, &str)> = entries
            .iter()
            .map(|(m, v)| (m.as_str(), v.as_str()))
            .collect();
        let expected = hex(text(case, "hex"));
        assert_eq!(
            encode_chromium_custom_data(&borrowed).unwrap(),
            expected,
            "{}",
            case["name"]
        );
        assert_eq!(
            decode_chromium_custom_data(&expected).unwrap(),
            entries,
            "{}",
            case["name"]
        );
        assert_eq!(case["aligned"], true);
        assert_eq!(case["lean_roundtrip"], true);
    }
}

#[test]
fn pickle_decode_matches_lean() {
    let root = vectors();
    for case in cases(&root, "pickle_decode") {
        let bytes = hex(text(case, "hex"));
        let decoded = decode_chromium_custom_data(&bytes);
        if case["entries"].is_null() {
            assert!(decoded.is_err(), "{} should be rejected", case["name"]);
        } else {
            assert_eq!(
                decoded.unwrap(),
                pairs(&case["entries"]),
                "{}",
                case["name"]
            );
        }
    }
}

#[test]
fn cf_html_matches_lean() {
    let root = vectors();
    for case in cases(&root, "cf_html") {
        let fragment = text(case, "fragment");
        let payload = encode_cf_html(fragment);
        assert_eq!(payload, text(case, "payload"), "{}", case["name"]);
        assert_eq!(payload.as_bytes(), hex(text(case, "payload_hex")));
        let offsets = parse_cf_html(&payload).unwrap();
        assert_eq!(offsets.start_html, usize_field(case, "start_html"));
        assert_eq!(offsets.end_html, usize_field(case, "end_html"));
        assert_eq!(offsets.start_fragment, usize_field(case, "start_fragment"));
        assert_eq!(offsets.end_fragment, usize_field(case, "end_fragment"));
        assert_eq!(
            &payload[offsets.start_fragment..offsets.end_fragment],
            fragment
        );
    }
}

#[test]
fn rtf_escape_matches_lean() {
    let root = vectors();
    for case in cases(&root, "rtf_escape") {
        let input = text(case, "input");
        let units: Vec<u64> = input.encode_utf16().map(u64::from).collect();
        let lean_units: Vec<u64> = case["utf16"]
            .as_array()
            .unwrap()
            .iter()
            .map(|u| u.as_u64().unwrap())
            .collect();
        assert_eq!(units, lean_units, "{}", case["name"]);
        let escaped = escape_rtf(input);
        assert_eq!(escaped, text(case, "escaped"), "{}", case["name"]);
        assert!(escaped.is_ascii());
        assert_eq!(unescape_rtf(&escaped).unwrap(), input);
        assert_eq!(case["lean_roundtrip"], true);
    }
}

#[test]
fn rtf_unescape_matches_lean() {
    let root = vectors();
    for case in cases(&root, "rtf_unescape") {
        let result = unescape_rtf(text(case, "input"));
        match case["output"].as_str() {
            Some(expected) => assert_eq!(result.unwrap(), expected, "{}", case["name"]),
            None => assert!(result.is_err(), "{} should be rejected", case["name"]),
        }
    }
}

#[test]
fn rtf_styled_runs_are_balanced() {
    let root = vectors();
    for case in cases(&root, "rtf_styled") {
        let rtf = text(case, "rtf");
        assert!(rtf.is_ascii());
        let mut depth = 0i32;
        let mut escaped = false;
        for byte in rtf.bytes() {
            match (escaped, byte) {
                (true, _) => escaped = false,
                (false, b'\\') => escaped = true,
                (false, b'{') => depth += 1,
                (false, b'}') => depth -= 1,
                _ => {}
            }
            assert!(depth >= 0, "{}", case["name"]);
        }
        assert_eq!(depth, 0, "{}", case["name"]);
        assert_eq!(case["balanced"], true);
    }
}

#[test]
fn spoiler_and_visibility_vectors_are_present() {
    // The spoiler scanner and format resolver live in other crates. Here we only check the
    // shape so a broken generator fails fast.
    let root = vectors();
    for case in cases(&root, "spoiler") {
        assert!(case["tokens"].is_array());
        assert!(case["plain"].is_string());
        let malformed = case["malformed"].as_bool().unwrap();
        // Fail closed: malformed input never keeps the rich payload.
        assert_eq!(
            case["rich"].as_bool().unwrap(),
            !malformed,
            "{}",
            case["name"]
        );
    }
    for case in cases(&root, "visibility") {
        let visible: Vec<&str> = case["visible"]
            .as_array()
            .unwrap()
            .iter()
            .map(|v| v.as_str().unwrap())
            .collect();
        assert!(!visible.is_empty(), "{}", case["name"]);
        assert!(
            visible.contains(&text(case, "preferred")),
            "{}",
            case["name"]
        );
        assert!(
            visible.contains(&text(case, "resolved")),
            "{}",
            case["name"]
        );
    }
}
