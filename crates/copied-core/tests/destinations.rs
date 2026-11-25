//! Destination regression tests: every target, the `ts-oracle-v1` destination
//! behaviors that still hold, and each greenfield improvement.

use copied_core::limits::{MAX_HTML_DEPTH, MAX_INPUT_BYTES};
use copied_core::{
    ClipboardPayload, ConversionOptions, CopyTarget, DiagnosticCode, DiagnosticSeverity,
    MarkdownConversion, TARGETS, convert_markdown, resolve_target,
};

const FIXTURE: &str = include_str!("fixtures/clipboard-fidelity.md");

fn convert(markdown: &str, target: CopyTarget) -> MarkdownConversion {
    convert_markdown(markdown, target, &ConversionOptions::default())
}

fn reps(markdown: &str, target: CopyTarget) -> ClipboardPayload {
    convert(markdown, target).representations
}

fn plain(markdown: &str, target: CopyTarget) -> String {
    reps(markdown, target).plain
}

fn html(markdown: &str, target: CopyTarget) -> String {
    reps(markdown, target).html.expect("text/html")
}

fn codes(conversion: &MarkdownConversion) -> Vec<(DiagnosticCode, DiagnosticSeverity)> {
    conversion
        .diagnostics
        .iter()
        .map(|d| (d.code, d.severity))
        .collect()
}

// ---------------------------------------------------------------------------
// Every target.

#[test]
fn every_target_produces_its_representations() {
    for definition in TARGETS {
        let result = convert(FIXTURE, definition.id);
        let r = &result.representations;
        let expected_rich = definition.rich;
        assert_eq!(
            r.html.is_some(),
            expected_rich && definition.id != CopyTarget::Signal,
            "{}",
            definition.id
        );
        assert_eq!(
            r.signal.is_some(),
            definition.id == CopyTarget::Signal,
            "{}",
            definition.id
        );
        assert!(r.rtf.is_none(), "{}", definition.id);
        assert_ne!(r.plain, "");
        if definition.id == CopyTarget::Signal {
            assert!(!r.plain.contains("surprise"));
        }
    }
}

#[test]
fn plain_text_family_strips_markup() {
    for target in [
        CopyTarget::PlainText,
        CopyTarget::Facebook,
        CopyTarget::Sms,
        CopyTarget::Twitter,
    ] {
        let text = plain(
            "**Bold** [link](https://example.org) `code`\n\n- item",
            target,
        );
        assert_eq!(
            text, "Bold link (https://example.org) code\n\n• item",
            "{target}"
        );
    }
}

#[test]
fn decorative_unicode_applies_to_facebook_twitter_and_telegram_only() {
    let options = ConversionOptions {
        decorative_unicode: true,
        ..ConversionOptions::default()
    };
    for target in [
        CopyTarget::Facebook,
        CopyTarget::Twitter,
        CopyTarget::Telegram,
    ] {
        assert_eq!(
            convert_markdown("**Hi** *yo*", target, &options)
                .representations
                .plain,
            "𝗛𝗶 𝘺𝘰",
            "{target}"
        );
    }
    assert_eq!(
        convert_markdown("**Hi**", CopyTarget::PlainText, &options)
            .representations
            .plain,
        "Hi"
    );
}

#[test]
fn rich_text_and_html_source_share_sanitized_html() {
    let markdown =
        "**Bold** [local](custom:ignored) ![Illustration](https://example.com/image.png)";
    let rich = convert(markdown, CopyTarget::RichText);
    assert_eq!(
        rich.representations.plain,
        "Bold local Illustration (https://example.com/image.png)"
    );
    assert_eq!(
        rich.representations.html.as_deref(),
        Some(
            "<p><strong>Bold</strong> local <a href=\"https://example.com/image.png\">Illustration</a></p>"
        )
    );
    assert_eq!(
        codes(&rich),
        [
            (DiagnosticCode::UnsupportedUrl, DiagnosticSeverity::Warning),
            (DiagnosticCode::FormattingOmitted, DiagnosticSeverity::Info)
        ]
    );
    let source = convert(markdown, CopyTarget::HtmlSource);
    assert_eq!(
        Some(source.representations.plain),
        rich.representations.html
    );
    assert_eq!(source.representations.html, None);
}

