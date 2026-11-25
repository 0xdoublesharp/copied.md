//! Property tests ported from `packages/converters/src/__tests__/invariants.test.ts`,
//! plus a fail-closed spoiler property for every spoiler-capable destination.
//!
//! `COPIED_MD_FUZZ_RUNS` overrides the number of cases (default 200, like the
//! TypeScript suite).

use copied_core::{
    ConversionOptions, CopyTarget, DiagnosticCode, DiagnosticSeverity, MetricUnit, TARGETS,
    convert_markdown, resolve_target,
};
use html5ever::tendril::TendrilSink;
use html5ever::{QualName, local_name, namespace_url, ns, parse_fragment};
use markup5ever_rcdom::{Handle, NodeData, RcDom};
use proptest::prelude::*;
use proptest::string::string_regex;

fn runs() -> u32 {
    std::env::var("COPIED_MD_FUZZ_RUNS")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(200)
}

const FRAGMENTS: &[&str] = &[
    "# Release check",
    "> Quote\n>\n> Second paragraph",
    "- [x] Task\n- **Bold** and *italic*\n  3. Nested",
    "| A | B |\n|---|---|\n| a | b |",
    "~~~ts\nconst a = \"<x>\";\n~~~",
    "[Link][ref]\n\n[ref]: ./notes \"Title\"",
    "[^x] and again [^x]\n\n[^x]: footnote",
    "![Icon](https://example.org/icon.png)",
    "[![Image][asset]][manual]\n\n[asset]: ./asset.png\n[manual]: ./manual",
    "<a href=\"https://example.org/manual\"><img src=\"./asset.png\" alt=\"Image\"></a>",
    "[Relative](./notes) [Fragment](#intro) [Mail](mailto:hi@example.org)",
    "[Unsafe](javascript:alert%281%29)",
    "<script>ignored()</script><b>HTML</b>",
    "one  \ntwo ~~three~~",
    "Before <span data-spoiler=\"true\">covered</span> after.",
    "Unclosed <span data-spoiler=\"true\">covered",
    "👩🏽‍💻 🇺🇦 e\u{301} 𝄞 漢字 & < > \\ { }",
    "---",
    // Sentinel and NUL characters must never forge structure.
    "\u{FDD0}forged\u{FDD2} \u{FDD1} <span data-spoiler=\"true\">s</span> \0",
    "<b><i><u>deep</u></i></b> <iframe src=\"javascript:x\"></iframe><svg><a href=\"javascript:x\">s</a></svg>",
];

fn markdown() -> impl Strategy<Value = String> {
    let part = prop_oneof![
        prop::sample::select(FRAGMENTS).prop_map(str::to_owned),
        string_regex("(?s).{0,60}").expect("regex"),
        string_regex("[a-zA-Z0-9 *_`~#>|\\[\\]()!<&\\n-]{0,40}").expect("regex"),
    ];
    prop::collection::vec(part, 0..=8).prop_map(|parts| parts.join("\n\n"))
}

fn element_children(handle: &Handle, out: &mut Vec<Handle>) {
    for child in handle.children.borrow().iter() {
        if let NodeData::Element { .. } = child.data {
            out.push(child.clone());
        }
        element_children(child, out);
    }
}

/// Mirrors `assertSafeHtml` from the TypeScript suite.
fn assert_safe_html(html: &str) {
    // `/<\/?a(?=[\s>])[^>]*>/gi`
    let lower = html.to_ascii_lowercase();
    let mut depth: i32 = 0;
    for (index, _) in lower.match_indices('<') {
        let rest = &lower[index + 1..];
        let (closing, rest) = rest.strip_prefix('/').map_or((false, rest), |r| (true, r));
        let Some(after) = rest.strip_prefix('a') else {
            continue;
        };
        if after.starts_with(|c: char| c.is_whitespace() || c == '>') && after.contains('>') {
            depth += if closing { -1 } else { 1 };
            assert!((0..=1).contains(&depth), "link nesting in {html}");
        }
    }
    assert_eq!(depth, 0, "unbalanced links in {html}");

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
            "<{tag}> in {html}"
        );
        for attr in attrs.borrow().iter() {
            let attr_name = attr.name.local.to_string();
            assert!(
                !attr_name.to_ascii_lowercase().starts_with("on"),
                "{attr_name} in {html}"
            );
            if attr_name == "href" || attr_name == "src" {
                let parsed = base.join(&attr.value).expect("parsable URL");
                assert!(
                    ["http", "https", "mailto"].contains(&parsed.scheme()),
                    "{} in {html}",
                    attr.value
                );
            }
        }
    }
}

