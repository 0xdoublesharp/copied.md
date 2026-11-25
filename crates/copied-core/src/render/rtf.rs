//! RTF helpers for the Telegram RTF representation.

/// Escapes text for RTF via the Lean-verified `copied-codec` UTF-16 escaper.
pub(crate) fn escape_rtf_utf16(s: &str) -> String {
    copied_codec::rtf::escape_rtf(s)
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