#[test]
fn rich_html_strips_scripts_handlers_and_unsafe_urls() {
    let markdown = "<script>alert(1)</script><style>body{}</style><iframe src=\"https://evil.test\"></iframe><div onclick=\"bad()\"><b>Clean</b> <a href=\"vbscript:x\">Unsafe</a></div>";
    assert_eq!(
        html(markdown, CopyTarget::RichText),
        "<div><b>Clean</b> <a>Unsafe</a></div>"
    );
}

#[test]
fn footnote_links_match_their_anchors() {
    let out = html("Footnote.[^n]\n\n[^n]: Notes.", CopyTarget::RichText);
    for fragment in ["#user-content-fn-n", "#user-content-fnref-n"] {
        assert!(out.contains(&format!("href=\"{fragment}\"")), "{out}");
        assert!(out.contains(&format!("id=\"{}\"", &fragment[1..])), "{out}");
    }
}

#[test]
fn slack_api_uses_mrkdwn_links_and_escapes() {
    assert_eq!(
        plain(
            "Read [the report](https://example.com).",
            CopyTarget::SlackApi
        ),
        "Read <https://example.com|the report>."
    );
    assert_eq!(
        plain(
            "Read [the report](https://example.com).",
            CopyTarget::SlackPaste
        ),
        "Read the report (https://example.com)."
    );
    assert_eq!(plain("a < b & c", CopyTarget::SlackApi), "a &lt; b &amp; c");
}

#[test]
fn slack_api_table_pipes_are_literal_inside_code_fences() {
    assert_eq!(
        plain(
            "| A | B |\n| --- | --- |\n| a\\|b | c |",
            CopyTarget::SlackApi
        ),
        "```\nA | B\na|b | c\n```"
    );
}

#[test]
fn reddit_keeps_ordered_list_starts_and_gfm_tables() {
    assert_eq!(
        plain("3. Parent\n   - Child\n4. Next", CopyTarget::Reddit),
        "3. Parent\n   - Child\n4. Next"
    );
    assert_eq!(
        plain("| a | b |\n|---|:-:|\n| c | d |", CopyTarget::Reddit),
        "| a |  b  |\n| - | :-: |\n| c |  d  |"
    );
}

#[test]
fn reddit_indents_code_and_preserves_its_block_structure() {
    let source = "```rust\ntop\n```\n\n- item\n  ```\n  nested\n  ```\n\n```\nafter list\n```\n\n> quote\n> ```\n> quoted\n> ```\n\n> - item\n>   ```\n>   both\n>   ```";
    let reddit = plain(source, CopyTarget::Reddit);
    assert!(reddit.starts_with("    top\n\n-   item\n\n        nested\n"));
    assert!(reddit.contains("\n\n***\n\n    after list\n\n> quote\n>\n>     quoted"));
    assert!(reddit.ends_with("> -   item\n>\n>         both"));
    let rendered = plain(&reddit, CopyTarget::HtmlSource).replace("\n<hr>", "");
    let expected = plain(source, CopyTarget::HtmlSource)
        .replace(" class=\"language-rust\"", "")
        .replace("<li>item\n", "<li>\n<p>item</p>\n");
    assert_eq!(rendered, expected);
    assert!(plain(source, CopyTarget::Discord).starts_with("```rust\ntop\n```"));
}

#[test]
fn reddit_keeps_code_content_in_ordered_and_code_first_list_items() {
    let source = "1. First\n   ```\n     indented  \n   ```\n2. ```\n   one\n   two  \n   ```";
    let reddit = plain(source, CopyTarget::Reddit);
    assert!(reddit.starts_with("1.  First\n\n          indented  \n"));
    assert!(reddit.ends_with("2.  &nbsp;\n\n        one\n        two  "));
    let rendered = plain(&reddit, CopyTarget::HtmlSource).replace("<p>\u{a0}</p>\n", "");
    let expected =
        plain(source, CopyTarget::HtmlSource).replace("<li>First\n", "<li>\n<p>First</p>\n");
    assert_eq!(rendered, expected);
}

#[test]
fn discord_caps_headings_and_links_images() {
    assert_eq!(plain("#### Deep", CopyTarget::Discord), "### Deep");
    assert_eq!(
        plain("![Alt](https://example.org/i.png)", CopyTarget::Discord),
        "[Alt](https://example.org/i.png)"
    );
    assert_eq!(
        plain(
            "[![Alt](https://example.org/i.png)](https://example.org)",
            CopyTarget::Discord
        ),
        "[Alt](https://example.org)"
    );
}