/// Mirrors `assertValidRtfGroups`.
fn assert_valid_rtf_groups(rtf: &str) {
    assert!(rtf.starts_with("{\\rtf1"), "{rtf}");
    let mut depth: i32 = 0;
    let bytes = rtf.as_bytes();
    let mut index = 0;
    while index < bytes.len() {
        match bytes[index] {
            b'\\' => index += 1,
            b'{' => depth += 1,
            b'}' => {
                depth -= 1;
                assert!(depth >= 0, "{rtf}");
            }
            _ => {}
        }
        index += 1;
    }
    assert_eq!(depth, 0, "{rtf}");
}

#[test]
fn resolves_exactly_the_published_targets_and_legacy_aliases() {
    let ids: std::collections::BTreeSet<_> = TARGETS.iter().map(|t| t.id).collect();
    assert_eq!(ids.len(), TARGETS.len());
    for definition in TARGETS {
        assert_eq!(resolve_target(definition.id.id()), Some(definition.id));
    }
    assert_eq!(resolve_target("html"), Some(CopyTarget::RichText));
    assert_eq!(resolve_target("slack"), Some(CopyTarget::SlackPaste));
    assert_eq!(resolve_target("missing"), None);
}

proptest! {
    #![proptest_config(ProptestConfig { cases: runs(), ..ProptestConfig::default() })]

    #[test]
    fn preserves_payload_metric_and_determinism_invariants(source in markdown(), has_url: bool, decorative_unicode: bool) {
        let options = ConversionOptions {
            decorative_unicode,
            source_url: has_url.then(|| "https://example.org/docs/page".to_owned()),
        };
        for definition in TARGETS {
            let id = definition.id;
            let result = convert_markdown(&source, id, &options);
            prop_assert_eq!(&convert_markdown(&source, id, &options), &result);
            prop_assert_eq!(result.schema_version, 1);
            prop_assert_eq!(result.target, id);
            prop_assert_eq!(result.metrics.source_code_units, source.encode_utf16().count());

            let reps = &result.representations;
            let signal_malformed = id == CopyTarget::Signal
                && result.diagnostics.iter().any(|d| d.code == DiagnosticCode::SpoilerOmitted && d.severity == DiagnosticSeverity::Warning);
            let rich = definition.rich && !signal_malformed;
            prop_assert_eq!(reps.html.is_some(), rich && id != CopyTarget::Signal);
            prop_assert_eq!(reps.signal.is_some(), rich && id == CopyTarget::Signal);
            prop_assert_eq!(reps.rtf.is_some(), id == CopyTarget::Telegram && !reps.plain.is_empty());

            if id == CopyTarget::Twitter {
                prop_assert_eq!(result.metrics.unit, MetricUnit::XWeightedCharacters);
                prop_assert_eq!(result.metrics.limit, Some(280));
                let exceeded = result.diagnostics.iter().any(|d| d.code == DiagnosticCode::LengthExceeded);
                prop_assert_eq!(exceeded, result.metrics.output_length > 280);
            } else {
                prop_assert_eq!(result.metrics.unit, MetricUnit::CodePoints);
                prop_assert_eq!(result.metrics.output_length, reps.plain.chars().count());
                prop_assert_eq!(result.metrics.limit, None);
            }

            let html = if id == CopyTarget::HtmlSource { Some(&reps.plain) } else { reps.html.as_ref().or(reps.signal.as_ref()) };
            if let Some(html) = html {
                assert_safe_html(html);
            }
            if let Some(rtf) = &reps.rtf {
                assert_valid_rtf_groups(rtf);
            }
            // Spoiler sentinels are internal to these renderers; source copies are neutralized.
            if matches!(id, CopyTarget::Telegram | CopyTarget::Discord | CopyTarget::Reddit) {
                for value in [Some(&reps.plain), reps.html.as_ref(), reps.rtf.as_ref()].into_iter().flatten() {
                    prop_assert!(!value.contains(['\u{FDD0}', '\u{FDD1}', '\u{FDD2}']), "sentinel leaked for {}", id);
                }
            }
        }
    }

    #[test]
    fn discards_unsafe_link_schemes_even_when_casing_and_labels_vary(
        label in "[a-zA-Z0-9]{1,24}",
        scheme in prop::sample::select(&["javascript", "JaVaScRiPt", "data", "vbscript", "file"][..]),
    ) {
        let source = format!("[{label}]({scheme}:example)");
        let options = ConversionOptions { source_url: Some("https://example.org/docs/".to_owned()), decorative_unicode: false };
        for definition in TARGETS {
            let result = convert_markdown(&source, definition.id, &options);
            prop_assert!(result.diagnostics.iter().any(|d| d.code == DiagnosticCode::UnsupportedUrl));
            let reps = &result.representations;
            for value in [Some(&reps.plain), reps.html.as_ref(), reps.rtf.as_ref(), reps.signal.as_ref()].into_iter().flatten() {
                prop_assert!(value.contains(label.as_str()), "{} lost the label: {}", definition.id, value);
                let unsafe_destination = format!("{scheme}:example");
                prop_assert!(!value.contains(&unsafe_destination), "{} kept {}", definition.id, value);
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Spoilers never leak.

const TOKEN: &str = "Zq9Spoiler";

const SAFE_FRAGMENTS: &[&str] = &[
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

fn spoiler_context() -> impl Strategy<Value = String> {
    let open = "<span data-spoiler=\"true\">";
    let contexts = vec![
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
    ];
    prop::sample::select(contexts)
}

fn spoiler_document() -> impl Strategy<Value = String> {
    let filler = prop_oneof![
        prop::sample::select(SAFE_FRAGMENTS).prop_map(str::to_owned),
        string_regex("[a-y .,]{1,30}").expect("regex"),
    ];
    (
        prop::collection::vec(filler.clone(), 0..3),
        spoiler_context(),
        prop::collection::vec(filler, 0..3),
    )
        .prop_map(|(before, spoiler, after)| {
            before
                .into_iter()
                .chain([spoiler])
                .chain(after)
                .collect::<Vec<_>>()
                .join("\n\n")
        })
}

/// Byte ranges strictly inside `open … close` pairs (non-nested markers).
fn paired_regions(text: &str, open: &str, close: &str) -> Vec<(usize, usize)> {
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
fn signal_regions(html: &str) -> Vec<(usize, usize)> {
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

/// Byte ranges inside fenced code blocks (`CommonMark` fence rules, ignoring
/// container prefixes such as `>` and list indentation).
fn fenced_regions(text: &str) -> Vec<(usize, usize)> {
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

fn assert_not_in_code(value: &str, what: &str) {
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

fn assert_hidden(value: &str, regions: &[(usize, usize)], what: &str) {
    for (index, _) in value.match_indices(TOKEN) {
        assert!(
            regions
                .iter()
                .any(|&(start, end)| index >= start && index + TOKEN.len() <= end),
            "spoiler text leaked outside a spoiler in {what}: {value:?}"
        );
    }
}

proptest! {
    #![proptest_config(ProptestConfig { cases: runs(), ..ProptestConfig::default() })]

    #[test]
    fn spoilers_fail_closed_on_every_spoiler_capable_destination(source in spoiler_document(), has_url: bool) {
        let options = ConversionOptions { source_url: has_url.then(|| "https://example.org/docs/".to_owned()), decorative_unicode: false };

        let signal = convert_markdown(&source, CopyTarget::Signal, &options).representations;
        assert_hidden(&signal.plain, &[], "signal text/plain");
        if let Some(html) = &signal.signal {
            assert_hidden(html, &signal_regions(html), "signal text/signal");
        }

        let telegram = convert_markdown(&source, CopyTarget::Telegram, &options).representations;
        assert_hidden(&telegram.plain, &[], "telegram text/plain");
        if let Some(rtf) = &telegram.rtf {
            assert_hidden(rtf, &[], "telegram text/rtf");
        }
        let html = telegram.html.as_deref().unwrap_or_default();
        assert_hidden(html, &paired_regions(html, "<tg-spoiler>", "</tg-spoiler>"), "telegram text/html");

        let discord = convert_markdown(&source, CopyTarget::Discord, &options).representations.plain;
        assert_hidden(&discord, &paired_regions(&discord, "||", "||"), "discord");
        assert_not_in_code(&discord, "discord");

        let reddit = convert_markdown(&source, CopyTarget::Reddit, &options).representations.plain;
        assert_hidden(&reddit, &paired_regions(&reddit, ">!", "!<"), "reddit");
        assert_not_in_code(&reddit, "reddit");
    }
}
