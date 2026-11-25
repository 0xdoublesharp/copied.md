//! Fuzzes `convert_markdown` across all 15 targets and both conversion options.
//!
//! Checks determinism, the `schemaVersion: 1` payload contract (MIME keys, presence of
//! each representation, UTF-16 `sourceCodeUnits`, code-point `outputLength` except the
//! weighted X metric), safe HTML, balanced RTF groups, no leaked spoiler sentinels, and
//! fail-closed spoilers on Signal, Telegram, Discord and Reddit.
#![no_main]

use arbitrary::Arbitrary;
use copied_core::{
    ConversionOptions, CopyTarget, DiagnosticCode, DiagnosticSeverity, MetricUnit, TARGETS,
    convert_markdown, oracle::convert_markdown_ts_v1,
};
use copied_fuzz::{
    SAFE_FRAGMENTS, assert_hidden, assert_not_in_code, assert_reddit_code_has_no_spoilers,
    assert_safe_html, assert_valid_rtf_groups, from_alphabet, paired_regions, signal_regions,
    spoiler_contexts,
};
use libfuzzer_sys::fuzz_target;

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
    "[Unsafe](javascript:alert%281%29) [Data](DATA:text/html,x) [VB](vbscript:x)",
    "<script>ignored()</script><b>HTML</b>",
    "one  \ntwo ~~three~~",
    "Before <span data-spoiler=\"true\">covered</span> after.",
    "Unclosed <span data-spoiler=\"true\">covered",
    "👩🏽‍💻 🇺🇦 e\u{301} 𝄞 漢字 & < > \\ { }",
    "---",
    "\u{FDD0}forged\u{FDD2} \u{FDD1} <span data-spoiler=\"true\">s</span> \0",
    "<b><i><u>deep</u></i></b> <iframe src=\"javascript:x\"></iframe><svg><a href=\"javascript:x\">s</a></svg>",
    "<a href=\"java\nscript:alert(1)\" onclick=\"x()\">x</a> <img src=x onerror=alert(1)>",
    "[a](<java\tscript:alert(1)>) <javascript:alert(1)> <https://example.org/a?b=c&d>",
    "**bold _italic `code` ~~strike~~_**",
];

const MARKDOWN_ALPHABET: &[u8] = b"abcXYZ019 *_`~#>|[]()!<&\\\n-:/.=\"'{}";

#[derive(Arbitrary, Debug)]
enum Piece {
    Raw(String),
    Fragment(u8),
    Markdownish(Vec<u8>),
}

#[derive(Arbitrary, Debug)]
enum SourceUrl {
    None,
    Fixed,
    Arbitrary(String),
}

#[derive(Arbitrary, Debug)]
enum Filler {
    Fragment(u8),
    Prose(Vec<u8>),
}

#[derive(Arbitrary, Debug)]
struct Input {
    pieces: Vec<Piece>,
    source_url: SourceUrl,
    decorative_unicode: bool,
    spoiler_before: Vec<Filler>,
    spoiler_context: u8,
    spoiler_after: Vec<Filler>,
}

const REPRESENTATION_KEYS: [&str; 4] = ["text/plain", "text/html", "text/rtf", "text/signal"];

fn markdown(pieces: &[Piece]) -> String {
    pieces
        .iter()
        .take(16)
        .map(|piece| match piece {
            Piece::Raw(text) => text.clone(),
            Piece::Fragment(index) => FRAGMENTS[usize::from(*index) % FRAGMENTS.len()].to_owned(),
            Piece::Markdownish(bytes) => from_alphabet(bytes, MARKDOWN_ALPHABET),
        })
        .collect::<Vec<_>>()
        .join("\n\n")
}