#[test]
fn x_counts_weighted_characters() {
    let text = format!("{} https://example.com", "漢".repeat(140));
    let result = convert(&text, CopyTarget::Twitter);
    assert_eq!(result.representations.plain, text);
    assert_eq!(result.metrics.output_length, 304);
    assert_eq!(result.metrics.limit, Some(280));
    assert_eq!(
        codes(&result),
        [(DiagnosticCode::LengthExceeded, DiagnosticSeverity::Warning)]
    );
    assert_eq!(copied_core::x_weighted_length("hi 👩🏽‍💻"), 5);
}

#[test]
fn telegram_rtf_keeps_literal_text_and_url_fields_separate() {
    let r = copied_core::oracle::convert_markdown_ts_v1(
        "**Café ☕** `{x} \\b` [local](javascript:bad) [safe](<https://example.com/café?q=\"x\">)",
        CopyTarget::Telegram,
        &ConversionOptions::default(),
    )
    .representations;
    let rtf = r.rtf.expect("rtf");
    assert!(rtf.contains("Caf\\u233? \\u9749?"));
    assert!(rtf.contains("\\{x\\} \\\\b"));
    assert!(rtf.contains("HYPERLINK \"https://example.com/caf%C3%A9?q=%22x%22\""));
    assert!(!rtf.contains("javascript:"));
}

#[test]
fn telegram_plain_text_uses_native_markdown() {
    let result = convert(
        "# Heading\n\n**bold**, *italic*, ~~strike~~, `code`, [link](https://example.org/)\n\n> quote\n\n- item\n  1. nested\n\n```rust\ncode\n```\n\n~~~\n```\n~~~\n\n``a`b``",
        CopyTarget::Telegram,
    );
    assert_eq!(
        result.representations.plain,
        "**Heading**\n\n**bold**, __italic__, ~~strike~~, `code`, link (https://example.org/)\n\n> quote\n\n• item\n  1. nested\n\n```rust\ncode\n```\n\n````\n```\n````\n\na`b"
    );
    assert!(result.representations.rtf.is_none());
}

#[test]
fn telegram_keeps_code_outside_emphasis_markers() {
    for (markdown, expected) in [
        ("**`http://`**", "`http://`"),
        ("**See `x` now**", "**See** `x` **now**"),
        ("**See`x`now**", "**See** `x` **now**"),
        ("*See `x` now*", "__See__ `x` __now__"),
        ("~~See `x` now~~", "~~See~~ `x` ~~now~~"),
        ("# See `x` now", "**See** `x` **now**"),
        ("**a _b `x` c_ d**", "**a b** `x` **c d**"),
        (
            "**[See `x` now](https://example.org/)**",
            "**See** `x` **now (https://example.org/)**",
        ),
        ("**See ``a`b`` now**", "**See** a`b **now**"),
    ] {
        assert_eq!(
            plain(markdown, CopyTarget::Telegram),
            expected,
            "{markdown}"
        );
    }
}

#[test]
fn telegram_html_keeps_quotes_and_code_next_to_lists() {
    let markdown = "Intro\n\n- Item\n  > First\n  >\n  > Second\n\n  ```\n    code\n  ```\n\nAfter";
    assert_eq!(
        html(markdown, CopyTarget::Telegram),
        "Intro<br><br>• Item<br><blockquote>First<br><br>Second</blockquote><br><pre>  code</pre><br>After"
    );
}

#[test]
fn whatsapp_markup_and_visible_urls() {
    let text = plain(FIXTURE, CopyTarget::Whatsapp);
    for part in [
        "*Release readiness*",
        "*Ready* for review, with _one_ follow-up and ~no migration~.",
        "1. Review the rollout plan (https://example.org/rollout).",
        "- Watch the queue.",
        "> Keep the previous build until checks finish.",
        "`check --dry-run`",
        "```if ready:\n    publish(\"staging\")```",
    ] {
        assert!(text.contains(part), "{part:?} missing from {text}");
    }
    assert_eq!(
        plain(
            "Inline: ``a`b``\n\n~~~\nFence: ```\n~~~",
            CopyTarget::Whatsapp
        ),
        "Inline: a`b\n\nFence: ```"
    );
}

