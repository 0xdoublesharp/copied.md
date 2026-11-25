//! Spoiler handling.
//!
//! * [`signal_spoiler_fallback`] ports `signal-spoilers.ts` with a hand-written
//!   tag-stack scanner. It fails closed: a crossing or unclosed spoiler omits
//!   everything from the spoiler to its matching close (or the end).
//! * For Telegram, Discord and Reddit the text renderers emit private sentinel
//!   characters exactly where the Signal renderer emits `<span>` tags.
//!   [`resolve`] then turns each spoiler region into the destination's native
//!   marker when that is structurally safe, or into `[spoiler omitted]`.
//!   Sentinel characters in the source are neutralized before parsing, so input
//!   cannot forge a region boundary.

use crate::html::dom::HElement;
use crate::html::text::{TextHooks, html_text};
use crate::url::is_js_whitespace;
use std::borrow::Cow;
use std::fmt::Write as _;

/// Signal spoiler opening tag.
pub(crate) const SPOILER_OPEN: &str = "<span data-spoiler=\"true\">";
/// Text shown in place of omitted spoiler content.
pub(crate) const PLACEHOLDER: &str = "[spoiler omitted]";

/// Sentinel for an opening spoiler span.
pub(crate) const OPEN_SPOILER: char = '\u{FDD0}';
/// Sentinel for an opening non-spoiler span.
pub(crate) const OPEN_SPAN: char = '\u{FDD1}';
/// Sentinel for a closing span.
pub(crate) const CLOSE_SPAN: char = '\u{FDD2}';

fn is_sentinel(ch: char) -> bool {
    matches!(ch, OPEN_SPOILER | OPEN_SPAN | CLOSE_SPAN)
}

/// Replaces sentinel characters in source text with U+FFFD.
pub(crate) fn neutralize(markdown: &str) -> Cow<'_, str> {
    if markdown.contains(is_sentinel) {
        Cow::Owned(markdown.chars().map(|c| if is_sentinel(c) { '\u{FFFD}' } else { c }).collect())
    } else {
        Cow::Borrowed(markdown)
    }
}

/// Whether a parsed span element is a Signal spoiler.
pub(crate) fn is_spoiler_span(element: &HElement) -> bool {
    element.attr("data-spoiler").is_some_and(|value| value.to_lowercase() == "true")
}

/// `signalSpanOpen`.
pub(crate) fn signal_span_open(element: &HElement) -> &'static str {
    if is_spoiler_span(element) { SPOILER_OPEN } else { "<span>" }
}

fn starts_with_ci(haystack: &[char], at: usize, needle: &str) -> bool {
    let mut index = at;
    for expected in needle.chars() {
        match haystack.get(index) {
            Some(ch) if ch.to_ascii_lowercase() == expected => index += 1,
            _ => return false,
        }
    }
    true
}

fn is_word(ch: char) -> bool {
    ch.is_ascii_alphanumeric() || ch == '_'
}

/// Index (in chars) where the value part of `data-spoiler=...true` matches.
fn spoiler_value_matches(chars: &[char], mut index: usize) -> bool {
    while chars.get(index).copied().is_some_and(is_js_whitespace) {
        index += 1;
    }
    if chars.get(index) != Some(&'=') {
        return false;
    }
    index += 1;
    while chars.get(index).copied().is_some_and(is_js_whitespace) {
        index += 1;
    }
    for quote in ['"', '\''] {
        if chars.get(index) == Some(&quote)
            && starts_with_ci(chars, index + 1, "true")
            && chars.get(index + 5).is_some_and(|&c| c != quote && !is_word(c))
        {
            return true;
        }
    }
    let start = if matches!(chars.get(index), Some('"' | '\'')) { index + 1 } else { index };
    if !starts_with_ci(chars, start, "true") || chars.get(start + 4).copied().is_some_and(is_word) {
        return false;
    }
    chars[start + 4..].iter().take_while(|&&c| c != '\n').all(|&c| c != '<' && c != '>')
}

