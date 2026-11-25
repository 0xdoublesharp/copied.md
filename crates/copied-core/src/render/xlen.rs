//! X (Twitter) weighted length: `twitter-text` 3.1 `parseTweet` with the v3
//! configuration (scale 100, default weight 200, Latin ranges weight 100, URLs
//! count as 23 characters, emoji sequences count once).

use super::xlen_data::{EMOJI, TLDS};
use unicode_normalization::UnicodeNormalization;

/// Standard X post limit in weighted characters.
pub const X_LIMIT: usize = 280;
const TRANSFORMED_URL_LENGTH: usize = 23;
const SCALE: usize = 100;
const DEFAULT_WEIGHT: usize = 200;
const MAX_URL_LENGTH: usize = 4096;
const MAX_TCO_SLUG_LENGTH: usize = 40;

fn char_weight(ch: char) -> usize {
    match u32::from(ch) {
        0..=4351 | 8192..=8205 | 8208..=8223 | 8242..=8247 => 100,
        _ => DEFAULT_WEIGHT,
    }
}

fn utf16_len(chars: &[char]) -> usize {
    chars.iter().map(|c| c.len_utf16()).sum()
}

fn is_invalid_domain_char(ch: char) -> bool {
    matches!(
        ch,
        '/' | '!' | '\'' | '#' | '%' | '&' | '(' | ')' | '*' | '+' | ',' | '\\' | '-' | '.' | ':' | ';' | '<' | '='
            | '>' | '?' | '@' | '[' | ']' | '^' | '_' | '{' | '|' | '}' | '~' | '$'
            | '\u{9}'..='\u{d}' | ' ' | '\u{85}' | '\u{a0}' | '\u{1680}' | '\u{180e}' | '\u{2000}'..='\u{200a}'
            | '\u{2028}' | '\u{2029}' | '\u{202f}' | '\u{205f}' | '\u{3000}' | '\u{fffe}' | '\u{feff}' | '\u{ffff}'
            | '\u{202a}'..='\u{202e}' | '\u{61c}' | '\u{200e}' | '\u{200f}' | '\u{2066}'..='\u{2069}'
    )
}

fn is_domain_char(ch: char) -> bool {
    !is_invalid_domain_char(ch)
}

fn is_latin_accent(ch: char) -> bool {
    matches!(
        u32::from(ch),
        0xC0..=0xD6 | 0xD8..=0xF6 | 0xF8..=0xFF | 0x100..=0x24F | 0x253 | 0x254 | 0x256 | 0x257 | 0x259 | 0x25B
            | 0x263 | 0x268 | 0x26F | 0x272 | 0x289 | 0x28B | 0x2BB | 0x300..=0x36F | 0x1E00..=0x1EFF
    )
}

fn is_path_general(ch: char) -> bool {
    ch.is_ascii_alphanumeric()
        || matches!(
            ch,
            '!' | '*'
                | '\''
                | ';'
                | ':'
                | '='
                | '+'
                | ','
                | '.'
                | '$'
                | '/'
                | '%'
                | '#'
                | '['
                | ']'
                | '-'
                | '\u{2013}'
                | '_'
                | '~'
                | '@'
                | '|'
                | '&'
        )
        || ('\u{400}'..='\u{4ff}').contains(&ch)
        || is_latin_accent(ch)
}

fn is_path_ending(ch: char) -> bool {
    ch.is_ascii_alphanumeric()
        || matches!(ch, '+' | '-' | '=' | '_' | '#' | '/')
        || ('\u{400}'..='\u{4ff}').contains(&ch)
        || is_latin_accent(ch)
}

fn is_query_char(ch: char) -> bool {
    ch.is_ascii_alphanumeric()
        || matches!(
            ch,
            '!' | '?'
                | '*'
                | '\''
                | '@'
                | '('
                | ')'
                | ';'
                | ':'
                | '&'
                | '='
                | '+'
                | '$'
                | '/'
                | '%'
                | '#'
                | '['
                | ']'
                | '-'
                | '_'
                | '.'
                | ','
                | '~'
                | '|'
        )
}

fn is_query_ending(ch: char) -> bool {
    ch.is_ascii_alphanumeric() || matches!(ch, '-' | '_' | '&' | '=' | '#' | '/')
}

fn is_preceding(ch: char) -> bool {
    !(ch.is_ascii_alphanumeric()
        || matches!(
            ch,
            '@' | '＠' | '$' | '#' | '＃' | '\u{fffe}' | '\u{feff}' | '\u{ffff}'
        ))
}