#[test]
fn signal_renders_composer_styles_and_omits_spoilers_in_plain_text() {
    let result = convert(
        "# Update\n\n***Ready***, ~~old~~, `a < b` and <span data-spoiler=\"true\">**surprise**</span> 🎉",
        CopyTarget::Signal,
    );
    assert_eq!(
        result.representations.plain,
        "Update\n\nReady, old, a < b and [spoiler omitted] 🎉"
    );
    assert_eq!(
        result.representations.signal.as_deref(),
        Some(
            "<strong>Update</strong><br><br><em><strong>Ready</strong></em>, <s>old</s>, <span data-monospace=\"true\">a &lt; b</span> and <span data-spoiler=\"true\"><strong>surprise</strong></span> 🎉"
        )
    );
    assert_eq!(
        codes(&result),
        [(DiagnosticCode::SpoilerOmitted, DiagnosticSeverity::Info)]
    );
}

#[test]
fn signal_fails_closed_for_malformed_spoilers() {
    for markdown in [
        "Before <span data-spoiler=\"true\">private\n\ntext",
        "Before <span data-spoiler=\"true\" private text",
        "Before <span class=x data-spoiler=\"true>private text",
        "<span data-spoiler=\"true\">\nprivate\n\ntext",
        "Before **<span data-spoiler=\"true\">private** text</span>",
        "Before <span data-spoiler=\"true\">**private</span> text**",
    ] {
        let result = convert(markdown, CopyTarget::Signal);
        assert!(
            result.representations.plain.contains("[spoiler omitted]"),
            "{markdown}"
        );
        assert!(
            !result.representations.plain.contains("private"),
            "{markdown}"
        );
        assert_eq!(result.representations.signal, None, "{markdown}");
        assert!(
            codes(&result).contains(&(DiagnosticCode::SpoilerOmitted, DiagnosticSeverity::Warning))
        );
    }
}

#[test]
fn signal_spoiler_attribute_quoting_and_explicit_close_lines() {
    let r = reps(
        "Before <SPAN class=x DATA-SPOILER='TRUE'>private</SPAN> after",
        CopyTarget::Signal,
    );
    assert_eq!(r.plain, "Before [spoiler omitted] after");
    assert_eq!(
        r.signal.as_deref(),
        Some("Before <span data-spoiler=\"true\">private</span> after")
    );
    let r = reps(
        "Before <span data-spoiler=\"true\">private\n\ntext\n\n</span>\n\nAfter",
        CopyTarget::Signal,
    );
    assert_eq!(r.plain, "Before [spoiler omitted]\n\nAfter");
    let r = reps(
        "<div>Before <span data-spoiler=\"true\">hidden\ntext</span> after.</div>",
        CopyTarget::Signal,
    );
    assert_eq!(r.plain, "Before [spoiler omitted] after.");
    assert!(
        r.signal
            .unwrap()
            .contains("<span data-spoiler=\"true\">hidden<br>text</span>")
    );
}

// ---------------------------------------------------------------------------
// Improvement 1: nested emphasis markers.

#[test]
fn bold_inside_headings_does_not_repeat_markers() {
    let markdown = "# **Bold** heading";
    assert_eq!(plain(markdown, CopyTarget::SlackApi), "*Bold heading*");
    assert_eq!(plain(markdown, CopyTarget::Whatsapp), "*Bold heading*");
    assert_eq!(
        html(markdown, CopyTarget::Telegram),
        "<strong>Bold heading</strong>"
    );
    assert_eq!(
        reps(markdown, CopyTarget::Signal).signal.as_deref(),
        Some("<strong>Bold heading</strong>")
    );
}

#[test]
fn nested_same_styles_collapse_but_different_styles_nest() {
    assert_eq!(plain("**a __b__ c**", CopyTarget::SlackApi), "*a b c*");
    assert_eq!(plain("*a _b_ c*", CopyTarget::Whatsapp), "_a b c_");
    assert_eq!(plain("~~a ~b~ c~~", CopyTarget::SlackApi), "~a b c~");
    assert_eq!(plain("***both***", CopyTarget::Whatsapp), "_*both*_");
    assert_eq!(
        html("# Title *it*", CopyTarget::Telegram),
        "<strong>Title <em>it</em></strong>"
    );
    assert_eq!(plain("# **B**", CopyTarget::Telegram), "**B**");
}

// ---------------------------------------------------------------------------
// Improvement 2: native spoilers.

const SPOILER: &str = "Hide <span data-spoiler=\"true\">the **end**</span> now.";
const MALFORMED: &str = "Before **<span data-spoiler=\"true\">private** text</span>";

