//! Discord and Reddit Markdown (`render-markdown.ts`): a port of
//! `mdast-util-to-markdown` (with GFM tables, strikethrough, footnotes, task
//! lists and autolink-literal escaping) configured with `bullet: '-'`,
//! `emphasis: '*'`, `fences: true` and `listItemIndent: 'one'`.

use super::spoilers::{
    CLOSE_SPAN, OPEN_OMITTED, OPEN_SPAN, OPEN_SPOILER, is_spoiler_span, partial_spoiler_start,
};
use super::text::{TextOptions, longest_streak, render_text};
use crate::ast::{Align, Node};
use crate::document::footnote_numbers;
use crate::html::dom::HElement;
use crate::html::tables::align_rows;
use crate::html::text::{TextHooks, html_text};
use crate::url::{is_js_whitespace, js_trim_end};
use std::collections::HashMap;

/// Markdown dialect.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Flavor {
    Discord,
    Reddit,
}

/// Rendering switches for Markdown targets.
#[derive(Debug, Clone, Copy)]
pub(crate) struct MarkdownOptions {
    pub flavor: Flavor,
    pub improved: bool,
    pub spoilers: bool,
}

// ---------------------------------------------------------------------------
// Tree preparation (the `visit` pass in `renderMarkdown`).

struct Prepare<'a> {
    opts: MarkdownOptions,
    numbers: &'a HashMap<String, usize>,
}

fn sub_text_options(opts: MarkdownOptions) -> TextOptions {
    TextOptions {
        improved: opts.improved,
        spoilers: opts.spoilers,
        ..TextOptions::default()
    }
}

impl Prepare<'_> {
    fn html_value(&self, value: &str) -> String {
        if !self.opts.spoilers {
            return html_text(value, TextHooks::default());
        }
        let trimmed = crate::url::js_trim(value);
        if trimmed.len() >= 7
            && trimmed.is_char_boundary(6)
            && trimmed[..6].eq_ignore_ascii_case("</span")
            && trimmed.ends_with('>')
            && trimmed[6..trimmed.len() - 1].chars().all(is_js_whitespace)
        {
            return CLOSE_SPAN.to_string();
        }
        let span = |element: &HElement, content: &str| {
            let open = if is_spoiler_span(element) {
                OPEN_SPOILER
            } else {
                OPEN_SPAN
            };
            let close = if element.explicit_close {
                CLOSE_SPAN.to_string()
            } else {
                String::new()
            };
            format!("{open}{content}{close}")
        };
        html_text(
            value,
            TextHooks {
                span: Some(&span),
                ..TextHooks::default()
            },
        )
    }

    /// Prepares a flow container's children (root, quote, item).
    fn flow(&self, nodes: Vec<Node>) -> Vec<Node> {
        if !self.opts.improved {
            return self.nodes(nodes);
        }
        // Keep raw HTML blocks as their own paragraphs. The oracle turned them
        // into bare text, which makes the root phrasing and glues every block
        // together (and can leave a code fence unclosed).
        let mut out = Vec::with_capacity(nodes.len());
        for node in nodes {
            match node {
                Node::Html(value) => {
                    let text = js_trim_end(&self.html_value(&value)).to_owned();
                    if !text.is_empty() {
                        out.push(Node::Paragraph(vec![Node::Text(text)]));
                    }
                }
                other => out.extend(self.nodes(vec![other])),
            }
        }
        out
    }

    fn nodes(&self, nodes: Vec<Node>) -> Vec<Node> {
        let mut out = Vec::with_capacity(nodes.len());
        for node in nodes {
            match node {
                Node::Html(value) => out.push(Node::Text(self.html_value(&value))),
                Node::Text(value) if self.opts.spoilers => match partial_spoiler_start(&value) {
                    Some(start) => {
                        out.push(Node::Text(format!("{}{OPEN_SPOILER}", &value[..start])));
                    }
                    None => out.push(Node::Text(value)),
                },
                Node::FootnoteReference { identifier, .. } => {
                    let number = self
                        .numbers
                        .get(&identifier)
                        .map_or_else(|| "undefined".to_owned(), ToString::to_string);
                    out.push(Node::Text(format!("[{number}]")));
                }
                Node::FootnoteDefinition {
                    identifier,
                    children,
                } => {
                    if let Some(number) = self.numbers.get(&identifier) {
                        let note = render_text(&children, sub_text_options(self.opts));
                        out.push(Node::Paragraph(vec![Node::Text(format!(
                            "[{number}] {note}"
                        ))]));
                    }
                }
                Node::Heading { depth, children } => {
                    let depth = if self.opts.flavor == Flavor::Discord {
                        depth.min(3)
                    } else {
                        depth
                    };
                    out.push(Node::Heading {
                        depth,
                        children: self.nodes(children),
                    });
                }
                Node::Table { align, children } if self.opts.flavor == Flavor::Discord => {
                    let table = Node::Table { align, children };
                    let value = if self.opts.improved {
                        aligned_table(&table, self.opts)
                    } else {
                        render_text(std::slice::from_ref(&table), sub_text_options(self.opts))
                    };
                    // A spoiler marker inside a code block would show literally.
                    let value = value.replace(OPEN_SPOILER, &OPEN_OMITTED.to_string());
                    out.push(Node::Code {
                        value,
                        lang: None,
                        meta: None,
                    });
                }
                Node::Link {
                    url,
                    title,
                    children,
                } if self.opts.flavor == Flavor::Discord => {
                    let children = self.nodes(images_to_text(children));
                    out.push(Node::Link {
                        url,
                        title,
                        children,
                    });
                }
                Node::Image { url, alt, .. } if self.opts.flavor == Flavor::Discord => {
                    let label = if alt.is_empty() {
                        "Image".to_owned()
                    } else {
                        alt
                    };
                    out.push(Node::Link {
                        url,
                        title: None,
                        children: vec![Node::Text(label)],
                    });
                }
                mut other => {
                    let flow = matches!(other, Node::Blockquote(_) | Node::ListItem { .. });
                    if let Some(children) = other.children_mut() {
                        let taken = std::mem::take(children);
                        *children = if flow {
                            self.flow(taken)
                        } else {
                            self.nodes(taken)
                        };
                    }
                    out.push(other);
                }
            }
        }
        out
    }
}

fn images_to_text(nodes: Vec<Node>) -> Vec<Node> {
    nodes
        .into_iter()
        .map(|node| match node {
            Node::Image { alt, .. } => Node::Text(if alt.is_empty() {
                "Image".to_owned()
            } else {
                alt
            }),
            mut other => {
                if let Some(children) = other.children_mut() {
                    let taken = std::mem::take(children);
                    *children = images_to_text(taken);
                }
                other
            }
        })
        .collect()
}

/// Aligned monospace text for a GFM table (cells as plain text, no pipe escapes).
fn aligned_table(table: &Node, opts: MarkdownOptions) -> String {
    let Node::Table { children, .. } = table else {
        return String::new();
    };
    let rows: Vec<Vec<String>> = children
        .iter()
        .map(|row| {
            row.children()
                .map(|cells| {
                    cells
                        .iter()
                        .map(|cell| {
                            let paragraph =
                                Node::Paragraph(cell.children().cloned().unwrap_or_default());
                            render_text(std::slice::from_ref(&paragraph), sub_text_options(opts))
                        })
                        .collect()
                })
                .unwrap_or_default()
        })
        .collect();
    align_rows(&rows, true).join("\n")
}