fn starts_with_ci(chars: &[char], at: usize, needle: &str) -> bool {
    let count = needle.chars().count();
    chars.get(at..at + count).is_some_and(|window| {
        window
            .iter()
            .zip(needle.chars())
            .all(|(&ch, expected)| ch == expected || ch.to_lowercase().eq(expected.to_lowercase()))
    })
}

/// TLD (or punycode label) at `at`, honoring the `(?=[^0-9a-zA-Z@+-]|$)` lookahead.
fn match_tld(chars: &[char], at: usize, limit: usize) -> Option<usize> {
    let boundary = |end: usize| {
        end >= limit
            || !(chars[end].is_ascii_alphanumeric() || matches!(chars[end], '@' | '+' | '-'))
    };
    for tld in TLDS {
        let len = tld.chars().count();
        if at + len <= limit && starts_with_ci(chars, at, tld) && boundary(at + len) {
            return Some(at + len);
        }
    }
    if at + 4 <= limit && starts_with_ci(chars, at, "xn--") {
        let mut end = at + 4;
        while end < limit && (chars[end].is_ascii_alphanumeric() || chars[end] == '-') {
            end += 1;
        }
        if end > at + 4 {
            return Some(end);
        }
    }
    None
}

/// Labels separated by dots starting at `start`: (label start, label end) where
/// each label is followed by a dot.
fn dotted_labels(
    chars: &[char],
    start: usize,
    allow: impl Fn(char) -> bool,
) -> Vec<(usize, usize)> {
    let mut labels = Vec::new();
    let mut index = start;
    loop {
        let label_start = index;
        while index < chars.len() && allow(chars[index]) {
            index += 1;
        }
        if index == label_start || chars.get(index) != Some(&'.') {
            return labels;
        }
        labels.push((label_start, index));
        index += 1;
    }
}

fn valid_label(chars: &[char], (start, end): (usize, usize), extra: &[char]) -> bool {
    is_domain_char(chars[start])
        && is_domain_char(chars[end - 1])
        && chars[start..end]
            .iter()
            .all(|&c| is_domain_char(c) || extra.contains(&c))
}

/// `validDomain` at `start`; returns the domain end.
fn match_domain(chars: &[char], start: usize) -> Option<usize> {
    let labels = dotted_labels(chars, start, |c| is_domain_char(c) || c == '-' || c == '_');
    for k in (0..labels.len()).rev() {
        if !labels[..k]
            .iter()
            .all(|&label| valid_label(chars, label, &['-', '_']))
        {
            continue;
        }
        if !valid_label(chars, labels[k], &['-']) {
            continue;
        }
        if let Some(end) = match_tld(chars, labels[k].1 + 1, chars.len()) {
            return Some(end);
        }
    }
    None
}

/// Balanced parenthesis group at `at`: `\((?:G+|(?:G*\(G+\)G*))\)`.
fn match_parens(chars: &[char], at: usize) -> Option<usize> {
    if chars.get(at) != Some(&'(') {
        return None;
    }
    let general = |i: usize| chars.get(i).copied().is_some_and(is_path_general);
    let mut index = at + 1;
    let run = |mut i: usize| {
        while general(i) {
            i += 1;
        }
        i
    };
    let after_first = run(index);
    if after_first > index && chars.get(after_first) == Some(&')') {
        return Some(after_first + 1);
    }
    index = after_first;
    if chars.get(index) != Some(&'(') {
        return None;
    }
    let inner_end = run(index + 1);
    if inner_end == index + 1 || chars.get(inner_end) != Some(&')') {
        return None;
    }
    let tail = run(inner_end + 1);
    (chars.get(tail) == Some(&')')).then_some(tail + 1)
}

/// `(\/validUrlPath*)?` at `at` (which must be `/`); returns the path end.
fn match_path(chars: &[char], at: usize) -> usize {
    let mut index = at + 1;
    let mut end = index;
    loop {
        if let Some(close) = match_parens(chars, index) {
            index = close;
            end = index;
        } else if chars.get(index).copied().is_some_and(is_path_general) {
            if is_path_ending(chars[index]) {
                end = index + 1;
            }
            index += 1;
        } else {
            return end;
        }
    }
}

fn match_query(chars: &[char], at: usize) -> Option<usize> {
    if chars.get(at) != Some(&'?') {
        return None;
    }
    let mut index = at + 1;
    let mut end = None;
    while index < chars.len() && is_query_char(chars[index]) {
        if is_query_ending(chars[index]) {
            end = Some(index + 1);
        }
        index += 1;
    }
    end
}

#[derive(Debug, Clone)]
struct UrlMatch {
    url_start: usize,
    end: usize,
    protocol: bool,
    domain: (usize, usize),
    has_path: bool,
    before: Option<char>,
}

