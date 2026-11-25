//! Windows `CF_HTML` clipboard envelope.
//!
//! This matches the Lean model `CopiedFormal.CfHtml`. Offsets are UTF-8 byte offsets written
//! as 10-digit, zero-padded decimals, so the header is always [`HEADER_LEN`] bytes long.

use alloc::string::String;

use crate::{CfHtmlOffsets, CodecError};

const VERSION_LINE: &str = "Version:0.9\r\n";
const HTML_OPEN: &str = "<html><body>\r\n";
const START_MARKER: &str = "<!--StartFragment-->";
const END_MARKER: &str = "<!--EndFragment-->";
const HTML_CLOSE: &str = "\r\n</body></html>";
const DIGITS: &[u8; 10] = b"0123456789";

/// Byte length of the fixed-width header that precedes `<html>`.
pub const HEADER_LEN: usize = 105;

fn push_field(out: &mut String, key: &str, value: usize) {
    out.push_str(key);
    let mut divisor: usize = 1_000_000_000;
    while divisor > 0 {
        out.push(char::from(DIGITS[value / divisor % 10]));
        divisor /= 10;
    }
    out.push_str("\r\n");
}

/// Wraps an HTML fragment in a valid Windows `CF_HTML` envelope with UTF-8 byte offsets.
///
/// Each offset field holds 10 digits, so fragments must be smaller than about 9.99 GB for
/// the offsets to be exact.
pub fn encode_cf_html(fragment: &str) -> String {
    let start_html = HEADER_LEN;
    let start_fragment = start_html + HTML_OPEN.len() + START_MARKER.len();
    let end_fragment = start_fragment + fragment.len();
    let end_html = end_fragment + END_MARKER.len() + HTML_CLOSE.len();

    let mut out = String::with_capacity(end_html);
    out.push_str(VERSION_LINE);
    push_field(&mut out, "StartHTML:", start_html);
    push_field(&mut out, "EndHTML:", end_html);
    push_field(&mut out, "StartFragment:", start_fragment);
    push_field(&mut out, "EndFragment:", end_fragment);
    out.push_str(HTML_OPEN);
    out.push_str(START_MARKER);
    out.push_str(fragment);
    out.push_str(END_MARKER);
    out.push_str(HTML_CLOSE);
    out
}

fn parse_decimal(value: &str) -> Result<usize, CodecError> {
    if value.is_empty() {
        return Err(CodecError::InvalidCfHtmlHeader);
    }
    let mut total: usize = 0;
    for byte in value.bytes() {
        if !byte.is_ascii_digit() {
            return Err(CodecError::InvalidCfHtmlHeader);
        }
        let digit = usize::from(byte - b'0');
        let Some(shifted) = total.checked_mul(10) else {
            return Err(CodecError::Overflow);
        };
        let Some(next) = shifted.checked_add(digit) else {
            return Err(CodecError::Overflow);
        };
        total = next;
    }
    Ok(total)
}

/// Header keys are non-empty ASCII alphanumerics, such as `StartHTML` or `SourceURL`.
fn is_header_key(key: &str) -> bool {
    if key.is_empty() {
        return false;
    }
    for byte in key.bytes() {
        if !byte.is_ascii_alphanumeric() {
            return false;
        }
    }
    true
}

fn field_slot(key: &str) -> Option<usize> {
    match key {
        "StartHTML" => Some(0),
        "EndHTML" => Some(1),
        "StartFragment" => Some(2),
        "EndFragment" => Some(3),
        _ => None,
    }
}

/// Parses a Windows `CF_HTML` envelope and validates byte offsets and fragment markers.
///
/// The header is a run of `Key:Value` lines starting with `Version`. Unknown keys such as
/// `SourceURL` are allowed. `StartHTML`, `EndHTML`, `StartFragment` and `EndFragment` must
/// each appear exactly once as unsigned decimals. The offsets must satisfy
/// `header end <= StartHTML <= StartFragment <= EndFragment <= EndHTML <= payload length`
/// and fall on UTF-8 character boundaries. The fragment must sit directly between
/// `<!--StartFragment-->` and `<!--EndFragment-->`.
pub fn parse_cf_html(payload: &str) -> Result<CfHtmlOffsets, CodecError> {
    let mut fields: [Option<usize>; 4] = [None; 4];
    let mut header_end = 0;
    let mut rest = payload;
    while let Some(newline) = rest.find('\n') {
        let raw = &rest[..newline];
        let line = raw.strip_suffix('\r').unwrap_or(raw);
        let Some((key, value)) = line.split_once(':') else {
            break;
        };
        if !is_header_key(key) {
            break;
        }
        if header_end == 0 && key != "Version" {
            return Err(CodecError::InvalidCfHtmlHeader);
        }
        if let Some(slot) = field_slot(key) {
            if fields[slot].is_some() {
                return Err(CodecError::InvalidCfHtmlHeader);
            }
            fields[slot] = Some(parse_decimal(value)?);
        }
        header_end += newline + 1;
        rest = &rest[newline + 1..];
    }

    let [
        Some(start_html),
        Some(end_html),
        Some(start_fragment),
        Some(end_fragment),
    ] = fields
    else {
        return Err(CodecError::InvalidCfHtmlHeader);
    };
    let ordered = header_end <= start_html
        && start_html <= start_fragment
        && start_fragment <= end_fragment
        && end_fragment <= end_html
        && end_html <= payload.len();
    if !ordered {
        return Err(CodecError::InvalidCfHtmlHeader);
    }
    for offset in [start_html, start_fragment, end_fragment, end_html] {
        if !payload.is_char_boundary(offset) {
            return Err(CodecError::InvalidCfHtmlHeader);
        }
    }
    let marked = payload[start_html..start_fragment].ends_with(START_MARKER)
        && payload[end_fragment..end_html].starts_with(END_MARKER);
    if !marked {
        return Err(CodecError::InvalidCfHtmlHeader);
    }
    Ok(CfHtmlOffsets {
        start_html,
        end_html,
        start_fragment,
        end_fragment,
    })
}
