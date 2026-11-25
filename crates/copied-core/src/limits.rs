//! Resource guards that keep conversion bounded in native and WASM builds.
//!
//! Inputs over [`MAX_INPUT_BYTES`], with an estimated nesting depth over
//! [`MAX_NESTING_DEPTH`], or with more unmatched-bracket work than
//! [`MAX_LABEL_STARTS`] are not parsed as Markdown. They are copied as literal
//! text instead and the conversion reports
//! [`DiagnosticCode::InputLimitExceeded`](crate::DiagnosticCode::InputLimitExceeded).

/// Largest Markdown source that is parsed (2 MiB).
pub const MAX_INPUT_BYTES: usize = 2 * 1024 * 1024;
/// Deepest Markdown container or phrasing nesting that is parsed.
pub const MAX_NESTING_DEPTH: usize = 128;
/// Deepest HTML element nesting kept from raw HTML; deeper elements are flattened.
pub const MAX_HTML_DEPTH: usize = 256;
/// Most `[` characters accepted; `markdown-rs` label matching is quadratic in them.
pub const MAX_LABEL_STARTS: usize = 10_000;

/// Why an input was not parsed as Markdown.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Limit {
    Bytes,
    Depth,
    Labels,
}

/// Checks the cheap pre-parse limits.
pub(crate) fn check(markdown: &str) -> Option<Limit> {
    if markdown.len() > MAX_INPUT_BYTES {
        return Some(Limit::Bytes);
    }
    if markdown.bytes().filter(|&b| b == b'[').count() > MAX_LABEL_STARTS {
        return Some(Limit::Labels);
    }
    if estimated_depth(markdown) > MAX_NESTING_DEPTH {
        return Some(Limit::Depth);
    }
    None
}

/// Conservative estimate of mdast nesting depth.
///
/// Block containers are counted per line from their markers (`>`, list markers,
/// and the indentation that nests list items). Phrasing nesting is estimated per
/// paragraph by counting delimiter characters that can still open emphasis.
pub(crate) fn estimated_depth(markdown: &str) -> usize {
    let mut max = 0;
    let mut open = [0usize; 3];
    for line in markdown.split('\n') {
        if line.trim().is_empty() {
            open = [0; 3];
            continue;
        }
        let (block, rest) = block_prefix(line);
        inline_open(rest, &mut open, &mut max, block);
        max = max.max(block);
    }
    max
}

fn block_prefix(line: &str) -> (usize, &str) {
    let bytes = line.as_bytes();
    let mut index = 0;
    let mut quotes = 0;
    let mut markers = 0;
    let mut indent = 0;
    loop {
        match bytes.get(index) {
            Some(b' ') => {
                indent += 1;
                index += 1;
            }
            Some(b'\t') => {
                indent += 4;
                index += 1;
            }
            Some(b'>') => {
                quotes += 1;
                index += 1;
            }
            Some(b'-' | b'+' | b'*')
                if matches!(bytes.get(index + 1), None | Some(b' ' | b'\t')) =>
            {
                markers += 1;
                index += 1;
            }
            Some(b'0'..=b'9') => {
                let start = index;
                while bytes.get(index).is_some_and(u8::is_ascii_digit) && index - start < 10 {
                    index += 1;
                }
                if matches!(bytes.get(index), Some(b'.' | b')'))
                    && matches!(bytes.get(index + 1), None | Some(b' ' | b'\t'))
                {
                    markers += 1;
                    index += 1;
                } else {
                    index = start;
                    break;
                }
            }
            _ => break,
        }
    }
    let nested_by_indent = if markers > 0 { indent } else { 0 };
    // Root, table structure, and footnote definitions add a small constant.
    (4 + quotes + 2 * markers + nested_by_indent, &line[index..])
}

/// Counts delimiter characters that may still be open, per delimiter kind. A
/// run only cancels openers of its own character, and only when it can close.
fn inline_open(text: &str, open: &mut [usize; 3], max: &mut usize, block: usize) {
    let chars: Vec<char> = text.chars().collect();
    let mut index = 0;
    while index < chars.len() {
        let ch = chars[index];
        let kind = match ch {
            '*' => 0,
            '_' => 1,
            '~' => 2,
            _ => {
                index += 1;
                continue;
            }
        };
        let start = index;
        while index < chars.len() && chars[index] == ch {
            index += 1;
        }
        let run = index - start;
        let before = start.checked_sub(1).map(|i| chars[i]);
        let after = chars.get(index).copied();
        let left = after.is_some_and(|c| !c.is_whitespace());
        let right = before.is_some_and(|c| !c.is_whitespace());
        let intraword =
            before.is_some_and(char::is_alphanumeric) && after.is_some_and(char::is_alphanumeric);
        if ch == '_' && intraword {
            continue;
        }
        let closes = right && (!left || open[kind] > 0);
        if closes {
            open[kind] = open[kind].saturating_sub(run);
        } else if left {
            open[kind] += run;
        }
        *max = (*max).max(block + open.iter().sum::<usize>());
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ordinary_documents_pass() {
        let doc =
            "# Title\n\n- a\n  - b\n    - c\n\n> quote **bold** _it_ snake_case_name\n\n2*3*4*5";
        assert_eq!(check(doc), None);
        assert!(estimated_depth(doc) < 20);
    }

    #[test]
    fn deep_inputs_are_rejected() {
        assert_eq!(check(&">".repeat(500)), Some(Limit::Depth));
        let list: String = (0..200).fold(String::new(), |acc, i| acc + &"  ".repeat(i) + "- a\n");
        assert_eq!(check(&list), Some(Limit::Depth));
        assert_eq!(
            check(&format!("{}a", "*a ".repeat(300))),
            Some(Limit::Depth)
        );
        assert_eq!(
            check(&"[".repeat(MAX_LABEL_STARTS + 1)),
            Some(Limit::Labels)
        );
        assert_eq!(check(&"a".repeat(MAX_INPUT_BYTES + 1)), Some(Limit::Bytes));
        assert_eq!(check("1234567890123. x"), None);
    }
}