// ---------------------------------------------------------------------------
// Serializer.

const FULL_PHRASING_SPANS: &[&str] = &[
    "autolink",
    "destinationLiteral",
    "destinationRaw",
    "reference",
    "titleQuote",
    "titleApostrophe",
];
const AUTOLINK_NOT: &[&str] = &["autolink", "link", "image", "label"];

#[derive(Debug, Clone, Copy)]
enum Cond {
    LineEnding,
    LineEndingOrEnd,
    Digits,
    CloseBracket,
    OpenBracket,
    Class(&'static str),
    Letters(&'static str),
    WordOr(&'static str),
    Slash,
}

impl Cond {
    /// Length (in chars) matched at `index`, if any.
    fn matches(self, chars: &[char], index: usize) -> Option<usize> {
        let ch = chars.get(index).copied();
        match self {
            Self::LineEnding => ch.filter(|c| matches!(c, '\r' | '\n')).map(|_| 1),
            Self::LineEndingOrEnd => match ch {
                None => Some(0),
                Some('\r' | '\n') => Some(1),
                _ => None,
            },
            Self::Digits => {
                let count = chars[index.min(chars.len())..]
                    .iter()
                    .take_while(|c| c.is_ascii_digit())
                    .count();
                (count > 0).then_some(count)
            }
            Self::CloseBracket => (ch == Some(']')).then_some(1),
            Self::OpenBracket => (ch == Some('[')).then_some(1),
            Self::Class(set) => ch.filter(|c| set.contains(*c)).map(|_| 1),
            Self::Letters(extra) => ch
                .filter(|c| c.is_ascii_alphabetic() || extra.contains(*c))
                .map(|_| 1),
            Self::WordOr(extra) => ch
                .filter(|c| c.is_ascii_alphanumeric() || *c == '_' || extra.contains(*c))
                .map(|_| 1),
            Self::Slash => (ch == Some('/')).then_some(1),
        }
    }
}

#[derive(Debug, Clone, Copy)]
struct Unsafe {
    ch: char,
    at_break: bool,
    before: Option<Cond>,
    after: Option<Cond>,
    in_construct: &'static [&'static str],
    not_in_construct: &'static [&'static str],
}

const fn u(ch: char) -> Unsafe {
    Unsafe {
        ch,
        at_break: false,
        before: None,
        after: None,
        in_construct: &[],
        not_in_construct: &[],
    }
}

const fn brk(ch: char) -> Unsafe {
    Unsafe {
        at_break: true,
        ..u(ch)
    }
}

const PHRASING: &[&str] = &["phrasing"];
const CODE_LANG: &[&str] = &["codeFencedLangGraveAccent", "codeFencedLangTilde"];
const EOL_CONSTRUCTS: &[&str] = &[
    "codeFencedLangGraveAccent",
    "codeFencedLangTilde",
    "codeFencedMetaGraveAccent",
    "codeFencedMetaTilde",
    "destinationLiteral",
    "headingAtx",
];

const UNSAFE: &[Unsafe] = &[
    Unsafe {
        after: Some(Cond::LineEnding),
        in_construct: PHRASING,
        ..u('\t')
    },
    Unsafe {
        before: Some(Cond::LineEnding),
        in_construct: PHRASING,
        ..u('\t')
    },
    Unsafe {
        in_construct: CODE_LANG,
        ..u('\t')
    },
    Unsafe {
        in_construct: EOL_CONSTRUCTS,
        ..u('\r')
    },
    Unsafe {
        in_construct: EOL_CONSTRUCTS,
        ..u('\n')
    },
    Unsafe {
        after: Some(Cond::LineEnding),
        in_construct: PHRASING,
        ..u(' ')
    },
    Unsafe {
        before: Some(Cond::LineEnding),
        in_construct: PHRASING,
        ..u(' ')
    },
    Unsafe {
        in_construct: CODE_LANG,
        ..u(' ')
    },
    Unsafe {
        after: Some(Cond::OpenBracket),
        in_construct: PHRASING,
        not_in_construct: FULL_PHRASING_SPANS,
        ..u('!')
    },
    Unsafe {
        in_construct: &["titleQuote"],
        ..u('"')
    },
    brk('#'),
    Unsafe {
        in_construct: &["headingAtx"],
        after: Some(Cond::LineEndingOrEnd),
        ..u('#')
    },
    Unsafe {
        after: Some(Cond::Letters("#")),
        in_construct: PHRASING,
        ..u('&')
    },
    Unsafe {
        in_construct: &["titleApostrophe"],
        ..u('\'')
    },
    Unsafe {
        in_construct: &["destinationRaw"],
        ..u('(')
    },
    Unsafe {
        before: Some(Cond::CloseBracket),
        in_construct: PHRASING,
        not_in_construct: FULL_PHRASING_SPANS,
        ..u('(')
    },
    Unsafe {
        before: Some(Cond::Digits),
        ..brk(')')
    },
    Unsafe {
        in_construct: &["destinationRaw"],
        ..u(')')
    },
    Unsafe {
        after: Some(Cond::Class(" \t\r\n*")),
        ..brk('*')
    },
    Unsafe {
        in_construct: PHRASING,
        not_in_construct: FULL_PHRASING_SPANS,
        ..u('*')
    },
    Unsafe {
        after: Some(Cond::Class(" \t\r\n")),
        ..brk('+')
    },
    Unsafe {
        after: Some(Cond::Class(" \t\r\n-")),
        ..brk('-')
    },
    Unsafe {
        before: Some(Cond::Digits),
        after: Some(Cond::Class(" \t\r\n")),
        ..brk('.')
    },
    Unsafe {
        before: Some(Cond::Digits),
        after: Some(Cond::LineEndingOrEnd),
        ..brk('.')
    },
    Unsafe {
        after: Some(Cond::Letters("!/?")),
        ..brk('<')
    },
    Unsafe {
        after: Some(Cond::Letters("!/?")),
        in_construct: PHRASING,
        not_in_construct: FULL_PHRASING_SPANS,
        ..u('<')
    },
    Unsafe {
        in_construct: &["destinationLiteral"],
        ..u('<')
    },
    brk('='),
    brk('>'),
    Unsafe {
        in_construct: &["destinationLiteral"],
        ..u('>')
    },
    brk('['),
    Unsafe {
        in_construct: PHRASING,
        not_in_construct: FULL_PHRASING_SPANS,
        ..u('[')
    },
    Unsafe {
        in_construct: &["label", "reference"],
        ..u('[')
    },
    Unsafe {
        after: Some(Cond::LineEnding),
        in_construct: PHRASING,
        ..u('\\')
    },
    Unsafe {
        in_construct: &["label", "reference"],
        ..u(']')
    },
    brk('_'),
    Unsafe {
        in_construct: PHRASING,
        not_in_construct: FULL_PHRASING_SPANS,
        ..u('_')
    },
    brk('`'),
    Unsafe {
        in_construct: &["codeFencedLangGraveAccent", "codeFencedMetaGraveAccent"],
        ..u('`')
    },
    Unsafe {
        in_construct: PHRASING,
        not_in_construct: FULL_PHRASING_SPANS,
        ..u('`')
    },
    brk('~'),
    // GFM strikethrough.
    Unsafe {
        in_construct: PHRASING,
        not_in_construct: FULL_PHRASING_SPANS,
        ..u('~')
    },
    // GFM table.
    Unsafe {
        in_construct: &["tableCell"],
        ..u('\r')
    },
    Unsafe {
        in_construct: &["tableCell"],
        ..u('\n')
    },
    Unsafe {
        after: Some(Cond::Class("\t :-")),
        ..brk('|')
    },
    Unsafe {
        in_construct: &["tableCell"],
        ..u('|')
    },
    Unsafe {
        after: Some(Cond::Class("-")),
        ..brk(':')
    },
    Unsafe {
        after: Some(Cond::Class(":|-")),
        ..brk('-')
    },
    // GFM footnote.
    Unsafe {
        in_construct: &["label", "phrasing", "reference"],
        ..u('[')
    },
    // GFM autolink literal.
    Unsafe {
        before: Some(Cond::WordOr("+-.")),
        after: Some(Cond::WordOr("-.")),
        in_construct: PHRASING,
        not_in_construct: AUTOLINK_NOT,
        ..u('@')
    },
    Unsafe {
        before: Some(Cond::Class("Ww")),
        after: Some(Cond::WordOr("-.")),
        in_construct: PHRASING,
        not_in_construct: AUTOLINK_NOT,
        ..u('.')
    },
    Unsafe {
        before: Some(Cond::Class("ps")),
        after: Some(Cond::Slash),
        in_construct: PHRASING,
        not_in_construct: AUTOLINK_NOT,
        ..u(':')
    },
];

impl Unsafe {
    fn in_scope(&self, stack: &[&str]) -> bool {
        let inside =
            self.in_construct.is_empty() || self.in_construct.iter().any(|c| stack.contains(c));
        let excluded = self.not_in_construct.iter().any(|c| stack.contains(c));
        inside && !excluded
    }