#[test]
fn telegram_spoilers_stay_hidden_on_both_surfaces() {
    let result = convert(SPOILER, CopyTarget::Telegram);
    let r = &result.representations;
    assert_eq!(
        r.html.as_deref(),
        Some("Hide <tg-spoiler>the <strong>end</strong></tg-spoiler> now.")
    );
    assert_eq!(r.plain, "Hide ||the **end**|| now.");
    assert!(r.rtf.is_none());
    assert_eq!(result.diagnostics, Vec::new());

    let malformed = convert(MALFORMED, CopyTarget::Telegram);
    assert_eq!(
        malformed.representations.html.as_deref(),
        Some("Before <strong>[spoiler omitted]</strong>")
    );
    assert_eq!(malformed.representations.plain, "Before [spoiler omitted]");
    assert_eq!(
        codes(&malformed),
        [(DiagnosticCode::SpoilerOmitted, DiagnosticSeverity::Warning)]
    );
}

#[test]
fn telegram_spoilers_fail_closed_when_the_plain_text_parser_cannot_hide_them() {
    for markdown in [
        "<span data-spoiler=\"true\">secret||x</span>",
        "prefix<span data-spoiler=\"true\">secret</span>",
        "<span data-spoiler=\"true\">secret</span>)",
        "<div><span data-spoiler=\"true\">secret\nsecond line</span></div>",
        "\\` <span data-spoiler=\"true\">secret</span> \\`",
        "\\`\\`\\`\n\n<span data-spoiler=\"true\">secret</span>\n\n\\`\\`\\`",
    ] {
        let result = convert(markdown, CopyTarget::Telegram);
        let text = &result.representations.plain;
        assert!(!text.contains("secret"), "{markdown}: {text}");
        assert!(text.contains("[spoiler omitted]"), "{markdown}: {text}");
        assert_eq!(
            codes(&result),
            [(DiagnosticCode::SpoilerOmitted, DiagnosticSeverity::Info)]
        );
        assert!(
            result
                .representations
                .html
                .unwrap()
                .contains("<tg-spoiler>secret")
        );
    }
    assert_eq!(
        plain(
            "**before <span data-spoiler=\"true\">secret</span> after**",
            CopyTarget::Telegram
        ),
        "**before** ||**secret**|| **after**"
    );
    assert_eq!(
        plain(
            "prefix<span data-spoiler=\"true\">secret</span> and <span data-spoiler=\"true\">hidden</span>",
            CopyTarget::Telegram
        ),
        "prefix[spoiler omitted] and ||hidden||"
    );
}

#[test]
fn telegram_spoilers_cannot_cross_blocks() {
    let r = reps(
        "<span data-spoiler=\"true\">a\n\n> secret</span>",
        CopyTarget::Telegram,
    );
    let out = r.html.unwrap();
    assert_eq!(out, "[spoiler omitted]<blockquote></blockquote>");
    assert!(!r.plain.contains("secret"));
}

#[test]
fn discord_spoilers_use_bars_and_fail_closed() {
    assert_eq!(
        plain(SPOILER, CopyTarget::Discord),
        "Hide ||the **end**|| now."
    );
    let malformed = convert(MALFORMED, CopyTarget::Discord);
    assert_eq!(
        malformed.representations.plain,
        "Before **[spoiler omitted]**"
    );
    assert_eq!(
        codes(&malformed),
        [(DiagnosticCode::SpoilerOmitted, DiagnosticSeverity::Warning)]
    );
    // Inside a table code block a marker would show literally.
    let table = convert(
        "| a | <span data-spoiler=\"true\">secret</span> |\n|---|---|\n| c | d |",
        CopyTarget::Discord,
    );
    assert!(!table.representations.plain.contains("secret"));
    assert!(table.representations.plain.contains("[spoiler omitted]"));
    assert_eq!(
        codes(&table),
        [(DiagnosticCode::SpoilerOmitted, DiagnosticSeverity::Info)]
    );
    // Content that would close the spoiler early is omitted.
    assert_eq!(
        plain(
            "<span data-spoiler=\"true\">a||b</span>",
            CopyTarget::Discord
        ),
        "[spoiler omitted]"
    );
}

