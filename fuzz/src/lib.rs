//! Shared oracles for the copied.md `cargo-fuzz` targets.
//!
//! The HTML, RTF and spoiler checks mirror `crates/copied-core/tests/invariants.rs`, so a
//! fuzzer finding means the same thing as a failing property test.

use html5ever::tendril::TendrilSink;
use html5ever::{QualName, local_name, namespace_url, ns, parse_fragment};
use markup5ever_rcdom::{Handle, NodeData, RcDom};

/// Builds a string from fuzz bytes using only characters in `alphabet`.
pub fn from_alphabet(bytes: &[u8], alphabet: &[u8]) -> String {
    bytes
        .iter()
        .map(|&b| char::from(alphabet[usize::from(b) % alphabet.len()]))
        .collect()
}

fn element_children(handle: &Handle, out: &mut Vec<Handle>) {
    for child in handle.children.borrow().iter() {
        if let NodeData::Element { .. } = child.data {
            out.push(child.clone());
        }
        element_children(child, out);
    }
}

/// Asserts that rendered HTML has balanced, non-nested links, no active elements, no
/// event-handler attributes, and only `http:`, `https:` or `mailto:` link destinations.
///
/// URL schemes are checked on parsed `href`/`src` values, so text that merely mentions
/// `javascript:` is allowed.
pub fn assert_safe_html(html: &str) {
    // `<script` or `<iframe` may legitimately appear inside a quoted attribute value, so
    // active elements are checked on the parsed DOM below instead of by substring.
    let lower = html.to_ascii_lowercase();

    // `/<\/?a(?=[\s>])[^>]*>/gi`
    let mut depth: i32 = 0;
    for (index, _) in lower.match_indices('<') {
        let rest = &lower[index + 1..];
        let (closing, rest) = rest.strip_prefix('/').map_or((false, rest), |r| (true, r));
        let Some(after) = rest.strip_prefix('a') else {
            continue;
        };
        if after.starts_with(|c: char| c.is_whitespace() || c == '>') && after.contains('>') {
            depth += if closing { -1 } else { 1 };
            assert!((0..=1).contains(&depth), "link nesting in {html:?}");
        }
    }
    assert_eq!(depth, 0, "unbalanced links in {html:?}");

    let context = QualName::new(None, ns!(html), local_name!("template"));
    let dom: RcDom = parse_fragment(
        RcDom::default(),
        html5ever::ParseOpts::default(),
        context,
        Vec::new(),
    )
    .one(html);
    let mut elements = Vec::new();
    element_children(&dom.document, &mut elements);
    let base = url::Url::parse("https://example.org/").expect("base");
    for element in elements {
        let NodeData::Element { name, attrs, .. } = &element.data else {
            continue;
        };
        let tag = name.local.to_string();
        assert!(
            !["script", "iframe", "object", "embed", "style", "form"].contains(&tag.as_str()),
            "<{tag}> in {html:?}"
        );
        for attr in attrs.borrow().iter() {
            let attr_name = attr.name.local.to_string().to_ascii_lowercase();
            assert!(!attr_name.starts_with("on"), "{attr_name} in {html:?}");
            if attr_name == "href" || attr_name == "src" {
                let value = attr.value.to_string();
                let compact: String = value
                    .trim()
                    .chars()
                    .filter(|c| !matches!(c, '\t' | '\n' | '\r'))
                    .collect::<String>()
                    .to_ascii_lowercase();
                assert!(
                    !compact.starts_with("javascript:"),
                    "javascript: destination in {html:?}"
                );
                let parsed = base.join(&value).unwrap_or_else(|e| {
                    panic!("unparsable {attr_name}={value:?} ({e}) in {html:?}")
                });
                assert!(
                    ["http", "https", "mailto"].contains(&parsed.scheme()),
                    "{value:?} in {html:?}"
                );
            }
        }
    }
}