    /// Tries to match at `start`; returns (position of the character, match end).
    fn match_at(&self, chars: &[char], start: usize) -> Option<(usize, usize)> {
        let mut index = start;
        if self.at_break {
            if !matches!(chars.get(index), Some('\r' | '\n')) {
                return None;
            }
            index += 1;
            while matches!(chars.get(index), Some('\t' | ' ')) {
                index += 1;
            }
        }
        if let Some(before) = self.before {
            index += before.matches(chars, index)?;
        }
        if chars.get(index) != Some(&self.ch) {
            return None;
        }
        let position = index;
        index += 1;
        if let Some(after) = self.after {
            index += after.matches(chars, index)?;
        }
        Some((position, index))
    }

    fn has_before(&self) -> bool {
        self.before.is_some() || self.at_break
    }
}

fn is_ascii_punctuation(ch: char) -> bool {
    ch.is_ascii_punctuation()
}

fn escape_backslashes(value: &[char], after: &str) -> String {
    let after_first = after.chars().next();
    let mut out = String::with_capacity(value.len());
    for (index, &ch) in value.iter().enumerate() {
        if ch == '\\' {
            let next = value.get(index + 1).copied().or(after_first);
            if next.is_some_and(is_ascii_punctuation) {
                out.push('\\');
            }
        }
        out.push(ch);
    }
    out
}

fn char_ref(ch: char) -> String {
    format!("&#x{:X};", u32::from(ch))
}

/// `micromark-util-classify-character`: `None` letter, `Some(1)` whitespace, `Some(2)` punctuation.
fn classify(ch: Option<char>) -> Option<u8> {
    match ch {
        None => Some(1),
        Some(c) if c.is_whitespace() || is_js_whitespace(c) => Some(1),
        Some(c) if is_unicode_punctuation(c) => Some(2),
        Some(_) => None,
    }
}

fn is_unicode_punctuation(c: char) -> bool {
    c.is_ascii_punctuation()
        || matches!(u32::from(c),
            0xA1..=0xBF | 0xD7 | 0xF7 | 0x2010..=0x2027 | 0x2030..=0x205E | 0x2070..=0x2BFF | 0x3001..=0x303F
            | 0xFE10..=0xFE6F | 0xFF01..=0xFF0F | 0xFF1A..=0xFF20 | 0xFF3B..=0xFF40 | 0xFF5B..=0xFF65
            | 0x1F000..=0x1FAFF)
}

/// `encodeInfo`: (encode inside, encode outside).
fn encode_info(outside: Option<char>, inside: Option<char>, marker: char) -> (bool, bool) {
    let outside_kind = classify(outside);
    let inside_kind = classify(inside);
    match outside_kind {
        None => match inside_kind {
            None => {
                let both = marker == '_';
                (both, both)
            }
            Some(1) => (true, true),
            _ => (false, true),
        },
        Some(1) => (inside_kind == Some(1), inside_kind == Some(1)),
        _ => (inside_kind == Some(1), false),
    }
}

fn indent_lines(value: &str, map: impl Fn(&str, usize, bool) -> String) -> String {
    let mut out = String::with_capacity(value.len());
    let mut line = 0;
    let mut rest = value;
    while let Some(i) = rest.find(['\r', '\n']) {
        let len = if rest[i..].starts_with("\r\n") { 2 } else { 1 };
        out.push_str(&map(&rest[..i], line, rest[..i].is_empty()));
        out.push_str(&rest[i..i + len]);
        rest = &rest[i + len..];
        line += 1;
    }
    out.push_str(&map(rest, line, rest.is_empty()));
    out
}

fn to_string(nodes: &[Node]) -> String {
    let mut out = String::new();
    for node in nodes {
        match node {
            Node::Text(v) | Node::InlineCode(v) | Node::Html(v) => out.push_str(v),
            Node::Code { value, .. } => out.push_str(value),
            Node::Image { alt, .. } => out.push_str(alt),
            other => {
                if let Some(children) = other.children() {
                    out.push_str(&to_string(children));
                }
            }
        }
    }
    out
}

fn has_literal_break(nodes: &[Node]) -> bool {
    nodes.iter().any(|node| match node {
        Node::Break => true,
        Node::Text(v) | Node::InlineCode(v) | Node::Html(v) => v.contains(['\r', '\n']),
        other => other.children().is_some_and(|c| has_literal_break(c)),
    })
}

fn has_block_code(nodes: &[Node]) -> bool {
    nodes.iter().any(|node| {
        matches!(node, Node::Code { .. }) || node.children().is_some_and(|c| has_block_code(c))
    })
}

#[derive(Debug, Clone, Copy, Default)]
struct Info<'a> {
    before: &'a str,
    after: &'a str,
}

#[derive(Default)]
struct Serializer {
    stack: Vec<&'static str>,
    index_stack: Vec<usize>,
    bullet_current: Option<char>,
    bullet_last_used: Option<char>,
    encode_surrounding: Option<(bool, bool)>,
    clipboard: bool,
    indented_code: bool,
}