#[test]
fn reddit_spoilers_use_reddit_markers_and_fail_closed() {
    assert_eq!(
        plain(SPOILER, CopyTarget::Reddit),
        "Hide >!the **end**!< now."
    );
    assert_eq!(
        plain(MALFORMED, CopyTarget::Reddit),
        "Before **[spoiler omitted]**"
    );
    let multiline = convert(
        "<div>x <span data-spoiler=\"true\">a\nb</span></div>",
        CopyTarget::Reddit,
    );
    assert_eq!(multiline.representations.plain, "x [spoiler omitted]");
    assert_eq!(
        codes(&multiline),
        [(DiagnosticCode::SpoilerOmitted, DiagnosticSeverity::Info)]
    );
    assert_eq!(
        plain(
            "<span data-spoiler=\"true\">a!<b</span>",
            CopyTarget::Reddit
        ),
        "[spoiler omitted]"
    );
    assert_eq!(
        plain(
            "<span data-spoiler=\"true\">&lt;b</span>",
            CopyTarget::Reddit
        ),
        "[spoiler omitted]"
    );
}

#[test]
fn literal_spoiler_code_is_left_alone() {
    let literal = "`<span data-spoiler=\"true\">example</span>`";
    for target in [
        CopyTarget::Telegram,
        CopyTarget::Discord,
        CopyTarget::Reddit,
        CopyTarget::Signal,
    ] {
        let result = convert(literal, target);
        assert!(result.representations.plain.contains("example"), "{target}");
        assert_eq!(result.diagnostics, Vec::new(), "{target}");
    }
}

#[test]
fn sentinel_characters_in_source_cannot_forge_spoilers() {
    let forged = "a <span data-spoiler=\"true\">x\u{FDD2} leaked</span> b";
    for target in [
        CopyTarget::Telegram,
        CopyTarget::Discord,
        CopyTarget::Reddit,
    ] {
        let text = plain(forged, target);
        assert!(
            !text.contains("leaked")
                || text.contains("||x\u{FFFD} leaked||")
                || text.contains(">!x\u{FFFD} leaked!<"),
            "{target}: {text}"
        );
    }
}

#[test]
fn telegram_spoiler_markers_cannot_be_forged_with_html_entities() {
    for source in [
        "<span data-spoiler=\"true\">secret&#xFDD2; hidden</span>",
        "<div><span data-spoiler=\"true\">secret&#xFDD2; hidden</span></div>",
    ] {
        let payload = reps(source, CopyTarget::Telegram);
        assert!(payload.plain.ends_with(" hidden||"), "{}", payload.plain);
        assert!(payload.html.unwrap().ends_with(" hidden</tg-spoiler>"));
    }
}

// ---------------------------------------------------------------------------
// Improvement 3: aligned monospace tables.

const TABLE: &str = "| Service | Status |\n| --- | --- |\n| Web | Ready |\n| Worker | [Logs](https://example.org/worker) |\n| A\\|B | 漢字 |";

#[test]
fn slack_paste_tables_are_aligned_code_blocks() {
    assert_eq!(
        html(TABLE, CopyTarget::SlackPaste),
        "<pre><code>Service | Status\n------- | ---------------------------------\nWeb     | Ready\nWorker  | Logs (https://example.org/worker)\nA|B     | 漢字</code></pre>"
    );
    // Plain text keeps the oracle rows.
    assert!(plain(TABLE, CopyTarget::SlackPaste).starts_with("Service | Status\nWeb | Ready"));
    let raw = "<table><caption>Details</caption><tr><th>First</th><th>Second</th></tr><tr><td>A<br>B</td><td><a href=\"javascript:alert(1)\">Label</a></td></tr></table>";
    assert_eq!(
        html(raw, CopyTarget::SlackPaste),
        "<pre><code>Details\nFirst | Second\n----- | ------\nA B   | Label</code></pre>"
    );
    assert_eq!(
        plain(raw, CopyTarget::SlackPaste),
        "Details\nFirst | Second\nA B | Label"
    );
}

#[test]
fn discord_tables_are_aligned_without_pipe_escapes() {
    assert_eq!(
        plain(TABLE, CopyTarget::Discord),
        "```\nService | Status\n------- | ---------------------------------\nWeb     | Ready\nWorker  | Logs (https://example.org/worker)\nA|B     | 漢字\n```"
    );
}

#[test]
fn whatsapp_tables_are_monospace_with_fallback() {
    assert_eq!(
        plain(
            "| a | b |\n|---|---|\n| **long** | c |",
            CopyTarget::Whatsapp
        ),
        "```a    | b\n---- | -\nlong | c```"
    );
    // A cell containing a triple backtick cannot sit inside a monospace block.
    assert_eq!(
        plain(
            "| a | b |\n|---|---|\n| `` a```b `` | c |",
            CopyTarget::Whatsapp
        ),
        "a | b\na```b | c"
    );
}

