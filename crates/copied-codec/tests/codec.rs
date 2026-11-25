//! Example-based and error-path tests for every codec.

use copied_codec::cf_html::{HEADER_LEN, encode_cf_html, parse_cf_html};
use copied_codec::pickle::{
    decode_chromium_custom_data, encode_chromium_custom_data, encode_signal_pickle,
};
use copied_codec::rtf::{escape_rtf, unescape_rtf};
use copied_codec::{CfHtmlOffsets, CodecError};

// ---------------------------------------------------------------- errors

#[test]
fn error_display_covers_every_variant() {
    let cases = [
        (CodecError::Truncated, "codec input truncated"),
        (
            CodecError::LengthMismatch {
                declared: 8,
                actual: 4,
            },
            "codec payload length mismatch: declared 8, actual 4",
        ),
        (
            CodecError::InvalidAlignment,
            "codec payload not 4-byte aligned",
        ),
        (CodecError::InvalidUtf16, "invalid UTF-16 sequence"),
        (CodecError::InvalidCfHtmlHeader, "invalid CF_HTML header"),
        (CodecError::InvalidRtfEscape, "invalid RTF escape sequence"),
        (CodecError::Overflow, "codec integer overflow"),
    ];
    for (error, message) in cases {
        assert_eq!(error.to_string(), message);
    }
    assert_is_error(&CodecError::Overflow);
}

fn assert_is_error<E: std::error::Error>(error: &E) {
    assert!(error.source().is_none());
}

// ---------------------------------------------------------------- pickle

/// Independent port of `signalPasteboardData` in `apps/macos/Pasteboard.swift`.
fn swift_signal_pasteboard_data(html: &str) -> Vec<u8> {
    let mut payload = Vec::new();
    payload.extend_from_slice(&1u32.to_le_bytes());
    for value in ["text/signal", html] {
        let units: Vec<u16> = value.encode_utf16().collect();
        payload.extend_from_slice(&u32::try_from(units.len()).unwrap().to_le_bytes());
        for unit in units {
            payload.extend_from_slice(&unit.to_le_bytes());
        }
        payload.extend(std::iter::repeat_n(0, (4 - payload.len() % 4) % 4));
    }
    let mut result = u32::try_from(payload.len()).unwrap().to_le_bytes().to_vec();
    result.extend_from_slice(&payload);
    result
}

#[test]
fn signal_pickle_matches_swift_encoder() {
    for html in ["", "a", "ab", "<b>hi</b>", "Café 🚀", "日本語"] {
        assert_eq!(
            encode_signal_pickle(html).unwrap(),
            swift_signal_pasteboard_data(html)
        );
    }
}

#[test]
fn pickle_layout_is_exact() {
    let bytes = encode_chromium_custom_data(&[("ab", "c")]).unwrap();
    assert_eq!(
        bytes,
        [
            20, 0, 0, 0, // payload size
            1, 0, 0, 0, // entry count
            2, 0, 0, 0, b'a', 0, b'b', 0, // even: no padding
            1, 0, 0, 0, b'c', 0, 0, 0, // odd: two pad bytes
        ]
    );
    assert_eq!(
        encode_chromium_custom_data(&[]).unwrap(),
        [4, 0, 0, 0, 0, 0, 0, 0]
    );
}

#[test]
fn pickle_round_trips_examples() {
    let entries = [("text/signal", "Hi 🚀"), ("text/plain", ""), ("x", "Café")];
    let bytes = encode_chromium_custom_data(&entries).unwrap();
    assert_eq!(bytes.len() % 4, 0);
    let decoded = decode_chromium_custom_data(&bytes).unwrap();
    let expected: Vec<(String, String)> = entries
        .iter()
        .map(|&(m, v)| (m.to_owned(), v.to_owned()))
        .collect();
    assert_eq!(decoded, expected);
}

#[test]
fn pickle_decode_rejects_malformed_input() {
    let cases: [(&[u8], CodecError); 9] = [
        (&[], CodecError::Truncated),
        (&[4, 0], CodecError::Truncated),
        (
            &[8, 0, 0, 0, 0, 0, 0, 0],
            CodecError::LengthMismatch {
                declared: 8,
                actual: 4,
            },
        ),
        (&[2, 0, 0, 0, 0, 0], CodecError::InvalidAlignment),
        (&[0, 0, 0, 0], CodecError::Truncated),
        (&[4, 0, 0, 0, 1, 0, 0, 0], CodecError::Truncated),
        // Declared unit count longer than the remaining bytes.
        (
            &[12, 0, 0, 0, 1, 0, 0, 0, 3, 0, 0, 0, 97, 0, 0, 0],
            CodecError::Truncated,
        ),
        // Odd count followed by a non-zero pad byte.
        (
            &[
                20, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 97, 0, 7, 0, 1, 0, 0, 0, 98, 0, 0, 0,
            ],
            CodecError::InvalidAlignment,
        ),
        // Lone high surrogate.
        (
            &[
                16, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 0x3D, 0xD8, 0, 0, 0, 0, 0, 0,
            ],
            CodecError::InvalidUtf16,
        ),
    ];
    for (bytes, error) in cases {
        assert_eq!(decode_chromium_custom_data(bytes), Err(error), "{bytes:?}");
    }
}