/// First char index of a partial `<span … data-spoiler="true` opening in text.
///
/// Hand-written equivalent of
/// `/<span\s+[^<>\n]*\bdata-spoiler\s*=\s*(?:"true(?=[^"\w])|'true(?=[^'\w])|["']?true\b[^<>\n]*(?=\n|$))/i`.
pub(crate) fn partial_spoiler_start(text: &str) -> Option<usize> {
    if !text.contains('<') {
        return None;
    }
    let chars: Vec<char> = text.chars().collect();
    for start in 0..chars.len() {
        if !starts_with_ci(&chars, start, "<span") {
            continue;
        }
        let mut index = start + 5;
        if !chars.get(index).copied().is_some_and(is_js_whitespace) {
            continue;
        }
        while chars.get(index).copied().is_some_and(is_js_whitespace) {
            index += 1;
        }
        // `[^<>\n]*` may be empty; try every candidate position for `\bdata-spoiler`.
        let mut candidate = index;
        loop {
            let boundary = !is_word(chars[candidate - 1]);
            if boundary && starts_with_ci(&chars, candidate, "data-spoiler") && spoiler_value_matches(&chars, candidate + 12) {
                return Some(chars[..start].iter().map(|c| c.len_utf8()).sum());
            }
            match chars.get(candidate) {
                Some('<' | '>' | '\n') | None => break,
                Some(_) => candidate += 1,
            }
        }
    }
    None
}

/// Result of [`signal_spoiler_fallback`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Fallback {
    pub plain: String,
    pub malformed: bool,
}

/// Finds the next `<(\/?)(span|strong|em|s)(?: [^>]*)?>` tag at or after `from`.
fn next_tag(html: &str, from: usize) -> Option<(usize, usize, bool, &'static str)> {
    let bytes = html.as_bytes();
    let mut index = from;
    while let Some(offset) = html[index..].find('<') {
        let start = index + offset;
        let mut cursor = start + 1;
        let closing = bytes.get(cursor) == Some(&b'/');
        if closing {
            cursor += 1;
        }
        for name in ["span", "strong", "em", "s"] {
            if !html[cursor..].starts_with(name) {
                continue;
            }
            let after = cursor + name.len();
            match bytes.get(after) {
                Some(b'>') => return Some((start, after + 1, closing, name)),
                Some(b' ') => {
                    if let Some(end) = html[after..].find('>') {
                        return Some((start, after + end + 1, closing, name));
                    }
                }
                _ => {}
            }
        }
        index = start + 1;
    }
    None
}

/// `signalSpoilerFallback`: plain text without spoilers and whether any spoiler is malformed.
pub(crate) fn signal_spoiler_fallback(html: &str) -> Option<Fallback> {
    if !html.contains(SPOILER_OPEN) {
        return None;
    }
    let mut open_tags: Vec<&str> = Vec::new();
    let mut spoiler_depth = 0usize;
    let mut malformed = false;
    let mut cursor = 0;
    let mut visible = String::new();
    let mut from = 0;
    while let Some((start, end, closing, name)) = next_tag(html, from) {
        from = end;
        if closing {
            let opening = open_tags.pop();
            if spoiler_depth > 0 && opening != Some(name) {
                malformed = true;
            }
        } else {
            open_tags.push(name);
        }
        if name != "span" {
            continue;
        }
        if spoiler_depth > 0 {
            if closing {
                spoiler_depth -= 1;
            } else {
                spoiler_depth += 1;
            }
            if spoiler_depth == 0 {
                cursor = end;
            }
        } else if &html[start..end] == SPOILER_OPEN {
            visible.push_str(&html[cursor..start]);
            visible.push_str(PLACEHOLDER);
            spoiler_depth = 1;
        }
    }
    if spoiler_depth == 0 {
        visible.push_str(&html[cursor..]);
    }
    Some(Fallback { plain: html_text(&visible, TextHooks::default()), malformed: malformed || spoiler_depth > 0 })
}

/// Output surface a sentinel stream was rendered for.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Surface {
    Plain,
    Rtf,
    TelegramHtml,
    Discord,
    Reddit,
}

/// Result of [`resolve`].
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub(crate) struct Resolved {
    pub text: String,
    /// Spoiler regions found.
    pub regions: usize,
    /// Regions replaced by the placeholder.
    pub omitted: usize,
}

fn inside_code_fence(before: &str) -> bool {
    let mut open = false;
    for line in before.split('\n') {
        let trimmed = line.trim_start_matches(['>', ' ', '-', '*', '+', '.', '0', '1', '2', '3', '4', '5', '6', '7', '8', '9']);
        if trimmed.starts_with("```") || trimmed.starts_with("~~~") {
            open = !open;
        }
    }
    open
}

/// HTML tags in `content`: (unmatched closes, unmatched opens).
fn html_balance(content: &str) -> (Vec<String>, Vec<String>) {
    let mut opens: Vec<String> = Vec::new();
    let mut closes = Vec::new();
    let mut rest = content;
    while let Some(start) = rest.find('<') {
        let Some(end) = rest[start..].find('>') else { break };
        let tag = &rest[start + 1..start + end];
        rest = &rest[start + end + 1..];
        let (closing, body) = tag.strip_prefix('/').map_or((false, tag), |b| (true, b));
        let name: String = body.chars().take_while(|c| c.is_ascii_alphanumeric() || *c == '-').collect();
        if name.is_empty() || name == "br" {
            continue;
        }
        if closing {
            if opens.last() == Some(&name) {
                opens.pop();
            } else {
                closes.push(name);
            }
        } else {
            opens.push(name);
        }
    }
    (closes, opens)
}