impl Serializer {
    fn enter(&mut self, name: &'static str) {
        self.stack.push(name);
    }

    fn exit(&mut self) {
        self.stack.pop();
    }

    fn safe(&self, input: &str, before: &str, after: &str, encode: &[char]) -> String {
        let value: Vec<char> = before
            .chars()
            .chain(input.chars())
            .chain(after.chars())
            .collect();
        let mut positions: Vec<usize> = Vec::new();
        let mut infos: HashMap<usize, (bool, bool)> = HashMap::new();
        for pattern in UNSAFE {
            if !pattern.in_scope(&self.stack) {
                continue;
            }
            let mut from = 0;
            while from < value.len() {
                let Some((start, position, end)) = (from..value.len())
                    .find_map(|s| pattern.match_at(&value, s).map(|(p, e)| (s, p, e)))
                else {
                    break;
                };
                let _ = start;
                let has_before = pattern.has_before();
                let has_after = pattern.after.is_some();
                if let Some(info) = infos.get_mut(&position) {
                    if info.0 && !has_before {
                        info.0 = false;
                    }
                    if info.1 && !has_after {
                        info.1 = false;
                    }
                } else {
                    positions.push(position);
                    infos.insert(position, (has_before, has_after));
                }
                from = end;
            }
        }
        positions.sort_unstable();
        let before_len = before.chars().count();
        let end = value.len() - after.chars().count();
        let mut start = before_len;
        let mut result = String::new();
        for (index, &position) in positions.iter().enumerate() {
            if position < start || position >= end {
                continue;
            }
            let info = infos[&position];
            let next_escaped = position + 1 < end
                && positions.get(index + 1) == Some(&(position + 1))
                && info.1
                && infos.get(&(position + 1)).is_some_and(|n| !n.0 && !n.1);
            let previous_escaped = index > 0
                && positions[index - 1] + 1 == position
                && info.0
                && infos.get(&(position - 1)).is_some_and(|p| !p.0 && !p.1);
            if next_escaped || previous_escaped {
                continue;
            }
            if start != position {
                result.push_str(&escape_backslashes(&value[start..position], "\\"));
            }
            start = position;
            let ch = value[position];
            if is_ascii_punctuation(ch) && !encode.contains(&ch) {
                result.push('\\');
            } else {
                result.push_str(&char_ref(ch));
                start += 1;
            }
        }
        result.push_str(&escape_backslashes(&value[start..end], after));
        result
    }

    /// `root`: html nodes turned into text make the whole root phrasing, which
    /// joins every block flush (a `mdast-util-to-markdown` quirk kept for parity).
    fn root(&mut self, nodes: &[Node]) -> String {
        let phrasing = nodes.iter().any(|node| {
            matches!(
                node,
                Node::Break
                    | Node::Delete(_)
                    | Node::Emphasis(_)
                    | Node::FootnoteReference { .. }
                    | Node::Image { .. }
                    | Node::ImageReference { .. }
                    | Node::InlineCode(_)
                    | Node::Link { .. }
                    | Node::LinkReference { .. }
                    | Node::Strong(_)
                    | Node::Text(_)
            )
        });
        if phrasing {
            self.container_phrasing(
                nodes,
                Info {
                    before: "\n",
                    after: "\n",
                },
            )
        } else {
            self.container_flow(nodes, None)
        }
    }

    fn container_flow(&mut self, nodes: &[Node], parent_spread: Option<bool>) -> String {
        let mut out = String::new();
        self.index_stack.push(0);
        for (index, node) in nodes.iter().enumerate() {
            if let Some(last) = self.index_stack.last_mut() {
                *last = index;
            }
            out.push_str(&self.handle(
                node,
                None,
                Info {
                    before: "\n",
                    after: "\n",
                },
            ));
            if !matches!(node, Node::List { .. }) {
                self.bullet_last_used = None;
            }
            if index + 1 < nodes.len() {
                let next = &nodes[index + 1];
                if self.indented_code && matches!(next, Node::Code { .. }) {
                    if matches!(node, Node::List { .. } | Node::Code { .. }) {
                        // A blank line alone leaves this code in the preceding list
                        // or code block. A thematic break works in both Reddit parsers.
                        out.push_str("\n\n***\n\n");
                    } else {
                        out.push_str("\n\n");
                    }
                } else {
                    out.push_str(&between(node, next, parent_spread));
                }
            }
        }
        self.index_stack.pop();
        out
    }

    fn container_phrasing(&mut self, nodes: &[Node], info: Info<'_>) -> String {
        let mut results: Vec<String> = Vec::with_capacity(nodes.len());
        let mut before: String = info.before.to_owned();
        let mut encode_after: Option<String> = None;
        self.index_stack.push(0);
        for (index, node) in nodes.iter().enumerate() {
            if let Some(last) = self.index_stack.last_mut() {
                *last = index;
            }
            let after: String = match nodes.get(index + 1) {
                Some(next) => self.peek(next),
                None => info.after.to_owned(),
            };
            let mut value = self.handle(
                node,
                None,
                Info {
                    before: &before,
                    after: &after,
                },
            );
            if let Some(encode) = &encode_after
                && value.starts_with(encode.as_str())
                && let Some(ch) = encode.chars().next()
            {
                value = format!("{}{}", char_ref(ch), &value[ch.len_utf8()..]);
            }
            let surrounding = self.encode_surrounding.take();
            encode_after = None;
            if let Some((encode_before, encode_after_flag)) = surrounding {
                if encode_before
                    && let Some(last) = results.last_mut()
                    && let Some(ch) = before.chars().next()
                    && last.ends_with(ch)
                {
                    last.truncate(last.len() - ch.len_utf8());
                    last.push_str(&char_ref(ch));
                }
                if encode_after_flag {
                    encode_after = Some(after.clone());
                }
            }
            before = value.chars().last().map(String::from).unwrap_or_default();
            results.push(value);
        }
        self.index_stack.pop();
        results.concat()
    }

    fn peek(&mut self, node: &Node) -> String {
        match node {
            Node::Emphasis(_) | Node::Strong(_) => "*".to_owned(),
            Node::InlineCode(_) => "`".to_owned(),
            Node::Delete(_) => "~".to_owned(),
            Node::Image { .. } => "!".to_owned(),
            Node::Link {
                url,
                title,
                children,
            } => if format_as_autolink(url, title.as_deref(), children) {
                "<"
            } else {
                "["
            }
            .to_owned(),
            other => {
                let value = self.handle(other, None, Info::default());
                value.chars().next().map(String::from).unwrap_or_default()
            }
        }
    }