#[test]
fn pickle_decode_rejects_trailing_payload() {
    let bytes = [16, 0, 0, 0, 1, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0];
    assert_eq!(
        decode_chromium_custom_data(&bytes),
        Err(CodecError::LengthMismatch {
            declared: 16,
            actual: 12
        })
    );
}

#[test]
fn pickle_decode_does_not_trust_entry_count() {
    let bytes = [4, 0, 0, 0, 0xFF, 0xFF, 0xFF, 0xFF];
    assert_eq!(
        decode_chromium_custom_data(&bytes),
        Err(CodecError::Truncated)
    );
    let bytes = [8, 0, 0, 0, 1, 0, 0, 0, 0xFF, 0xFF, 0xFF, 0x7F];
    assert_eq!(
        decode_chromium_custom_data(&bytes),
        Err(CodecError::Truncated)
    );
}

// ---------------------------------------------------------------- CF_HTML

#[test]
fn cf_html_envelope_is_exact() {
    let payload = encode_cf_html("<b>hi</b>");
    assert_eq!(
        payload,
        "Version:0.9\r\nStartHTML:0000000105\r\nEndHTML:0000000182\r\n\
         StartFragment:0000000139\r\nEndFragment:0000000148\r\n\
         <html><body>\r\n<!--StartFragment--><b>hi</b><!--EndFragment-->\r\n</body></html>"
    );
    assert_eq!(&payload[HEADER_LEN..HEADER_LEN + 6], "<html>");
}

#[test]
fn cf_html_offsets_use_utf8_bytes() {
    let fragment = "Café 🚀";
    let payload = encode_cf_html(fragment);
    let offsets = parse_cf_html(&payload).unwrap();
    assert_eq!(
        offsets,
        CfHtmlOffsets {
            start_html: Some(105),
            end_html: Some(173 + 10),
            start_fragment: 139,
            end_fragment: 149
        }
    );
    assert_eq!(
        &payload[offsets.start_fragment..offsets.end_fragment],
        fragment
    );
    assert_eq!(offsets.end_html, Some(payload.len()));
}

const BODY: &str = "<html><body>\r\n<!--StartFragment-->x<!--EndFragment-->\r\n</body></html>";

#[test]
fn cf_html_parse_accepts_lf_and_extra_keys() {
    let head = |sh: usize, eh: usize, sf: usize, ef: usize| {
        format!(
            "Version:1.0\nStartHTML:{sh:010}\nEndHTML:{eh:010}\nStartFragment:{sf:010}\n\
             EndFragment:{ef:010}\nSourceURL:https://example.com/\n"
        )
    };
    let start_html = head(0, 0, 0, 0).len();
    let start_fragment = start_html + BODY.find("-->x").unwrap() + 3;
    let end_html = start_html + BODY.len();
    let payload = head(start_html, end_html, start_fragment, start_fragment + 1) + BODY;
    let offsets = parse_cf_html(&payload).unwrap();
    assert_eq!(offsets.start_html, Some(start_html));
    assert_eq!(&payload[offsets.start_fragment..offsets.end_fragment], "x");
}

#[test]
fn cf_html_parse_allows_trailing_bytes_after_end_html() {
    let mut payload = encode_cf_html("x");
    payload.push('\0');
    assert!(parse_cf_html(&payload).is_ok());
}

fn with_field(payload: &str, key: &str, value: &str) -> String {
    let start = payload.find(key).unwrap() + key.len();
    let end = start + payload[start..].find('\r').unwrap();
    format!("{}{}{}", &payload[..start], value, &payload[end..])
}

#[test]
fn cf_html_parse_accepts_no_context() {
    for (newline, sentinel, fragment) in [
        ("\r\n", "-1", "<p>Café 🚀</p>"),
        ("\n", "-000000001", "StartHTML: Café 🚀\nFin"),
    ] {
        let header = |start: usize, end: usize| {
            format!(
                "Version:1.0{newline}StartHTML:{sentinel}{newline}EndHTML:{sentinel}{newline}\
                 StartFragment:{start:010}{newline}EndFragment:{end:010}{newline}\
                 SourceURL:https://example.com/{newline}"
            )
        };
        let start_fragment = header(0, 0).len();
        let end_fragment = start_fragment + fragment.len();
        let payload = header(start_fragment, end_fragment) + fragment;
        let offsets = parse_cf_html(&payload).unwrap();
        assert_eq!(offsets.start_html, None);
        assert_eq!(offsets.end_html, None);
        assert_eq!(offsets.start_fragment, start_fragment);
        assert_eq!(offsets.end_fragment, end_fragment);
        assert_eq!(
            &payload[offsets.start_fragment..offsets.end_fragment],
            fragment
        );
    }
}

