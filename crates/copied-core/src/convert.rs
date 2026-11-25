//! `convertMarkdown` (`conversion.ts`).

use crate::document::{has_images, read_markdown};
use crate::html::{Destination, render_rich_html};
use crate::render::markdown::{Flavor, MarkdownOptions, render_markdown};
use crate::render::spoilers::{self, Surface, neutralize, signal_spoiler_fallback};
use crate::render::text::{Telegram, TextOptions, render_text};
use crate::render::xlen::{X_LIMIT, weighted_length};
use crate::{
    ClipboardPayload, ConversionMetrics, ConversionOptions, CopyTarget, Diagnostic, DiagnosticCode, DiagnosticSeverity,
    MarkdownConversion, MetricUnit,
};

fn diagnostic(code: DiagnosticCode, severity: DiagnosticSeverity, message: &str) -> Diagnostic {
    Diagnostic { code, severity, message: message.to_owned() }
}

fn spoiler_diagnostic(malformed: bool, app: &str) -> Diagnostic {
    if malformed {
        diagnostic(DiagnosticCode::SpoilerOmitted, DiagnosticSeverity::Warning, "A malformed spoiler was omitted.")
    } else {
        let message = format!("{app} can't show this spoiler here, so it was omitted.");
        diagnostic(DiagnosticCode::SpoilerOmitted, DiagnosticSeverity::Info, &message)
    }
}

/// Converts Markdown for `target`. `improved` selects the greenfield behavior;
/// `false` reproduces `ts-oracle-v1` byte for byte.
#[allow(clippy::too_many_lines)]
pub(crate) fn convert(markdown: &str, target: CopyTarget, options: &ConversionOptions, improved: bool) -> MarkdownConversion {
    let definition = target.definition();
    let spoiler_target = improved && matches!(target, CopyTarget::Telegram | CopyTarget::Discord | CopyTarget::Reddit);
    let source = if spoiler_target { neutralize(markdown) } else { std::borrow::Cow::Borrowed(markdown) };
    let source_url = options.source_url.as_deref();
    let document = read_markdown(&source, source_url);
    let children = &document.children;
    let mut diagnostics = document.diagnostics.clone();
    let images = has_images(children);

    let malformed = spoiler_target
        && signal_spoiler_fallback(&render_text(children, TextOptions { signal: true, improved, ..TextOptions::default() }))
            .is_some_and(|fallback| fallback.malformed);

    let mut representations = ClipboardPayload::default();
    let plain = match target {
        CopyTarget::Discord | CopyTarget::Reddit => {
            let flavor = if target == CopyTarget::Discord { Flavor::Discord } else { Flavor::Reddit };
            let raw = render_markdown(children, MarkdownOptions { flavor, improved, spoilers: spoiler_target });
            if spoiler_target {
                let surface = if flavor == Flavor::Discord { Surface::Discord } else { Surface::Reddit };
                let resolved = spoilers::resolve(&raw, surface, !malformed);
                if resolved.regions > 0 && (malformed || resolved.omitted > 0) {
                    let app = if flavor == Flavor::Discord { "Discord" } else { "Reddit" };
                    diagnostics.push(spoiler_diagnostic(malformed, app));
                }
                resolved.text
            } else {
                raw
            }
        }
        _ => {
            let raw = render_text(
                children,
                TextOptions {
                    slack_api: target == CopyTarget::SlackApi,
                    slack_paste: target == CopyTarget::SlackPaste,
                    whatsapp: target == CopyTarget::Whatsapp,
                    decorative_unicode: options.decorative_unicode
                        && matches!(target, CopyTarget::Facebook | CopyTarget::Twitter | CopyTarget::Telegram),
                    telegram: (target == CopyTarget::Telegram).then_some(Telegram::Plain),
                    improved,
                    spoilers: spoiler_target,
                    ..TextOptions::default()
                },
            );
            if spoiler_target { spoilers::resolve(&raw, Surface::Plain, false).text } else { raw }
        }
    };
    representations.plain = plain;

    if target == CopyTarget::Telegram {
        let telegram = |mode| TextOptions { telegram: Some(mode), improved, spoilers: spoiler_target, ..TextOptions::default() };
        if !representations.plain.is_empty() {
            let rtf = render_text(children, telegram(Telegram::Rtf));
            representations.rtf = Some(if spoiler_target { spoilers::resolve(&rtf, Surface::Rtf, false).text } else { rtf });
        }
        let html = render_text(children, telegram(Telegram::Html));
        if spoiler_target {
            let resolved = spoilers::resolve(&html, Surface::TelegramHtml, !malformed);
            if resolved.regions > 0 {
                diagnostics.push(if malformed {
                    spoiler_diagnostic(true, "Telegram")
                } else {
                    diagnostic(
                        DiagnosticCode::SpoilerOmitted,
                        DiagnosticSeverity::Info,
                        "Plain-text and RTF paste omit Telegram spoilers.",
                    )
                });
            }
            representations.html = Some(resolved.text);
        } else {
            representations.html = Some(html);
        }
    } else if target == CopyTarget::Signal {
        let html = render_text(children, TextOptions { signal: true, improved, ..TextOptions::default() });
        let fallback = signal_spoiler_fallback(&html);
        if let Some(fallback) = &fallback {
            representations.plain.clone_from(&fallback.plain);
            diagnostics.push(if fallback.malformed {
                diagnostic(DiagnosticCode::SpoilerOmitted, DiagnosticSeverity::Warning, "A malformed Signal spoiler was omitted.")
            } else {
                diagnostic(DiagnosticCode::SpoilerOmitted, DiagnosticSeverity::Info, "Plain-text paste omits Signal spoilers.")
            });
        }
        if !fallback.is_some_and(|f| f.malformed) {
            representations.signal = Some(html);
        }
    } else if definition.rich || target == CopyTarget::HtmlSource {
        let destination = match target {
            CopyTarget::Teams => Some(Destination::Teams),
            CopyTarget::Word => Some(Destination::Word),
            CopyTarget::SlackPaste => Some(Destination::SlackPaste),
            _ => None,
        };
        let rich = render_rich_html(children, source_url, destination, improved);
        if rich.depth_limited && !diagnostics.iter().any(|d| d.code == DiagnosticCode::InputLimitExceeded) {
            diagnostics.push(diagnostic(
                DiagnosticCode::InputLimitExceeded,
                DiagnosticSeverity::Warning,
                "Deeply nested HTML was flattened.",
            ));
        }
        if target == CopyTarget::HtmlSource {
            representations.plain = rich.html;
        } else {
            representations.html = Some(rich.html);
        }
    }

    if images && (definition.rich || target == CopyTarget::HtmlSource) {
        diagnostics.push(diagnostic(DiagnosticCode::FormattingOmitted, DiagnosticSeverity::Info, "Images are represented as links."));
    }

    let output = &representations.plain;
    let weighted = (target == CopyTarget::Twitter).then(|| weighted_length(output));
    if weighted.is_some_and(|w| w > X_LIMIT) {
        diagnostics.push(diagnostic(
            DiagnosticCode::LengthExceeded,
            DiagnosticSeverity::Warning,
            "This exceeds the standard X post limit of 280 weighted characters.",
        ));
    }

    MarkdownConversion {
        schema_version: 1,
        target,
        metrics: ConversionMetrics {
            source_code_units: markdown.encode_utf16().count(),
            output_length: weighted.unwrap_or_else(|| output.chars().count()),
            unit: if weighted.is_some() { MetricUnit::XWeightedCharacters } else { MetricUnit::CodePoints },
            limit: weighted.map(|_| X_LIMIT),
        },
        representations,
        diagnostics,
    }
}