    #[allow(clippy::too_many_lines)]
    fn handle(&mut self, node: &Node, parent: Option<&Node>, info: Info<'_>) -> String {
        match node {
            Node::Paragraph(children) => {
                self.enter("paragraph");
                self.enter("phrasing");
                let value = self.container_phrasing(children, info);
                self.exit();
                self.exit();
                value
            }
            Node::Heading { depth, children } => self.heading(*depth, children),
            Node::ThematicBreak => "***".to_owned(),
            Node::Blockquote(children) => {
                self.enter("blockquote");
                let value = self.container_flow(children, None);
                self.exit();
                indent_lines(&value, |line, _, blank| {
                    if blank {
                        ">".to_owned()
                    } else {
                        format!("> {line}")
                    }
                })
            }
            Node::List {
                ordered, children, ..
            } => self.list(node, *ordered, children),
            Node::ListItem { .. } => self.list_item(node, parent),
            Node::Code { value, lang, meta } => self.code(value, lang.as_deref(), meta.as_deref()),
            Node::Html(value) => value.clone(),
            Node::Text(value) => self.safe(value, info.before, info.after, &[]),
            Node::Emphasis(children) => self.attention(children, info, '*', 1, "emphasis"),
            Node::Strong(children) => self.attention(children, info, '*', 2, "strong"),
            Node::Delete(children) => {
                self.enter("strikethrough");
                let value = self.container_phrasing(
                    children,
                    Info {
                        before: "~~",
                        after: "~",
                    },
                );
                self.exit();
                format!("~~{value}~~")
            }
            Node::InlineCode(value) => self.inline_code(value),
            Node::Break => {
                let in_scope = UNSAFE
                    .iter()
                    .any(|p| p.ch == '\n' && p.in_scope(&self.stack));
                if in_scope {
                    if info.before.contains([' ', '\t']) {
                        String::new()
                    } else {
                        " ".to_owned()
                    }
                } else {
                    "\\\n".to_owned()
                }
            }
            Node::Link {
                url,
                title,
                children,
            } => self.link(url, title.as_deref(), children),
            Node::Image { url, title, alt } => {
                self.enter("image");
                self.enter("label");
                let mut value = String::from("![");
                value += &self.safe(alt, &value.clone(), "]", &[]);
                value += "](";
                self.exit();
                value += &self.destination(url, title.as_deref(), &value.clone());
                self.exit();
                value
            }
            Node::Table { align, children } => self.table(align, children),
            // References and footnotes are resolved before serialization; rows and
            // cells are handled by `table`.
            Node::LinkReference { .. }
            | Node::ImageReference { .. }
            | Node::TableRow(_)
            | Node::TableCell(_)
            | Node::Definition { .. }
            | Node::FootnoteDefinition { .. }
            | Node::FootnoteReference { .. } => String::new(),
        }
    }

    fn heading(&mut self, depth: u8, children: &[Node]) -> String {
        let rank = depth.clamp(1, 6);
        if depth < 3 && !to_string(children).is_empty() && has_literal_break(children) {
            self.enter("headingSetext");
            self.enter("phrasing");
            let value = self.container_phrasing(
                children,
                Info {
                    before: "\n",
                    after: "\n",
                },
            );
            self.exit();
            self.exit();
            let last_line = value.rsplit(['\r', '\n']).next().unwrap_or("");
            let underline =
                if rank == 1 { "=" } else { "-" }.repeat(last_line.encode_utf16().count());
            return format!("{value}\n{underline}");
        }
        let sequence = "#".repeat(usize::from(rank));
        self.enter("headingAtx");
        self.enter("phrasing");
        let mut value = self.container_phrasing(
            children,
            Info {
                before: "# ",
                after: "\n",
            },
        );
        if let Some(first) = value.chars().next()
            && matches!(first, '\t' | ' ')
        {
            value = format!("{}{}", char_ref(first), &value[1..]);
        }
        self.exit();
        self.exit();
        if value.is_empty() {
            sequence
        } else {
            format!("{sequence} {value}")
        }
    }

    fn attention(
        &mut self,
        children: &[Node],
        info: Info<'_>,
        marker: char,
        count: usize,
        name: &'static str,
    ) -> String {
        self.enter(name);
        let markers: String = std::iter::repeat_n(marker, count).collect();
        let mut between = self.container_phrasing(
            children,
            Info {
                before: &markers,
                after: &marker.to_string(),
            },
        );
        let head = between.chars().next();
        let (open_inside, open_outside) = encode_info(info.before.chars().last(), head, marker);
        if open_inside && let Some(head) = head {
            between = format!("{}{}", char_ref(head), &between[head.len_utf8()..]);
        }
        let tail = between.chars().last();
        let (close_inside, close_outside) = encode_info(info.after.chars().next(), tail, marker);
        if close_inside && let Some(tail) = tail {
            between = format!(
                "{}{}",
                &between[..between.len() - tail.len_utf8()],
                char_ref(tail)
            );
        }
        self.exit();
        self.encode_surrounding = Some((open_outside, close_outside));
        format!("{markers}{between}{markers}")
    }

    fn inline_code(&self, value: &str) -> String {
        let mut value = value.to_owned();
        // A fence one longer than any run of exactly that many grave accents.
        let mut runs = std::collections::HashSet::new();
        let mut run = 0;
        for c in value.chars().chain(std::iter::once('\0')) {
            if c == '`' {
                run += 1;
            } else if run > 0 {
                runs.insert(run);
                run = 0;
            }
        }
        let mut size = 1;
        while runs.contains(&size) {
            size += 1;
        }
        let sequence = "`".repeat(size);
        let has_content = value.chars().any(|c| !matches!(c, ' ' | '\r' | '\n'));
        let starts_blank = value.starts_with([' ', '\r', '\n']);
        let ends_blank = value.ends_with([' ', '\r', '\n']);
        if has_content
            && ((starts_blank && ends_blank) || value.starts_with('`') || value.ends_with('`'))
        {
            value = format!(" {value} ");
        }
        // Replace line endings that would start blocks with spaces.
        let mut chars: Vec<char> = value.chars().collect();
        for pattern in UNSAFE.iter().filter(|p| p.at_break) {
            let mut from = 0;
            while from < chars.len() {
                let Some((start, end)) = (from..chars.len())
                    .find_map(|s| pattern.match_at(&chars, s).map(|(_, e)| (s, e)))
                else {
                    break;
                };
                let mut position = start;
                if chars[position] == '\n' && position > 0 && chars[position - 1] == '\r' {
                    position -= 1;
                }
                chars.splice(position..=start, [' ']);
                from = end - (start - position);
            }
        }
        let value: String = chars.into_iter().collect();
        let mut out = format!("{sequence}{value}{sequence}");
        if self.stack.contains(&"tableCell") {
            out = out.replace('|', "\\|");
        }
        out
    }

    fn code(&mut self, raw: &str, lang: Option<&str>, meta: Option<&str>) -> String {
        if self.indented_code {
            return indent_lines(raw, |line, _, blank| {
                if blank {
                    String::new()
                } else {
                    format!("    {line}")
                }
            });
        }
        let sequence = "`".repeat((longest_streak(raw, '`') + 1).max(3));
        self.enter("codeFenced");
        let mut value = sequence.clone();
        if let Some(lang) = lang.filter(|l| !l.is_empty()) {
            self.enter("codeFencedLangGraveAccent");
            value += &self.safe(lang, &value.clone(), " ", &['`']);
            self.exit();
            if let Some(meta) = meta.filter(|m| !m.is_empty()) {
                self.enter("codeFencedMetaGraveAccent");
                value.push(' ');
                value += &self.safe(meta, &value.clone(), "\n", &['`']);
                self.exit();
            }
        }
        value.push('\n');
        if !raw.is_empty() {
            value.push_str(raw);
            if !self.clipboard || !raw.ends_with('\n') {
                value.push('\n');
            }
        }
        value.push_str(&sequence);
        self.exit();
        value
    }

