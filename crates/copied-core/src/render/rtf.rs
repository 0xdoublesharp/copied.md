//! RTF helpers for the Telegram RTF representation.

use std::fmt::Write as _;

/// Escapes text for RTF (`render-text.ts:18`): `\`, `{`, `}` get a backslash and
/// every UTF-16 code unit outside printable ASCII, tab, and newline becomes a
/// signed `\uN?` escape.
///
/// The coordinator swaps this for `copied_codec::rtf::escape_rtf` on merge.
pub(crate) fn escape_rtf_utf16(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for ch in s.chars() {
        match ch {
            '\\' | '{' | '}' => {
                out.push('\\');
                out.push(ch);
            }
            '\t' | '\n' | ' '..='~' => out.push(ch),
            _ => {
                let mut units = [0u16; 2];
                for &unit in ch.encode_utf16(&mut units).iter() {
                    let signed = i32::from(unit) - if unit < 0x8000 { 0 } else { 0x10000 };
                    let _ = write!(out, "\\u{signed}?");
                }
            }
        }
    }
    out
}

/// Wraps rendered RTF text in the Telegram document header.
pub(crate) fn document(text: &str) -> String {
    // Telegram treats a tab followed by 0 as an automatic list marker.
    let body = text
        .replace("\t0", "    0")
        .replace('\n', "\\par\n")
        .replace('\t', "\\tab ");
    format!(
        "{{\\rtf1\\ansi\\deff0{{\\fonttbl{{\\f0\\fswiss Helvetica;}}{{\\f1\\fmodern Courier;}}}}\\uc1\\f0 {body}}}"
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn escapes_utf16_units() {
        assert_eq!(
            escape_rtf_utf16("Café ☕ 🚀 {x} \\b"),
            "Caf\\u233? \\u9749? \\u-10179?\\u-8576? \\{x\\} \\\\b"
        );
        assert_eq!(escape_rtf_utf16("\r\u{7f}"), "\\u13?\\u127?");
    }

    #[test]
    fn wraps_documents() {
        assert_eq!(
            document("a\n\t0\tb"),
            "{\\rtf1\\ansi\\deff0{\\fonttbl{\\f0\\fswiss Helvetica;}{\\f1\\fmodern Courier;}}\\uc1\\f0 a\\par\n    0\\tab b}"
        );
    }
}