/// RTF groups in `content`: (unmatched closes, opener prefixes of unmatched groups).
fn rtf_balance(content: &str) -> (usize, Vec<String>) {
    let chars: Vec<char> = content.chars().collect();
    let mut opens: Vec<String> = Vec::new();
    let mut closes = 0;
    let mut index = 0;
    while index < chars.len() {
        match chars[index] {
            '\\' => index += 2,
            '{' => {
                let mut prefix = String::from("{");
                index += 1;
                while chars.get(index) == Some(&'\\') && chars.get(index + 1).is_some_and(|c| c.is_ascii_alphabetic() || *c == '*') {
                    prefix.push('\\');
                    index += 1;
                    while chars.get(index).is_some_and(|c| c.is_ascii_alphanumeric() || *c == '*' || *c == '-') {
                        prefix.push(chars[index]);
                        index += 1;
                    }
                    if chars.get(index) == Some(&' ') {
                        prefix.push(' ');
                        index += 1;
                        break;
                    }
                }
                opens.push(prefix);
            }
            '}' => {
                if opens.pop().is_none() {
                    closes += 1;
                }
                index += 1;
            }
            _ => index += 1,
        }
    }
    (closes, opens)
}

fn placeholder(surface: Surface, content: &str) -> String {
    match surface {
        Surface::Rtf => {
            let (closes, opens) = rtf_balance(content);
            format!("{PLACEHOLDER}{}{}", "}".repeat(closes), opens.concat())
        }
        Surface::TelegramHtml => {
            let (closes, opens) = html_balance(content);
            let mut out = String::from(PLACEHOLDER);
            for name in closes {
                let _ = write!(out, "</{name}>");
            }
            for name in opens {
                let _ = write!(out, "<{name}>");
            }
            out
        }
        Surface::Plain | Surface::Discord | Surface::Reddit => PLACEHOLDER.to_owned(),
    }
}

fn native(surface: Surface, before: &str, content: &str) -> Option<String> {
    match surface {
        Surface::TelegramHtml => {
            let (closes, opens) = html_balance(content);
            let blocky = ["<pre", "</pre", "<blockquote", "</blockquote"].iter().any(|t| content.contains(t));
            (closes.is_empty() && opens.is_empty() && !blocky).then(|| format!("<tg-spoiler>{content}</tg-spoiler>"))
        }
        Surface::Discord => (!content.is_empty()
            && !content.contains("||")
            && !content.contains("```")
            && !inside_code_fence(before))
        .then(|| format!("||{content}||")),
        Surface::Reddit => {
            let unescaped_pipe = content.char_indices().any(|(i, c)| c == '|' && !content[..i].ends_with('\\'));
            (!content.is_empty()
                && !content.contains('\n')
                && !content.contains("!<")
                && !content.contains(">!")
                && !unescaped_pipe
                && !inside_code_fence(before))
            .then(|| format!(">!{content}!<"))
        }
        Surface::Plain | Surface::Rtf => None,
    }
}

