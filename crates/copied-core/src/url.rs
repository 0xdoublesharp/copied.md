//! URL policy shared by every destination (`document.ts:13`).

use std::fmt::Write as _;
use url::Url;

const ALLOWED_SCHEMES: [&str; 3] = ["http", "https", "mailto"];

/// Whether `value` starts with a URL scheme (`/^[a-z][a-z\d+.-]*:/i`).
pub(crate) fn has_scheme_prefix(value: &str) -> bool {
    let bytes = value.as_bytes();
    if !bytes.first().is_some_and(u8::is_ascii_alphabetic) {
        return false;
    }
    for &byte in &bytes[1..] {
        if byte == b':' {
            return true;
        }
        if !(byte.is_ascii_alphanumeric() || matches!(byte, b'+' | b'.' | b'-')) {
            return false;
        }
    }
    false
}

/// Returns a destination that is safe to copy, or `None`.
///
/// Fragments are kept verbatim. Absolute `http:`, `https:`, and `mailto:` URLs keep
/// their original spelling; relative URLs resolve against `source_url` and use the
/// normalized WHATWG serialization. Every other scheme is rejected.
pub fn safe_url(value: &str, source_url: Option<&str>) -> Option<String> {
    if value.starts_with('#') {
        return Some(value.to_owned());
    }
    let parsed = match source_url {
        Some(base) => Url::parse(base).ok()?.join(value).ok()?,
        None => Url::parse(value).ok()?,
    };
    if !ALLOWED_SCHEMES.contains(&parsed.scheme()) {
        return None;
    }
    Some(if has_scheme_prefix(value) {
        value.to_owned()
    } else {
        parsed.as_str().to_owned()
    })
}

/// `micromark-util-sanitize-uri`'s `normalizeUri`: percent-encodes characters that
/// are not allowed in a URL while keeping valid escapes.
pub(crate) fn normalize_uri(value: &str) -> String {
    let mut result = String::with_capacity(value.len());
    let bytes = value.as_bytes();
    let mut index = 0;
    for (offset, ch) in value.char_indices() {
        if offset < index {
            continue;
        }
        index = offset + ch.len_utf8();
        let keep = if ch == '%' {
            bytes.get(offset + 1).is_some_and(u8::is_ascii_alphanumeric)
                && bytes.get(offset + 2).is_some_and(u8::is_ascii_alphanumeric)
        } else if ch.is_ascii() {
            matches!(ch, '!' | '#' | '$' | '&'..=';' | '=' | '?'..='Z' | '_' | 'a'..='z' | '~')
        } else {
            false
        };
        if keep {
            result.push(ch);
            if ch == '%' {
                result.push_str(&value[offset + 1..offset + 3]);
                index = offset + 3;
            }
        } else {
            let mut buffer = [0; 4];
            for byte in ch.encode_utf8(&mut buffer).bytes() {
                let _ = write!(result, "%{byte:02X}");
            }
        }
    }
    result
}

/// JavaScript `String.prototype.trim` whitespace (includes U+FEFF and line terminators).
pub(crate) fn is_js_whitespace(ch: char) -> bool {
    matches!(
        ch,
        '\u{9}' | '\u{a}' | '\u{b}' | '\u{c}' | '\u{d}' | ' ' | '\u{a0}' | '\u{1680}' | '\u{2000}'
            ..='\u{200a}'
                | '\u{2028}'
                | '\u{2029}'
                | '\u{202f}'
                | '\u{205f}'
                | '\u{3000}'
                | '\u{feff}'
    )
}

/// JavaScript `trim()`.
pub(crate) fn js_trim(value: &str) -> &str {
    value.trim_matches(is_js_whitespace)
}

/// JavaScript `trimEnd()`.
pub(crate) fn js_trim_end(value: &str) -> &str {
    value.trim_end_matches(is_js_whitespace)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn keeps_safe_absolute_urls_verbatim() {
        assert_eq!(
            safe_url("https://example.org/a b", None).as_deref(),
            Some("https://example.org/a b")
        );
        assert_eq!(
            safe_url("MAILTO:x@y.z", None).as_deref(),
            Some("MAILTO:x@y.z")
        );
        assert_eq!(safe_url("#frag", None).as_deref(), Some("#frag"));
    }

    #[test]
    fn rejects_unsafe_and_relative_without_base() {
        for value in [
            "javascript:alert(1)",
            "data:text/html,x",
            "vbscript:x",
            "file:///etc",
            "./rel",
            "",
        ] {
            assert_eq!(safe_url(value, None), None, "{value}");
        }
        assert_eq!(safe_url("./x", Some("not a url")), None);
    }

    #[test]
    fn resolves_relative_urls() {
        assert_eq!(
            safe_url("./notes", Some("https://example.org/docs/page")).as_deref(),
            Some("https://example.org/docs/notes")
        );
        assert_eq!(safe_url("javascript:x", Some("https://example.org/")), None);
    }

    #[test]
    fn normalizes_uris_like_micromark() {
        assert_eq!(
            normalize_uri("https://example.com/café?q=\"x\"&y=z"),
            "https://example.com/caf%C3%A9?q=%22x%22&y=z"
        );
        assert_eq!(normalize_uri("a%2Fb%zz %"), "a%2Fb%zz%20%25");
    }

    #[test]
    fn trims_like_javascript() {
        assert_eq!(js_trim("\u{feff} a \u{3000}"), "a");
        assert_eq!(js_trim_end(" a\u{a0}"), " a");
        assert!(has_scheme_prefix("a+b.c-d:"));
        assert!(!has_scheme_prefix("1a:"));
        assert!(!has_scheme_prefix("ab"));
        assert!(!has_scheme_prefix("a/b:"));
    }
}
