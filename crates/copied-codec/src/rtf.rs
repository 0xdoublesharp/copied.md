//! ASCII-safe RTF text escaping.
//!
//! This matches `escapeRtf` in `packages/converters/src/render-text.ts` and the Lean model
//! `CopiedFormal.Rtf`:
//!
//! * `\`, `{` and `}` become `\\`, `\{` and `\}`.
//! * Tab, newline and printable ASCII (`0x20..=0x7E`) pass through.
//! * Any other UTF-16 code unit becomes `\uN?`, where `N` is the unit read as a signed
//!   16-bit integer. For example `é` is `\u233?` and `🚀` is `\u-10179?\u-8576?`.
//!
//! [`unescape_rtf`] accepts:
//!
//! | input             | meaning                                 |
//! |-------------------|-----------------------------------------|
//! | `\\`, `\{`, `\}`  | the literal character                   |
//! | `\uN?`            | code unit `N`, `0 <= N <= 32767`        |
//! | `\u-N?`           | code unit `65536 - N`, `1 <= N <= 32768`|
//! | unescaped `{`/`}` | rejected                                |
//! | any other char    | itself                                  |
//!
//! `N` is one or more ASCII digits. Leading zeros are allowed. `\u-0?` is rejected.

use alloc::string::String;
use alloc::vec::Vec;
use core::str::Chars;

use crate::CodecError;

const DIGITS: &[u8; 10] = b"0123456789";

fn push_unit_escape(out: &mut String, unit: u16) {
    let signed = i16::from_ne_bytes(unit.to_ne_bytes());
    out.push_str("\\u");
    if signed < 0 {
        out.push('-');
    }
    let mut magnitude = signed.unsigned_abs();
    let mut buf = [0u8; 5];
    let mut start = buf.len();
    loop {
        start -= 1;
        buf[start] = DIGITS[usize::from(magnitude % 10)];
        magnitude /= 10;
        if magnitude == 0 {
            break;
        }
    }
    for &digit in &buf[start..] {
        out.push(char::from(digit));
    }
    out.push('?');
}

/// Escapes a UTF-8 string into ASCII-safe RTF using signed 16-bit UTF-16 `\uN?` escapes.
pub fn escape_rtf(value: &str) -> String {
    let mut out = String::with_capacity(value.len());
    for c in value.chars() {
        match c {
            '\\' | '{' | '}' => {
                out.push('\\');
                out.push(c);
            }
            '\t' | '\n' | ' '..='~' => out.push(c),
            _ => {
                let mut buf = [0u16; 2];
                for &unit in c.encode_utf16(&mut buf).iter() {
                    push_unit_escape(&mut out, unit);
                }
            }
        }
    }
    out
}

fn push_char(units: &mut Vec<u16>, c: char) {
    let mut buf = [0u16; 2];
    units.extend_from_slice(c.encode_utf16(&mut buf));
}

/// Parses `[-]digits?` after `\u` into a UTF-16 code unit.
fn parse_unit(chars: &mut Chars<'_>) -> Result<u16, CodecError> {
    let mut negative = false;
    let mut seen_digit = false;
    let mut magnitude: u32 = 0;
    loop {
        match chars.next() {
            Some('-') if !negative && !seen_digit => negative = true,
            Some(c @ '0'..='9') => {
                magnitude = magnitude * 10 + (u32::from(c) - u32::from('0'));
                // Every accepted value is at most 32768, so larger magnitudes can stop here.
                if magnitude > 32768 {
                    return Err(CodecError::InvalidRtfEscape);
                }
                seen_digit = true;
            }
            Some('?') if seen_digit => break,
            _ => return Err(CodecError::InvalidRtfEscape),
        }
    }
    let unit = match (negative, magnitude) {
        (true, 1..=32768) => 65536 - magnitude,
        (false, 0..=32767) => magnitude,
        _ => return Err(CodecError::InvalidRtfEscape),
    };
    u16::try_from(unit).map_err(|_| CodecError::InvalidRtfEscape)
}

/// Unescapes an RTF-escaped text run back into UTF-8.
///
/// Returns [`CodecError::InvalidRtfEscape`] for malformed escapes or unescaped braces, and
/// [`CodecError::InvalidUtf16`] when `\uN?` escapes leave an unpaired surrogate.
pub fn unescape_rtf(value: &str) -> Result<String, CodecError> {
    let mut units: Vec<u16> = Vec::with_capacity(value.len());
    let mut chars = value.chars();
    while let Some(c) = chars.next() {
        match c {
            '{' | '}' => return Err(CodecError::InvalidRtfEscape),
            '\\' => match chars.next() {
                Some(escaped @ ('\\' | '{' | '}')) => push_char(&mut units, escaped),
                Some('u') => units.push(parse_unit(&mut chars)?),
                _ => return Err(CodecError::InvalidRtfEscape),
            },
            _ => push_char(&mut units, c),
        }
    }
    String::from_utf16(&units).map_err(|_| CodecError::InvalidUtf16)
}