fn match_url_body(chars: &[char], start: usize, before: Option<char>) -> Option<UrlMatch> {
    let candidates: &[bool] =
        if starts_with_ci(chars, start, "https://") || starts_with_ci(chars, start, "http://") {
            &[true, false]
        } else {
            &[false]
        };
    for &protocol in candidates {
        let domain_start = if protocol {
            start
                + if starts_with_ci(chars, start, "https://") {
                    8
                } else {
                    7
                }
        } else {
            start
        };
        let Some(domain_end) = match_domain(chars, domain_start) else {
            continue;
        };
        let mut end = domain_end;
        if chars.get(end) == Some(&':') {
            let digits = chars[end + 1..]
                .iter()
                .take_while(|c| c.is_ascii_digit())
                .count();
            if digits > 0 {
                end += 1 + digits;
            }
        }
        let mut has_path = false;
        if chars.get(end) == Some(&'/') {
            end = match_path(chars, end);
            has_path = true;
        }
        if let Some(query_end) = match_query(chars, end) {
            end = query_end;
        }
        return Some(UrlMatch {
            url_start: start,
            end,
            protocol,
            domain: (domain_start, domain_end),
            has_path,
            before,
        });
    }
    None
}

fn match_url_at(chars: &[char], index: usize) -> Option<UrlMatch> {
    if let Some(&ch) = chars.get(index)
        && is_preceding(ch)
        && let Some(found) = match_url_body(chars, index + 1, Some(ch))
    {
        return Some(found);
    }
    if index == 0 {
        return match_url_body(chars, 0, None);
    }
    None
}

/// ASCII domains inside `chars[start..end]` (`validAsciiDomain`, global).
fn ascii_domains(chars: &[char], start: usize, end: usize) -> Vec<(usize, usize)> {
    let allow = |c: char| c.is_ascii_alphanumeric() || c == '-' || is_latin_accent(c);
    let mut found = Vec::new();
    let mut index = start;
    while index < end {
        let labels: Vec<(usize, usize)> = dotted_labels(&chars[..end], index, allow)
            .into_iter()
            .take_while(|&(_, e)| e < end)
            .collect();
        let matched = (0..labels.len())
            .rev()
            .find_map(|k| match_tld(chars, labels[k].1 + 1, end));
        match matched {
            Some(stop) => {
                found.push((index, stop));
                index = stop;
            }
            None => index += 1,
        }
    }
    found
}

/// t.co handling: not a t.co URL, a t.co URL with too long a slug, or its end.
enum Tco {
    No,
    Skip,
    End(usize),
}

fn tco_end(chars: &[char], m: &UrlMatch) -> Tco {
    let start = m.url_start;
    let prefix = if starts_with_ci(chars, start, "https://t.co/") {
        13
    } else if starts_with_ci(chars, start, "http://t.co/") {
        12
    } else {
        return Tco::No;
    };
    let slug_start = start + prefix;
    let slug = chars[slug_start..m.end]
        .iter()
        .take_while(|c| c.is_ascii_alphanumeric())
        .count();
    if slug == 0 {
        return Tco::No;
    }
    if slug > MAX_TCO_SLUG_LENGTH {
        return Tco::Skip;
    }
    let end = slug_start + slug;
    Tco::End(match_query(&chars[..m.end], end).unwrap_or(end))
}

/// URL entities as (start, end, url length in chars) in char indices.
fn extract_urls(chars: &[char]) -> Vec<(usize, usize)> {
    if !chars.contains(&'.') {
        return Vec::new();
    }
    let mut urls = Vec::new();
    let mut index = 0;
    while index < chars.len() {
        let Some(found) = match_url_at(chars, index) else {
            index += 1;
            continue;
        };
        index = found.end.max(index + 1);
        let protocol_len = if found.protocol {
            found.domain.0 - found.url_start
        } else {
            "https://".len()
        };
        if protocol_len + utf16_len(&chars[found.url_start..found.end]) > MAX_URL_LENGTH {
            continue;
        }
        if found.protocol {
            match tco_end(chars, &found) {
                Tco::Skip => {}
                Tco::End(end) => urls.push((found.url_start, end)),
                Tco::No => urls.push((found.url_start, found.end)),
            }
            continue;
        }
        if found
            .before
            .is_some_and(|c| matches!(c, '-' | '_' | '.' | '/'))
        {
            continue;
        }
        let domains = ascii_domains(chars, found.domain.0, found.domain.1);
        let count = domains.len();
        for (position, (start, end)) in domains.into_iter().enumerate() {
            if position + 1 == count && found.has_path {
                let rest = found.end - found.domain.1;
                urls.push((start, start + (end - start) + rest));
            } else {
                urls.push((start, end));
            }
        }
    }
    urls
}