/// Asserts that an RTF document opens with `{\rtf1` and its groups are balanced.
pub fn assert_valid_rtf_groups(rtf: &str) {
    assert!(rtf.starts_with("{\\rtf1"), "{rtf:?}");
    assert_balanced_rtf(rtf);
}

/// Asserts that unescaped `{`/`}` in an RTF fragment never close below depth zero and
/// end at depth zero.
pub fn assert_balanced_rtf(rtf: &str) {
    let mut depth: i64 = 0;
    let bytes = rtf.as_bytes();
    let mut index = 0;
    while index < bytes.len() {
        match bytes[index] {
            b'\\' => index += 1,
            b'{' => depth += 1,
            b'}' => {
                depth -= 1;
                assert!(depth >= 0, "{rtf:?}");
            }
            _ => {}
        }
        index += 1;
    }
    assert_eq!(depth, 0, "{rtf:?}");
}

/// Spoiler token planted in fuzzed documents. Filler alphabets never produce it.
pub const TOKEN: &str = "Zq9Spoiler";

/// Prose fragments that cannot open a code block or swallow the spoiler markup.
pub const SAFE_FRAGMENTS: &[&str] = &[
    "# Heading",
    "> Quote",
    "- item\n- **bold** item",
    "1. one\n2. two",
    "| A | B |\n|---|---|\n| a | b |",
    "Text with *emphasis* and `code`.",
    "[Link](https://example.org/page)",
    "---",
    "Footnote use[^n]\n\n[^n]: The note.",
];

/// Spoiler templates from `invariants.rs`, each containing [`TOKEN`] inside a spoiler.
pub fn spoiler_contexts() -> Vec<String> {
    let open = "<span data-spoiler=\"true\">";
    vec![
        format!("a {open}{TOKEN}</span> b"),
        format!("**{open}{TOKEN}</span>** after"),
        format!("# heading {open}{TOKEN}</span>"),
        format!("- {open}{TOKEN}</span>\n- next"),
        format!("> {open}{TOKEN}</span>"),
        format!("| A | {open}{TOKEN}</span> |\n|---|---|\n| c | d {open}{TOKEN}</span> |"),
        format!("{open}{TOKEN}\n\nmore {TOKEN}</span> visible"),
        format!("x {open}{TOKEN}"),
        format!("**{open}{TOKEN}** y</span> z"),
        format!("{open}**{TOKEN}</span> z**"),
        format!("<div>a {open}{TOKEN}\n{TOKEN}</span> b</div>"),
        format!("<div>a {open}{TOKEN}</div>\n\nafter"),
        format!("x <span data-spoiler=\"true\" {TOKEN}"),
        format!("[{open}{TOKEN}</span>](https://example.org/)"),
        format!("{open}[{TOKEN}](https://example.org/{TOKEN}) x</span>"),
        format!("{open}<span>{TOKEN}</span> {TOKEN}</span>"),
        format!("x[^s]\n\n[^s]: {open}{TOKEN}</span>"),
        format!("{open}a||{TOKEN}</span> {open}b!<{TOKEN}</span>"),
        format!("<SPAN DATA-SPOILER='TRUE'>{TOKEN}</SPAN>"),
        format!("{open}\u{FDD2}{TOKEN}</span>"),
        format!("# **a {open}{TOKEN}** c</span>"),
        format!("**x __a {open}{TOKEN}__ c</span>**"),
        format!("```\n~~~\n```\n\n| a | {open}{TOKEN}</span> |\n|---|---|\n| c | d |"),
        format!("````\n```\n````\n\n| a | {open}{TOKEN}</span> |\n|---|---|\n| c | d |"),
        format!("{open} {TOKEN} </span>"),
    ]
}

/// Byte ranges strictly inside `open … close` pairs (non-nested markers).
pub fn paired_regions(text: &str, open: &str, close: &str) -> Vec<(usize, usize)> {
    let mut regions = Vec::new();
    let mut from = 0;
    while let Some(start) = text[from..].find(open).map(|i| from + i + open.len()) {
        let Some(end) = text[start..].find(close).map(|i| start + i) else {
            break;
        };
        regions.push((start, end));
        from = end + close.len();
    }
    regions
}