// ---------------------------------------------------------------------------
// Improvement 4: Teams, Word, and Slack paste HTML polish.

const QUOTE: &str = "border-left: 3px solid rgba(127, 127, 127, 0.45); margin: 8px 0; padding-left: 12px; opacity: 0.85;";

#[test]
fn teams_and_word_style_quotes_and_code() {
    let teams = html("> quote\n\n`code`", CopyTarget::Teams);
    assert_eq!(
        teams,
        format!(
            "<blockquote style=\"{QUOTE}\">\n<p>quote</p>\n</blockquote>\n<p><code style=\"font-family: monospace; background-color: rgba(127, 127, 127, 0.14);\">code</code></p>"
        )
    );
    let word = html("```\nx\n```", CopyTarget::Word);
    assert_eq!(
        word,
        "<pre style=\"font-family: Consolas, monospace; white-space: pre; background-color: rgba(127, 127, 127, 0.14);\"><code style=\"font-family: Consolas, monospace; background-color: rgba(127, 127, 127, 0.14);\">x\n</code></pre>"
    );
    assert!(html("> q", CopyTarget::Word).starts_with(&format!("<blockquote style=\"{QUOTE}\">")));
    // Generic rich text stays unstyled.
    assert_eq!(
        html("> q", CopyTarget::RichText),
        "<blockquote>\n<p>q</p>\n</blockquote>"
    );
}

#[test]
fn teams_and_word_keep_the_gfm_header_alignment() {
    let source = "| Default | Center | Right |\n| --- | :---: | ---: |";
    for target in [CopyTarget::Teams, CopyTarget::Word] {
        let out = html(source, target);
        for (name, align) in [
            ("Default", "left"),
            ("Center", "center"),
            ("Right", "right"),
        ] {
            assert!(
                out.contains(&format!("text-align: {align};\">{name}</th>")),
                "{out}"
            );
        }
        if target == CopyTarget::Word {
            assert_eq!(
                out.matches("background-color: rgba(127, 127, 127, 0.12);")
                    .count(),
                3
            );
        }
    }
}

#[test]
fn teams_and_word_prepare_tables_and_code_from_the_fixture() {
    for (target, font) in [
        (CopyTarget::Teams, "monospace"),
        (CopyTarget::Word, "Consolas"),
    ] {
        let rich = reps(FIXTURE, target);
        let generic = reps(FIXTURE, CopyTarget::RichText);
        assert_eq!(rich.plain, generic.plain);
        let out = rich.html.unwrap();
        assert!(out.contains("border-collapse: collapse"));
        assert!(out.contains(&format!("font-family: {font}")));
        assert!(out.contains("    publish(\"staging\")"));
        assert!(out.contains("href=\"https://example.org/rollout\""));
    }
}

#[test]
fn task_list_checkboxes_become_glyphs_for_paste_targets() {
    let markdown = "- [x] Done\n- [ ] Open";
    for target in [
        CopyTarget::SlackPaste,
        CopyTarget::Teams,
        CopyTarget::Word,
        CopyTarget::RichText,
    ] {
        let out = html(markdown, target);
        assert!(
            out.contains(">☑ Done</li>") && out.contains(">☐ Open</li>"),
            "{target}: {out}"
        );
        assert!(!out.contains("<input"), "{target}");
    }
    let source = plain(markdown, CopyTarget::HtmlSource);
    assert!(source.contains("<input type=\"checkbox\" checked disabled> Done"));
    assert!(source.contains("<input type=\"checkbox\" disabled> Open"));
    // Raw HTML checkboxes are sanitized to disabled checkboxes and shown as glyphs too.
    assert_eq!(
        html("<p><input type=\"checkbox\"> raw</p>", CopyTarget::Word),
        "<p>☐ raw</p>"
    );
}

// ---------------------------------------------------------------------------
// Limits.

#[test]
fn oversized_input_is_copied_literally_with_a_diagnostic() {
    let big = "a".repeat(MAX_INPUT_BYTES + 1);
    let result = convert(&big, CopyTarget::PlainText);
    assert_eq!(result.representations.plain, big);
    assert_eq!(
        codes(&result),
        [(
            DiagnosticCode::InputLimitExceeded,
            DiagnosticSeverity::Warning
        )]
    );
}