    fn list(&mut self, node: &Node, ordered: bool, items: &[Node]) -> String {
        self.enter("list");
        let bullet_current = self.bullet_current;
        let mut bullet = if ordered { '.' } else { '-' };
        let bullet_other = if ordered { ')' } else { '*' };
        // Every list has a parent (root at least), so a repeated bullet switches.
        let mut use_other = self.bullet_last_used == Some(bullet);
        if !ordered {
            let first_empty = matches!(items.first(), Some(Node::ListItem { children, .. }) if children.is_empty());
            let n = self.stack.len();
            let m = self.index_stack.len();
            if first_empty
                && n >= 4
                && self.stack[n - 1] == "list"
                && self.stack[n - 2] == "listItem"
                && self.stack[n - 3] == "list"
                && self.stack[n - 4] == "listItem"
                && m >= 3
                && self.index_stack[m - 1] == 0
                && self.index_stack[m - 2] == 0
                && self.index_stack[m - 3] == 0
            {
                use_other = true;
            }
        }
        if use_other {
            bullet = bullet_other;
        }
        self.bullet_current = Some(bullet);
        let spread = matches!(node, Node::List { spread: true, .. });
        let value = self.container_flow_items(items, node, spread);
        self.bullet_last_used = Some(bullet);
        self.bullet_current = bullet_current;
        self.exit();
        value
    }

    fn container_flow_items(&mut self, items: &[Node], parent: &Node, spread: bool) -> String {
        let mut out = String::new();
        self.index_stack.push(0);
        for (index, item) in items.iter().enumerate() {
            if let Some(last) = self.index_stack.last_mut() {
                *last = index;
            }
            out.push_str(&self.handle(
                item,
                Some(parent),
                Info {
                    before: "\n",
                    after: "\n",
                },
            ));
            self.bullet_last_used = None;
            if index + 1 < items.len() {
                out.push_str(&between(item, &items[index + 1], Some(spread)));
            }
        }
        self.index_stack.pop();
        out
    }

    fn list_item(&mut self, node: &Node, parent: Option<&Node>) -> String {
        let Node::ListItem {
            spread,
            checked,
            children,
        } = node
        else {
            return String::new();
        };
        let mut bullet = self.bullet_current.unwrap_or('-').to_string();
        if let Some(Node::List {
            ordered: true,
            start,
            ..
        }) = parent
        {
            let index = self.index_stack.last().copied().unwrap_or(0);
            bullet = format!("{}{bullet}", u64::from(start.unwrap_or(1)) + index as u64);
        }
        let size = bullet.chars().count() + 1;
        // Old Reddit removes up to four continuation spaces from list lines.
        // Align the item content at that column before adding the code indent.
        let padding = if self.indented_code && has_block_code(children) {
            size.max(4)
        } else {
            size
        };
        self.enter("listItem");
        let mut flow = self.container_flow(children, Some(*spread));
        self.exit();
        if self.indented_code
            && matches!(children.first(), Some(Node::Code { .. }))
            && !flow.is_empty()
        {
            // Old Reddit requires a blank line within the item to parse its blocks.
            // A visually empty first paragraph separates the list marker from the code.
            flow.insert_str(0, "&nbsp;\n\n");
        }
        let indent = " ".repeat(padding);
        let marker = format!("{bullet}{}", " ".repeat(padding - size + 1));
        let mut value = indent_lines(&flow, |line, index, blank| {
            if index > 0 {
                if blank {
                    line.to_owned()
                } else {
                    format!("{indent}{line}")
                }
            } else if blank {
                format!("{bullet}{line}")
            } else {
                format!("{marker}{line}")
            }
        });
        let checkable = checked.is_some() && matches!(children.first(), Some(Node::Paragraph(_)));
        if checkable {
            let checkbox = if *checked == Some(true) {
                "[x] "
            } else {
                "[ ] "
            };
            value = insert_checkbox(&value, checkbox);
        }
        value
    }

    fn link(&mut self, url: &str, title: Option<&str>, children: &[Node]) -> String {
        if format_as_autolink(url, title, children) {
            let stack = std::mem::take(&mut self.stack);
            self.enter("autolink");
            let inner = self.container_phrasing(
                children,
                Info {
                    before: "<",
                    after: ">",
                },
            );
            self.stack = stack;
            return format!("<{inner}>");
        }
        self.enter("link");
        self.enter("label");
        let mut value = String::from("[");
        value += &self.container_phrasing(
            children,
            Info {
                before: "[",
                after: "](",
            },
        );
        value += "](";
        self.exit();
        value += &self.destination(url, title, &value.clone());
        self.exit();
        value
    }

    fn destination(&mut self, url: &str, title: Option<&str>, before: &str) -> String {
        let mut value = String::new();
        let literal =
            (url.is_empty() && title.is_some()) || url.chars().any(|c| c <= ' ' || c == '\u{7f}');
        if literal {
            self.enter("destinationLiteral");
            value.push('<');
            value += &self.safe(url, &format!("{before}<"), ">", &[]);
            value.push('>');
        } else {
            self.enter("destinationRaw");
            value += &self.safe(url, before, if title.is_some() { " " } else { ")" }, &[]);
        }
        self.exit();
        if let Some(title) = title {
            self.enter("titleQuote");
            value += " \"";
            value += &self.safe(title, &format!("{before}{value}"), "\"", &[]);
            value.push('"');
            self.exit();
        }
        value.push(')');
        value
    }

    fn table(&mut self, align: &[Align], rows: &[Node]) -> String {
        self.enter("table");
        let mut matrix: Vec<Vec<String>> = Vec::with_capacity(rows.len());
        for row in rows {
            self.enter("tableRow");
            let cells = row.children().map_or(&[][..], Vec::as_slice);
            let mut values = Vec::with_capacity(cells.len());
            for cell in cells {
                self.enter("tableCell");
                self.enter("phrasing");
                let children = cell.children().map_or(&[][..], Vec::as_slice);
                values.push(self.container_phrasing(
                    children,
                    Info {
                        before: " ",
                        after: " ",
                    },
                ));
                self.exit();
                self.exit();
            }
            self.exit();
            matrix.push(values);
        }
        self.exit();
        if self.clipboard {
            compact_table(&matrix, align)
        } else {
            markdown_table(&matrix, align)
        }
    }
}

fn insert_checkbox(value: &str, checkbox: &str) -> String {
    // `/^(?:[*+-]|\d+\.)([\r\n]| {1,3})/`
    let bytes = value.as_bytes();
    let mut index = 0;
    if matches!(bytes.first(), Some(b'*' | b'+' | b'-')) {
        index = 1;
    } else {
        while bytes.get(index).is_some_and(u8::is_ascii_digit) {
            index += 1;
        }
        if index == 0 || bytes.get(index) != Some(&b'.') {
            return value.to_owned();
        }
        index += 1;
    }
    match bytes.get(index) {
        Some(b'\r' | b'\n') => index += 1,
        Some(b' ') => {
            let spaces = bytes[index..]
                .iter()
                .take(3)
                .take_while(|&&b| b == b' ')
                .count();
            index += spaces;
        }
        _ => return value.to_owned(),
    }
    format!("{}{checkbox}{}", &value[..index], &value[index..])
}