fn check_payload_contract(source: &str, options: &ConversionOptions) {
    for definition in TARGETS {
        let id = definition.id;
        let result = convert_markdown(source, id, options);
        assert_eq!(
            convert_markdown(source, id, options),
            result,
            "non-deterministic {id}"
        );
        assert_eq!(result.schema_version, 1);
        assert_eq!(result.target, id);
        assert_eq!(
            result.metrics.source_code_units,
            source.encode_utf16().count()
        );

        let json = serde_json::to_value(&result).expect("serializable");
        assert_eq!(json["schemaVersion"], 1);
        let representations = json["representations"].as_object().expect("object");
        assert!(
            representations.contains_key("text/plain"),
            "{id} has no text/plain"
        );
        for (key, value) in representations {
            assert!(
                REPRESENTATION_KEYS.contains(&key.as_str()),
                "{id} emitted {key}"
            );
            assert!(value.is_string(), "{id} {key} is not a string");
        }

        let reps = &result.representations;
        let signal_malformed = id == CopyTarget::Signal
            && result.diagnostics.iter().any(|d| {
                d.code == DiagnosticCode::SpoilerOmitted
                    && d.severity == DiagnosticSeverity::Warning
            });
        let rich = definition.rich && !signal_malformed;
        assert_eq!(
            reps.html.is_some(),
            rich && id != CopyTarget::Signal,
            "{id} text/html"
        );
        assert_eq!(
            reps.signal.is_some(),
            rich && id == CopyTarget::Signal,
            "{id} text/signal"
        );
        assert!(reps.rtf.is_none(), "{id} text/rtf");

        if id == CopyTarget::Twitter {
            assert_eq!(result.metrics.unit, MetricUnit::XWeightedCharacters);
            assert_eq!(result.metrics.limit, Some(280));
            assert_eq!(
                result.metrics.output_length,
                copied_core::x_weighted_length(&reps.plain)
            );
            let exceeded = result
                .diagnostics
                .iter()
                .any(|d| d.code == DiagnosticCode::LengthExceeded);
            assert_eq!(exceeded, result.metrics.output_length > 280);
        } else {
            assert_eq!(result.metrics.unit, MetricUnit::CodePoints);
            assert_eq!(result.metrics.output_length, reps.plain.chars().count());
            assert_eq!(result.metrics.limit, None);
        }

        let html = if id == CopyTarget::HtmlSource {
            Some(&reps.plain)
        } else {
            reps.html.as_ref().or(reps.signal.as_ref())
        };
        if let Some(html) = html {
            assert_safe_html(html);
        }
        if id == CopyTarget::Telegram {
            let legacy = convert_markdown_ts_v1(source, id, options).representations;
            assert_eq!(
                legacy.rtf.is_some(),
                !legacy.plain.is_empty(),
                "{id} oracle text/rtf"
            );
            if let Some(rtf) = &legacy.rtf {
                assert_valid_rtf_groups(rtf);
            }
        }
        if matches!(
            id,
            CopyTarget::Telegram | CopyTarget::Discord | CopyTarget::Reddit
        ) {
            for value in [Some(&reps.plain), reps.html.as_ref(), reps.rtf.as_ref()]
                .into_iter()
                .flatten()
            {
                assert!(
                    !value.contains(['\u{FDD0}', '\u{FDD1}', '\u{FDD2}']),
                    "sentinel leaked for {id}: {value:?}"
                );
            }
        }
    }
}

fn filler(item: &Filler) -> String {
    match item {
        Filler::Fragment(index) => {
            SAFE_FRAGMENTS[usize::from(*index) % SAFE_FRAGMENTS.len()].to_owned()
        }
        // Lowercase a-y, digits and light punctuation can never spell the token or open
        // markup that would legitimately expose it (fences, raw HTML, links).
        Filler::Prose(bytes) => {
            let text = from_alphabet(
                &bytes[..bytes.len().min(30)],
                b"abcdefghijklmnopqrstuvwxy .,",
            );
            if text.trim().is_empty() {
                "x".to_owned()
            } else {
                text
            }
        }
    }
}

fn check_spoilers_fail_closed(input: &Input, options: &ConversionOptions) {
    let contexts = spoiler_contexts();
    let spoiler = contexts[usize::from(input.spoiler_context) % contexts.len()].clone();
    let source = input
        .spoiler_before
        .iter()
        .take(3)
        .map(filler)
        .chain([spoiler])
        .chain(input.spoiler_after.iter().take(3).map(filler))
        .collect::<Vec<_>>()
        .join("\n\n");
    let options = ConversionOptions {
        source_url: options.source_url.clone(),
        decorative_unicode: false,
    };

    let signal = convert_markdown(&source, CopyTarget::Signal, &options).representations;
    assert_hidden(&signal.plain, &[], "signal text/plain");
    if let Some(html) = &signal.signal {
        assert_hidden(html, &signal_regions(html), "signal text/signal");
    }

    let telegram = convert_markdown(&source, CopyTarget::Telegram, &options).representations;
    assert_hidden(
        &telegram.plain,
        &paired_regions(&telegram.plain, "||", "||"),
        "telegram text/plain",
    );
    assert_not_in_code(&telegram.plain, "telegram text/plain");
    let html = telegram.html.as_deref().unwrap_or_default();
    assert_hidden(
        html,
        &paired_regions(html, "<tg-spoiler>", "</tg-spoiler>"),
        "telegram text/html",
    );

    let discord = convert_markdown(&source, CopyTarget::Discord, &options)
        .representations
        .plain;
    assert_hidden(&discord, &paired_regions(&discord, "||", "||"), "discord");
    assert_not_in_code(&discord, "discord");

    let reddit = convert_markdown(&source, CopyTarget::Reddit, &options)
        .representations
        .plain;
    assert_hidden(&reddit, &paired_regions(&reddit, ">!", "!<"), "reddit");
    assert_reddit_code_has_no_spoilers(&reddit);
}

fuzz_target!(|input: Input| {
    let options = ConversionOptions {
        source_url: match &input.source_url {
            SourceUrl::None => None,
            SourceUrl::Fixed => Some("https://example.org/docs/page".to_owned()),
            SourceUrl::Arbitrary(url) => Some(url.clone()),
        },
        decorative_unicode: input.decorative_unicode,
    };
    check_payload_contract(&markdown(&input.pieces), &options);
    check_spoilers_fail_closed(&input, &options);
});