#[test]
fn deep_nesting_is_rejected_without_overflowing_the_stack() {
    for source in [
        ">".repeat(200_000),
        "[".repeat(100_000),
        format!("{}x", "*a ".repeat(5000)),
        (0..300).fold(String::new(), |acc, i| acc + &"  ".repeat(i) + "- a\n"),
    ] {
        for target in [
            CopyTarget::RichText,
            CopyTarget::Discord,
            CopyTarget::Telegram,
            CopyTarget::Signal,
        ] {
            let result = convert(&source, target);
            assert!(
                result
                    .diagnostics
                    .iter()
                    .any(|d| d.code == DiagnosticCode::InputLimitExceeded),
                "{target}"
            );
        }
    }
}

#[test]
fn deep_raw_html_is_flattened() {
    let source = format!("<div>{}</div>", "<b>".repeat(MAX_HTML_DEPTH * 4));
    let result = convert(&source, CopyTarget::RichText);
    assert!(
        result
            .diagnostics
            .iter()
            .any(|d| d.code == DiagnosticCode::InputLimitExceeded)
    );
    let plain_text = convert(&source, CopyTarget::PlainText);
    assert_eq!(plain_text.representations.plain, "");
}

#[test]
fn json_contract_round_trips() {
    let result = convert("**hi**", CopyTarget::Signal);
    let json = serde_json::to_value(&result).unwrap();
    assert_eq!(
        json["representations"]["text/signal"],
        "<strong>hi</strong>"
    );
    let back: MarkdownConversion = serde_json::from_value(json).unwrap();
    assert_eq!(back, result);
    assert_eq!(resolve_target("slack"), Some(CopyTarget::SlackPaste));
}

// ---------------------------------------------------------------------------
// Spoiler regressions found in review.

#[test]
fn spoilers_crossing_collapsed_emphasis_fail_closed() {
    for markdown in [
        "# **a <span data-spoiler=\"true\">Zq** c</span>",
        "**x __a <span data-spoiler=\"true\">Zq__ c</span>**",
        "*x _a <span data-spoiler=\"true\">Zq_ c</span>*",
        "~~x ~a <span data-spoiler=\"true\">Zq~ c</span>~~",
    ] {
        for target in [
            CopyTarget::Discord,
            CopyTarget::Reddit,
            CopyTarget::Telegram,
        ] {
            let result = convert(markdown, target);
            let r = &result.representations;
            for value in [Some(&r.plain), r.html.as_ref(), r.rtf.as_ref()]
                .into_iter()
                .flatten()
            {
                assert!(!value.contains("Zq"), "{target} leaked {markdown}: {value}");
            }
            assert!(
                codes(&result)
                    .contains(&(DiagnosticCode::SpoilerOmitted, DiagnosticSeverity::Warning)),
                "{target}"
            );
        }
    }
}

#[test]
fn discord_table_spoilers_stay_omitted_after_tricky_fences() {
    for prefix in ["```\n~~~\n```", "````\n```\n````", "~~~\n```\n~~~"] {
        let markdown = format!(
            "{prefix}\n\n| a | <span data-spoiler=\"true\">Zq</span> |\n|---|---|\n| c | d |"
        );
        let text = plain(&markdown, CopyTarget::Discord);
        assert!(!text.contains("Zq"), "{text}");
        assert!(text.contains("[spoiler omitted]"), "{text}");
        let reddit = plain(&markdown, CopyTarget::Reddit);
        assert!(reddit.contains(">!Zq!<"), "{reddit}");
    }
}

#[test]
fn reddit_spoilers_need_tight_content() {
    assert_eq!(
        plain("<span data-spoiler=\"true\"> x</span>", CopyTarget::Reddit),
        "[spoiler omitted]"
    );
    assert_eq!(
        plain("<span data-spoiler=\"true\">x </span>y", CopyTarget::Reddit),
        "[spoiler omitted]y"
    );
}

#[test]
fn crossing_delimiter_runs_cannot_bypass_the_depth_limit() {
    for k in [3_000, 20_000] {
        let source = format!("{}{}", " _a b*".repeat(k), " c_".repeat(k));
        let result = convert(&source, CopyTarget::RichText);
        assert!(
            result
                .diagnostics
                .iter()
                .any(|d| d.code == DiagnosticCode::InputLimitExceeded),
            "k={k}"
        );
    }
}

#[test]
fn nested_links_from_foreign_content_are_unwrapped() {
    let source = "<A>\n\n<svg><a href=\"https://example.org\">s</a></svg> t";
    let out = html(source, CopyTarget::RichText);
    assert_eq!(out.matches("<a").count(), 1, "{out}");
    assert!(out.contains('s') && out.contains('t'));
}