fn format_as_autolink(url: &str, title: Option<&str>, children: &[Node]) -> bool {
    let raw = to_string(children);
    let scheme = {
        let bytes = url.as_bytes();
        let mut index = 0;
        if bytes.first().is_some_and(u8::is_ascii_alphabetic) {
            index = 1;
            while bytes
                .get(index)
                .is_some_and(|b| b.is_ascii_alphabetic() || matches!(b, b'+' | b'.' | b'-'))
            {
                index += 1;
            }
        }
        index >= 2 && bytes.get(index) == Some(&b':')
    };
    !url.is_empty()
        && title.is_none()
        && children.len() == 1
        && matches!(children[0], Node::Text(_))
        && (raw == url || format!("mailto:{raw}") == url)
        && scheme
        && !url
            .chars()
            .any(|c| c <= ' ' || c == '<' || c == '>' || c == '\u{7f}')
}

fn between(left: &Node, right: &Node, parent_spread: Option<bool>) -> String {
    if let Some(spread) = parent_spread {
        let left_paragraph = matches!(left, Node::Paragraph(_));
        let right_ok = matches!(right, Node::Paragraph(_) | Node::Definition { .. })
            || matches!(right, Node::Heading { depth, children } if *depth < 3 && !to_string(children).is_empty() && has_literal_break(children));
        if left_paragraph && right_ok {
            return "\n\n".to_owned();
        }
        return if spread {
            "\n\n".to_owned()
        } else {
            "\n".to_owned()
        };
    }
    "\n\n".to_owned()
}

fn markdown_table(matrix: &[Vec<String>], align: &[Align]) -> String {
    let columns = matrix.iter().map(Vec::len).max().unwrap_or(0);
    let width = |s: &str| s.encode_utf16().count();
    let mut longest = vec![0usize; columns];
    for row in matrix {
        for (index, cell) in row.iter().enumerate() {
            longest[index] = longest[index].max(width(cell));
        }
    }
    let alignments: Vec<Align> = (0..columns)
        .map(|i| align.get(i).copied().unwrap_or(Align::None))
        .collect();
    let mut delimiter = Vec::with_capacity(columns);
    for (index, alignment) in alignments.iter().enumerate() {
        let (before, after) = match alignment {
            Align::Center => (":", ":"),
            Align::Left => (":", ""),
            Align::Right => ("", ":"),
            Align::None => ("", ""),
        };
        let size = longest[index]
            .saturating_sub(before.len() + after.len())
            .max(1);
        let cell = format!("{before}{}{after}", "-".repeat(size));
        longest[index] = longest[index].max(cell.len());
        delimiter.push(cell);
    }
    let mut rows: Vec<&[String]> = matrix.iter().map(Vec::as_slice).collect();
    let delimiter_row = delimiter;
    rows.insert(rows.len().min(1), &delimiter_row);
    let mut lines = Vec::with_capacity(rows.len());
    for row in rows {
        let mut line = String::new();
        for column in 0..columns {
            let cell = row.get(column).map_or("", String::as_str);
            let size = longest[column] - width(cell);
            let (before, after) = match alignments[column] {
                Align::Right => (size, 0),
                Align::Center => (size / 2 + size % 2, size / 2),
                _ => (0, size),
            };
            if column == 0 {
                line.push('|');
            }
            line.push(' ');
            line.push_str(&" ".repeat(before));
            line.push_str(cell);
            line.push_str(&" ".repeat(after));
            line.push(' ');
            line.push('|');
        }
        lines.push(line);
    }
    lines.join("\n")
}

fn compact_table(matrix: &[Vec<String>], align: &[Align]) -> String {
    fn cell(table: &mut String, value: &str) {
        if !value.is_empty() {
            table.push(' ');
            table.push_str(value);
        }
        table.push_str(" |");
    }

    let Some((header, rows)) = matrix.split_first() else {
        return String::new();
    };
    let mut table = String::from("|");
    for column in 0..align.len() {
        cell(&mut table, header.get(column).map_or("", String::as_str));
    }
    table.push_str("\n|");
    for alignment in align {
        cell(
            &mut table,
            match alignment {
                Align::None => "---",
                Align::Left => ":---",
                Align::Right => "---:",
                Align::Center => ":---:",
            },
        );
    }
    for row in rows {
        table.push_str("\n|");
        for value in row {
            cell(&mut table, value);
        }
    }
    table
}

pub(crate) fn serialize_clipboard_markdown(nodes: &[Node]) -> String {
    Serializer {
        clipboard: true,
        ..Serializer::default()
    }
    .root(nodes)
}