#[test]
fn cf_html_parse_rejects_bad_no_context_offsets() {
    let header = |start: usize, end: usize| {
        format!(
            "Version:1.0\r\nStartHTML:-1\r\nEndHTML:-1\r\n\
             StartFragment:{start:010}\r\nEndFragment:{end:010}\r\nStartSelection:0\r\nEndSelection:0\r\n"
        )
    };
    let start = header(0, 0).len();
    let end = start + "é".len();
    let good = header(start, end) + "é";
    let bad = [
        with_field(&good, "StartHTML:", "00"),
        with_field(&good, "EndHTML:", "99"),
        with_field(&good, "EndHTML:", "-2"),
        with_field(&good, "StartFragment:", "-000000001"),
        header(start, end + 1) + "é",
        header(end, start) + "é",
        header(start + 1, end) + "é",
        header(start, end - 1) + "é",
        header(start - 3, end) + "é",
    ];
    for payload in &bad {
        assert_eq!(
            parse_cf_html(payload),
            Err(CodecError::InvalidCfHtmlHeader),
            "{payload:?}"
        );
    }
}

#[test]
fn cf_html_parse_rejects_bad_headers() {
    let good = encode_cf_html("é");
    let bad = [
        // Header must start with Version.
        good.replacen("Version:0.9", "Versio:0.9", 1),
        // Missing a required field.
        good.replacen("EndFragment:", "EndFragmentX:", 1),
        // Duplicate field.
        good.replacen("EndHTML:", "StartHTML:", 1),
        // Empty value.
        with_field(&good, "StartHTML:", ""),
        // Not a number.
        with_field(&good, "StartHTML:", "-abc"),
        // StartFragment before StartHTML.
        with_field(&good, "StartFragment:", "0000000100"),
        // EndHTML past the payload.
        with_field(&good, "EndHTML:", "0000099999"),
        // Header overlaps StartHTML.
        with_field(&good, "StartHTML:", "0000000001"),
        // EndFragment inside the two-byte `é`.
        with_field(&good, "EndFragment:", "0000000140"),
        // Fragment not enclosed by markers.
        with_field(&good, "StartFragment:", "0000000138"),
        // Missing end marker.
        good.replacen("<!--EndFragment-->", "<!--EndFragmenX-->", 1),
        // No header at all.
        String::from("<html><body></body></html>"),
        // Header line with a colon but a non-alphanumeric key ends the header early.
        good.replacen("StartHTML:", "Start HTML:", 1),
    ];
    for payload in &bad {
        assert_eq!(
            parse_cf_html(payload),
            Err(CodecError::InvalidCfHtmlHeader),
            "{payload:?}"
        );
    }
    // `usize` overflow, first in the multiply and then in the add (64-bit targets).
    for digits in ["99999999999999999999999", "18446744073709551616"] {
        let overflow = with_field(&good, "StartHTML:", digits);
        assert_eq!(
            parse_cf_html(&overflow),
            Err(CodecError::Overflow),
            "{digits}"
        );
    }
    let empty_key = good.replacen("StartHTML:", ":", 1);
    assert_eq!(
        parse_cf_html(&empty_key),
        Err(CodecError::InvalidCfHtmlHeader)
    );
}

// ---------------------------------------------------------------- RTF

#[test]
fn rtf_escape_matches_render_text() {
    let cases = [
        ("Café", "Caf\\u233?"),
        ("🚀", "\\u-10179?\\u-8576?"),
        ("Café 🚀", "Caf\\u233? \\u-10179?\\u-8576?"),
        ("{a\\b}", "\\{a\\\\b\\}"),
        ("tab\tnew\nline ~", "tab\tnew\nline ~"),
        ("\r\u{7f}\0", "\\u13?\\u127?\\u0?"),
        ("\u{7fff}\u{8000}\u{ffff}", "\\u32767?\\u-32768?\\u-1?"),
    ];
    for (input, escaped) in cases {
        assert_eq!(escape_rtf(input), escaped);
        assert_eq!(unescape_rtf(escaped).unwrap(), input);
    }
}

#[test]
fn rtf_unescape_accepts_documented_forms() {
    assert_eq!(unescape_rtf("\\u0065?").unwrap(), "A");
    assert_eq!(unescape_rtf("\\u-32768?").unwrap(), "\u{8000}");
    assert_eq!(unescape_rtf("plain é").unwrap(), "plain é");
    assert_eq!(unescape_rtf("\\\\\\{\\}").unwrap(), "\\{}");
    assert_eq!(unescape_rtf("").unwrap(), "");
}

#[test]
fn rtf_unescape_rejects_malformed_input() {
    let escapes = [
        "{",
        "a}",
        "\\x",
        "\\",
        "\\u",
        "\\u?",
        "\\u-?",
        "\\u--1?",
        "\\u1-2?",
        "\\u-0?",
        "\\u32768?",
        "\\u-32769?",
        "\\u12",
        "\\u12x",
        "\\u99999999999999999999?",
    ];
    for input in escapes {
        assert_eq!(
            unescape_rtf(input),
            Err(CodecError::InvalidRtfEscape),
            "{input:?}"
        );
    }
    for input in ["\\u-10179?", "\\u-8576?x", "\\u-10179?\\u65?"] {
        assert_eq!(
            unescape_rtf(input),
            Err(CodecError::InvalidUtf16),
            "{input:?}"
        );
    }
}