/// Resolves sentinel-marked spoiler regions in rendered output.
///
/// With `allow_native` false (a malformed spoiler anywhere), every region becomes
/// the placeholder, and an unclosed region omits everything after it.
pub(crate) fn resolve(rendered: &str, surface: Surface, allow_native: bool) -> Resolved {
    let mut out = String::with_capacity(rendered.len());
    let mut resolved = Resolved::default();
    let mut depth = 0usize;
    let mut content = String::new();
    for ch in rendered.chars() {
        if depth == 0 {
            match ch {
                OPEN_SPOILER => {
                    depth = 1;
                    content.clear();
                }
                OPEN_SPAN | CLOSE_SPAN => {}
                _ => out.push(ch),
            }
            continue;
        }
        match ch {
            OPEN_SPOILER | OPEN_SPAN => depth += 1,
            CLOSE_SPAN => {
                depth -= 1;
                if depth == 0 {
                    resolved.regions += 1;
                    let replacement = allow_native.then(|| native(surface, &out, &content)).flatten();
                    if let Some(native) = replacement {
                        out.push_str(&native);
                    } else {
                        resolved.omitted += 1;
                        out.push_str(&placeholder(surface, &content));
                    }
                }
            }
            _ => content.push(ch),
        }
    }
    if depth > 0 {
        resolved.regions += 1;
        resolved.omitted += 1;
        out.push_str(&placeholder(surface, &content));
    }
    resolved.text = out;
    resolved
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detects_partial_spoilers_like_the_regex() {
        assert_eq!(partial_spoiler_start("Before <span data-spoiler=\"true\" private text"), Some(7));
        assert_eq!(partial_spoiler_start("x <span class=x data-spoiler=\"true>private"), Some(2));
        assert_eq!(partial_spoiler_start("<SPAN DATA-SPOILER='TRUE' x"), Some(0));
        assert_eq!(partial_spoiler_start("<span data-spoiler=true more"), Some(0));
        assert_eq!(partial_spoiler_start("<span data-spoiler=true <b>"), None);
        assert_eq!(partial_spoiler_start("<span data-spoiler=\"true\""), Some(0));
        assert_eq!(partial_spoiler_start("<span xdata-spoiler=true"), None);
        assert_eq!(partial_spoiler_start("<span data-spoiler=\"false\" x"), None);
        assert_eq!(partial_spoiler_start("<spandata-spoiler=true"), None);
        assert_eq!(partial_spoiler_start("é <span\n data-spoiler = 'true' z"), Some(3));
    }

    #[test]
    fn signal_fallback_fails_closed() {
        let valid = signal_spoiler_fallback("a <span data-spoiler=\"true\"><strong>x</strong></span> b").unwrap();
        assert_eq!(valid, Fallback { plain: "a [spoiler omitted] b".into(), malformed: false });
        let crossing = signal_spoiler_fallback("<strong><span data-spoiler=\"true\">p</strong> t</span> z").unwrap();
        assert_eq!(crossing, Fallback { plain: "[spoiler omitted] z".into(), malformed: true });
        let open = signal_spoiler_fallback("a <span data-spoiler=\"true\">p<br>q").unwrap();
        assert_eq!(open, Fallback { plain: "a [spoiler omitted]".into(), malformed: true });
        assert_eq!(signal_spoiler_fallback("<span>x</span>"), None);
        assert_eq!(next_tag("<sup><s x>", 0), Some((5, 10, false, "s")));
    }

    #[test]
    fn resolves_native_and_placeholder_regions() {
        let stream = format!("a {OPEN_SPOILER}x{OPEN_SPAN}y{CLOSE_SPAN}{CLOSE_SPAN} b {OPEN_SPAN}c{CLOSE_SPAN}");
        assert_eq!(resolve(&stream, Surface::Discord, true).text, "a ||xy|| b c");
        assert_eq!(resolve(&stream, Surface::Reddit, true).text, "a >!xy!< b c");
        assert_eq!(resolve(&stream, Surface::Plain, true).text, "a [spoiler omitted] b c");
        let tg = format!("<strong>a {OPEN_SPOILER}b</strong> <a href=\"u\">c{CLOSE_SPAN}d</a>");
        assert_eq!(resolve(&tg, Surface::TelegramHtml, true).text, "<strong>a [spoiler omitted]</strong><a>d</a>");
        let rtf = format!("{{\\b a}} {OPEN_SPOILER}{{\\field{{\\*\\fldinst HYPERLINK \"u\"}}{{\\fldrslt la{CLOSE_SPAN}bel}}}}");
        assert_eq!(resolve(&rtf, Surface::Rtf, true).text, "{\\b a} [spoiler omitted]{\\field{\\fldrslt bel}}");
        let unclosed = format!("a {OPEN_SPOILER}secret\n\nmore");
        let resolved = resolve(&unclosed, Surface::Discord, true);
        assert_eq!((resolved.text.as_str(), resolved.omitted), ("a [spoiler omitted]", 1));
        let fenced = format!("```\n{OPEN_SPOILER}x{CLOSE_SPAN}\n```");
        assert_eq!(resolve(&fenced, Surface::Discord, true).text, "```\n[spoiler omitted]\n```");
        assert_eq!(resolve(&format!("{OPEN_SPOILER}a||b{CLOSE_SPAN}"), Surface::Discord, true).omitted, 1);
        assert_eq!(resolve(&format!("{OPEN_SPOILER}a!<b{CLOSE_SPAN}"), Surface::Reddit, true).omitted, 1);
        assert_eq!(resolve(&format!("{OPEN_SPOILER}x{CLOSE_SPAN}"), Surface::Discord, false).text, PLACEHOLDER);
        assert_eq!(neutralize("a\u{FDD0}b"), "a\u{FFFD}b");
    }
}