/// Ranges inside Signal spoiler spans, counting nested spans.
pub fn signal_regions(html: &str) -> Vec<(usize, usize)> {
    let open = "<span data-spoiler=\"true\">";
    let mut regions = Vec::new();
    let mut from = 0;
    while let Some(start) = html[from..].find(open).map(|i| from + i + open.len()) {
        let mut depth = 1;
        let mut cursor = start;
        let end = loop {
            let next_open = html[cursor..].find("<span").map(|i| cursor + i);
            let next_close = html[cursor..].find("</span>").map(|i| cursor + i);
            match (next_open, next_close) {
                (Some(o), Some(c)) if o < c => {
                    depth += 1;
                    cursor = o + 5;
                }
                (_, Some(c)) => {
                    depth -= 1;
                    cursor = c + 7;
                    if depth == 0 {
                        break c;
                    }
                }
                _ => break html.len(),
            }
        };
        regions.push((start, end));
        from = end;
    }
    regions
}

/// Byte ranges inside fenced code blocks (`CommonMark` fence rules, ignoring container
/// prefixes such as `>` and list indentation).
pub fn fenced_regions(text: &str) -> Vec<(usize, usize)> {
    let mut regions = Vec::new();
    let mut open: Option<(char, usize, usize)> = None;
    let mut offset = 0;
    for line in text.split_inclusive('\n') {
        let trimmed = line.trim_start_matches(|c: char| c == '>' || c.is_whitespace());
        let fence_char = trimmed.chars().next().filter(|c| *c == '`' || *c == '~');
        let run = fence_char.map_or(0, |c| trimmed.chars().take_while(|&x| x == c).count());
        match (open, fence_char) {
            (None, Some(c)) if run >= 3 => open = Some((c, run, offset + line.len())),
            (Some((c, len, start)), Some(found))
                if found == c && run >= len && trimmed[run * c.len_utf8()..].trim().is_empty() =>
            {
                regions.push((start, offset));
                open = None;
            }
            _ => {}
        }
        offset += line.len();
    }
    if let Some((_, _, start)) = open {
        regions.push((start, text.len()));
    }
    regions
}

/// Asserts every occurrence of [`TOKEN`] lies entirely inside one of `regions`.
pub fn assert_hidden(value: &str, regions: &[(usize, usize)], what: &str) {
    for (index, _) in value.match_indices(TOKEN) {
        assert!(
            regions
                .iter()
                .any(|&(start, end)| index >= start && index + TOKEN.len() <= end),
            "spoiler text leaked outside a spoiler in {what}: {value:?}"
        );
    }
}

/// Asserts [`TOKEN`] never appears inside a fenced code block.
pub fn assert_not_in_code(value: &str, what: &str) {
    let fenced = fenced_regions(value);
    for (index, _) in value.match_indices(TOKEN) {
        assert!(
            !fenced
                .iter()
                .any(|&(start, end)| index >= start && index < end),
            "spoiler text shown inside a code block in {what}: {value:?}"
        );
    }
}

/// Asserts [`TOKEN`] never appears inside a parsed Reddit Markdown code block
/// (indented or fenced).
pub fn assert_reddit_code_has_no_spoilers(value: &str) {
    fn walk(node: &markdown::mdast::Node) {
        if let markdown::mdast::Node::Code(code) = node {
            assert!(
                !code.value.contains(TOKEN),
                "spoiler text shown inside a Reddit code block: {:?}",
                code.value
            );
        }
        if let Some(children) = node.children() {
            for child in children {
                walk(child);
            }
        }
    }
    let parsed =
        markdown::to_mdast(value, &markdown::ParseOptions::gfm()).expect("Reddit Markdown parses");
    walk(&parsed);
}