fn emoji_at(chars: &[char], index: usize) -> Option<usize> {
    let max = (chars.len() - index).min(12);
    let mut candidate = String::new();
    let mut best = None;
    for length in 1..=max {
        candidate.push(chars[index + length - 1]);
        if EMOJI.binary_search(&candidate.as_str()).is_ok() {
            best = Some(length);
        }
    }
    best
}

fn emoji_entities(chars: &[char]) -> Vec<(usize, usize)> {
    let mut found = Vec::new();
    let mut index = 0;
    while index < chars.len() {
        match emoji_at(chars, index) {
            Some(length) => {
                found.push((index, length));
                index += length;
            }
            None => index += 1,
        }
    }
    found
}

/// Weighted length of `text` in X characters.
pub fn weighted_length(text: &str) -> usize {
    let chars: Vec<char> = text.nfc().collect();
    let urls: std::collections::HashMap<usize, usize> = extract_urls(&chars)
        .into_iter()
        .map(|(s, e)| (s, e - s))
        .collect();
    let emoji: std::collections::HashMap<usize, usize> =
        emoji_entities(&chars).into_iter().collect();
    let mut weighted = 0;
    let mut index = 0;
    while index < chars.len() {
        if let Some(&length) = urls.get(&index) {
            weighted += TRANSFORMED_URL_LENGTH * SCALE;
            index += length.max(1);
        } else if let Some(&length) = emoji.get(&index) {
            weighted += DEFAULT_WEIGHT;
            index += length;
        } else {
            weighted += char_weight(chars[index]);
            index += 1;
        }
    }
    weighted / SCALE
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn weighs_cjk_and_urls() {
        assert_eq!(
            weighted_length(&format!("{} https://example.com", "漢".repeat(140))),
            304
        );
        assert_eq!(weighted_length("hello"), 5);
        assert_eq!(weighted_length("see example.com/path."), 4 + 23 + 1);
        assert_eq!(weighted_length("x.example.com:8080/a(b)c?q=1"), 23);
        assert_eq!(weighted_length("mail hi@example.org"), 5 + 2 + 1 + 11);
        assert_eq!(weighted_length("_example.com"), 12);
        assert_eq!(weighted_length("https://t.co/abc?x=1!"), 24);
        assert_eq!(weighted_length("a.b.com.evil"), 28);
        assert_eq!(weighted_length("ftp://example.com"), 17);
        assert_eq!(weighted_length("例え.jp/x"), 9);
        assert_eq!(weighted_length("https://例え.jp/x"), 23);
        assert_eq!(weighted_length("my_sub.example.com/x?y"), 26);
        assert_eq!(weighted_length("example.com?x=1"), 27);
        assert_eq!(weighted_length("Mail (mailto:hi@example.org)"), 28);
    }

    #[test]
    fn weighs_emoji_sequences_once() {
        assert_eq!(weighted_length("👩🏽‍💻"), 2);
        assert_eq!(weighted_length("🇺🇦"), 2);
        assert_eq!(weighted_length("e\u{301}"), 1);
        assert_eq!(weighted_length("©"), 1);
        assert_eq!(weighted_length("©\u{fe0f}"), 2);
        assert_eq!(weighted_length("\u{2014}"), 1);
        assert_eq!(weighted_length("\u{fe0f}"), 2);
        assert_eq!(weighted_length("1\u{fe0f}\u{20e3}"), 2);
        assert_eq!(weighted_length("#\u{fe0f}\u{20e3} ok"), 5);
        assert_eq!(weighted_length("👍🏽👍"), 4);
    }

    #[test]
    fn matches_twitter_text_edge_cases() {
        let long_slug = format!("https://t.co/{}", "a".repeat(41));
        let huge = format!("https://example.com/{}", "a".repeat(5000));
        for (text, expected) in [
            ("a.xn--p1ai/x", 23),
            ("a_.b.com", 8),
            ("x_y.example.com", 25),
            ("http://t.co/abc", 23),
            (long_slug.as_str(), 54),
            ("https://t.co/", 23),
            ("x.com/a(b(c)d)e", 23),
            ("x.com/a(b", 25),
            ("x.com:/a", 26),
            ("x.com:80", 26),
            (huge.as_str(), 5020),
            ("go to EXAMPLE.COM now", 33),
            ("https://example.com/(x)", 23),
            ("x.com/@user/", 23),
        ] {
            assert_eq!(weighted_length(text), expected, "{text}");
        }
    }
}