/// `renderMarkdown`.
pub(crate) fn render_markdown(children: &[Node], opts: MarkdownOptions) -> String {
    let numbers = footnote_numbers(children);
    let prepared = Prepare {
        opts,
        numbers: &numbers,
    }
    .flow(children.to_vec());
    let mut serializer = Serializer {
        indented_code: opts.improved && opts.flavor == Flavor::Reddit,
        ..Serializer::default()
    };
    let output = serializer.root(&prepared);
    if serializer.indented_code {
        output.trim_end_matches(['\r', '\n']).to_owned()
    } else {
        js_trim_end(&output).to_owned()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::document::read_markdown;

    fn md(source: &str, flavor: Flavor) -> String {
        render_markdown(
            &read_markdown(source, None).children,
            MarkdownOptions {
                flavor,
                improved: false,
                spoilers: false,
            },
        )
    }

    #[test]
    fn escapes_like_remark_stringify() {
        assert_eq!(
            md("a \\* b \\_c\\_ \\[x] 1\\. \\# h", Flavor::Reddit),
            "a \\* b \\_c\\_ \\[x] 1. # h"
        );
        assert_eq!(md("\\# h", Flavor::Reddit), "\\# h");
        assert_eq!(
            md("[a](https://x.test/?a=1&b=2)", Flavor::Reddit),
            "[a](https://x.test/?a=1\\&b=2)"
        );
        assert_eq!(md("<https://x.test>", Flavor::Reddit), "<https://x.test>");
    }

    #[test]
    fn serializes_blocks() {
        assert_eq!(
            md("3. a\n   - b\n4. c", Flavor::Reddit),
            "3. a\n   - b\n4. c"
        );
        assert_eq!(
            md("- [x] done\n- [ ] open", Flavor::Reddit),
            "- [x] done\n- [ ] open"
        );
        assert_eq!(md("> q\n>\n> r", Flavor::Reddit), "> q\n>\n> r");
        assert_eq!(md("~~~\n```\n~~~", Flavor::Reddit), "````\n```\n````");
        assert_eq!(md("#### h", Flavor::Discord), "### h");
        assert_eq!(md("a  \nb", Flavor::Reddit), "a\\\nb");
        assert_eq!(md("---", Flavor::Reddit), "***");
        assert_eq!(
            md("| a | b |\n|:-|-:|\n| ccc | d |", Flavor::Reddit),
            "| a   |  b |\n| :-- | -: |\n| ccc |  d |"
        );
    }

    #[test]
    fn encodes_attention_edges() {
        assert_eq!(encode_info(Some('a'), Some(' '), '*'), (true, true));
        assert_eq!(encode_info(Some(' '), Some('a'), '*'), (false, false));
        assert_eq!(encode_info(Some('.'), Some(' '), '*'), (true, false));
        assert_eq!(encode_info(Some('a'), Some('b'), '_'), (true, true));
        assert_eq!(encode_info(Some('a'), Some('.'), '*'), (false, true));
    }

    fn serialize(nodes: &[Node]) -> String {
        let mut serializer = Serializer::default();
        serializer.root(nodes)
    }

    fn text(value: &str) -> Node {
        Node::Text(value.to_owned())
    }

    #[test]
    #[allow(clippy::too_many_lines)]
    fn serializes_constructed_edge_cases() {
        // A hard break inside a table cell becomes a space.
        let cell = Node::TableCell(vec![text("a"), Node::Break, text("b")]);
        let spaced = Node::TableCell(vec![text("c "), Node::Break, text("d")]);
        let table = Node::Table {
            align: vec![Align::None, Align::None],
            children: vec![Node::TableRow(vec![cell, spaced])],
        };
        assert_eq!(serialize(&[table]), "| a b | c d |\n| --- | --- |");
        // Setext headings for multi-line content, and the paragraph join rule.
        let heading = Node::Heading {
            depth: 1,
            children: vec![text("a"), Node::Break, text("bc")],
        };
        assert_eq!(serialize(std::slice::from_ref(&heading)), "a\\\nbc\n==");
        let item = Node::ListItem {
            spread: false,
            checked: None,
            children: vec![Node::Paragraph(vec![text("p")]), heading],
        };
        let list = Node::List {
            ordered: false,
            start: None,
            spread: false,
            children: vec![item],
        };
        assert_eq!(serialize(&[list]), "- p\n\n  a\\\n  bc\n  ==");
        // An empty first item nested two lists deep switches bullets.
        let empty = Node::ListItem {
            spread: false,
            checked: None,
            children: vec![],
        };
        let inner = Node::List {
            ordered: false,
            start: None,
            spread: false,
            children: vec![empty],
        };
        let middle = Node::List {
            ordered: false,
            start: None,
            spread: false,
            children: vec![Node::ListItem {
                spread: false,
                checked: None,
                children: vec![inner],
            }],
        };
        let outer = Node::List {
            ordered: false,
            start: None,
            spread: false,
            children: vec![Node::ListItem {
                spread: false,
                checked: None,
                children: vec![middle],
            }],
        };
        assert_eq!(serialize(&[outer]), "- - *");
        // Inline code that would start a block after a line ending, and padding.
        assert_eq!(
            serialize(&[Node::Paragraph(vec![Node::InlineCode("a\r\n# b".into())])]),
            "`a # b`"
        );
        assert_eq!(
            serialize(&[Node::Paragraph(vec![Node::InlineCode("`x".into())])]),
            "`` `x ``"
        );
        assert_eq!(
            serialize(&[Node::Paragraph(vec![Node::InlineCode(" a ".into())])]),
            "`  a  `"
        );
        // Destinations with spaces use angle brackets; empty URL with a title too.
        let link = Node::Link {
            url: "https://x.test/a b".into(),
            title: Some("T \"q\"".into()),
            children: vec![text("l")],
        };
        assert_eq!(
            serialize(&[Node::Paragraph(vec![link])]),
            "[l](<https://x.test/a b> \"T \\\"q\\\"\")"
        );
        let image = Node::Image {
            url: String::new(),
            title: Some("t".into()),
            alt: "a]".into(),
        };
        assert_eq!(
            serialize(&[Node::Paragraph(vec![image])]),
            "![a\\]](<> \"t\")"
        );
        // Autolinks for mailto, and code fences with meta.
        let mail = Node::Link {
            url: "mailto:a@b.c".into(),
            title: None,
            children: vec![text("a@b.c")],
        };
        assert_eq!(serialize(&[Node::Paragraph(vec![mail])]), "<a@b.c>");
        let code = Node::Code {
            value: "x".into(),
            lang: Some("js".into()),
            meta: Some("title=`a`".into()),
        };
        assert_eq!(serialize(&[code]), "```js title=&#x60;a&#x60;\nx\n```");
        // Ordered task items and items starting with a line ending.
        let task = Node::ListItem {
            spread: false,
            checked: Some(true),
            children: vec![Node::Paragraph(vec![text("t")])],
        };
        let ordered = Node::List {
            ordered: true,
            start: Some(9),
            spread: false,
            children: vec![task],
        };
        assert_eq!(serialize(&[ordered]), "9. [x] t");
        assert_eq!(insert_checkbox("1.\nx", "[ ] "), "1.\n[ ] x");
        assert_eq!(insert_checkbox("x", "[ ] "), "x");
        assert_eq!(insert_checkbox("-x", "[ ] "), "-x");
        // A heading starting with a space keeps it as a character reference.
        assert_eq!(
            serialize(&[Node::Heading {
                depth: 2,
                children: vec![text(" a")]
            }]),
            "## &#x20;a"
        );
        // Attention markers encode a trailing space inside.
        assert_eq!(
            serialize(&[Node::Paragraph(vec![
                text("a"),
                Node::Strong(vec![text("b ")]),
                text("c")
            ])]),
            "a**b&#x20;**&#x63;"
        );
    }

    #[test]
    fn escapes_with_every_unsafe_condition() {
        let serializer = Serializer {
            stack: vec!["paragraph", "phrasing"],
            ..Serializer::default()
        };
        let safe = |value: &str| serializer.safe(value, "\n", "\n", &[]);
        assert_eq!(safe("a](b"), "a]\\(b");
        assert_eq!(safe("&amp; & x"), "\\&amp; & x");
        assert_eq!(safe("a@b.c www.x http://y"), "a\\@b.c www\\.x http\\://y");
        assert_eq!(safe("1) one"), "1\\) one");
        assert_eq!(safe("![x"), "!\\[x");
        assert_eq!(safe("a\\*b \\"), "a\\\\\\*b \\\\");
        assert_eq!(safe(" lead"), "&#x20;lead");
        let heading = Serializer {
            stack: vec!["headingAtx", "phrasing"],
            ..serializer
        };
        assert_eq!(heading.safe("a #", "# ", "\n", &[]), "a \\#");
        assert_eq!(classify(Some('\u{3000}')), Some(1));
        assert_eq!(classify(Some('🎉')), Some(2));
        assert_eq!(classify(Some('a')), None);
        assert_eq!(longest_streak("a``b```", '`'), 3);
    }

    #[test]
    fn discord_links_drop_nested_images() {
        let tree =
            read_markdown("[**![a](https://x.test/i.png)** b](https://x.test)", None).children;
        let out = render_markdown(
            &tree,
            MarkdownOptions {
                flavor: Flavor::Discord,
                improved: true,
                spoilers: false,
            },
        );
        assert_eq!(out, "[**a** b](https://x.test)");
    }
}
