//! Markdown → destination-specific clipboard conversion for copied.md.
//!
//! [`convert_markdown`] parses GitHub Flavored Markdown and renders it for one of
//! the 15 [`TARGETS`]. Output matches the `ts-oracle-v1` TypeScript engine except
//! for the documented greenfield improvements in `tests/parity/divergences.toml`.

use serde::{Deserialize, Serialize};
use std::borrow::Cow;
use std::fmt;

mod ast;
mod convert;
mod document;
mod html;
pub mod limits;
mod render;
mod url;

pub use html::to_markdown::html_to_markdown;
pub use render::unicode::{to_bold, to_bold_italic, to_italic};
pub use render::xlen::weighted_length as x_weighted_length;
pub use url::safe_url;

/// All supported copy destinations.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum CopyTarget {
    RichText,
    PlainText,
    HtmlSource,
    SlackPaste,
    Discord,
    Telegram,
    Signal,
    Reddit,
    Facebook,
    Twitter,
    Sms,
    SlackApi,
    Teams,
    Word,
    Whatsapp,
}

impl CopyTarget {
    pub const fn id(self) -> &'static str {
        match self {
            Self::RichText => "rich-text",
            Self::PlainText => "plain-text",
            Self::HtmlSource => "html-source",
            Self::SlackPaste => "slack-paste",
            Self::Discord => "discord",
            Self::Telegram => "telegram",
            Self::Signal => "signal",
            Self::Reddit => "reddit",
            Self::Facebook => "facebook",
            Self::Twitter => "twitter",
            Self::Sms => "sms",
            Self::SlackApi => "slack-api",
            Self::Teams => "teams",
            Self::Word => "word",
            Self::Whatsapp => "whatsapp",
        }
    }

    pub const fn definition(self) -> &'static CopyTargetDefinition {
        match self {
            Self::RichText => &TARGETS[0],
            Self::PlainText => &TARGETS[1],
            Self::HtmlSource => &TARGETS[2],
            Self::SlackPaste => &TARGETS[3],
            Self::Discord => &TARGETS[4],
            Self::Telegram => &TARGETS[5],
            Self::Signal => &TARGETS[6],
            Self::Reddit => &TARGETS[7],
            Self::Facebook => &TARGETS[8],
            Self::Twitter => &TARGETS[9],
            Self::Sms => &TARGETS[10],
            Self::SlackApi => &TARGETS[11],
            Self::Teams => &TARGETS[12],
            Self::Word => &TARGETS[13],
            Self::Whatsapp => &TARGETS[14],
        }
    }
}

impl fmt::Display for CopyTarget {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.id())
    }
}

/// Static catalog entry for a copy target.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CopyTargetDefinition {
    pub id: CopyTarget,
    pub label: Cow<'static, str>,
    pub rich: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub enabled_by_default: Option<bool>,
}

/// Canonical ordered catalog of all 15 copy targets.
pub const TARGETS: &[CopyTargetDefinition] = &[
    CopyTargetDefinition {
        id: CopyTarget::RichText,
        label: Cow::Borrowed("Rich Text"),
        rich: true,
        enabled_by_default: None,
    },
    CopyTargetDefinition {
        id: CopyTarget::PlainText,
        label: Cow::Borrowed("Plain Text"),
        rich: false,
        enabled_by_default: None,
    },
    CopyTargetDefinition {
        id: CopyTarget::HtmlSource,
        label: Cow::Borrowed("HTML Source"),
        rich: false,
        enabled_by_default: None,
    },
    CopyTargetDefinition {
        id: CopyTarget::SlackPaste,
        label: Cow::Borrowed("Slack"),
        rich: true,
        enabled_by_default: None,
    },
    CopyTargetDefinition {
        id: CopyTarget::Discord,
        label: Cow::Borrowed("Discord"),
        rich: false,
        enabled_by_default: None,
    },
    CopyTargetDefinition {
        id: CopyTarget::Telegram,
        label: Cow::Borrowed("Telegram"),
        rich: true,
        enabled_by_default: None,
    },
    CopyTargetDefinition {
        id: CopyTarget::Signal,
        label: Cow::Borrowed("Signal"),
        rich: true,
        enabled_by_default: None,
    },
    CopyTargetDefinition {
        id: CopyTarget::Reddit,
        label: Cow::Borrowed("Reddit"),
        rich: false,
        enabled_by_default: None,
    },
    CopyTargetDefinition {
        id: CopyTarget::Facebook,
        label: Cow::Borrowed("Facebook"),
        rich: false,
        enabled_by_default: None,
    },
    CopyTargetDefinition {
        id: CopyTarget::Twitter,
        label: Cow::Borrowed("X"),
        rich: false,
        enabled_by_default: None,
    },
    CopyTargetDefinition {
        id: CopyTarget::Sms,
        label: Cow::Borrowed("SMS"),
        rich: false,
        enabled_by_default: None,
    },
    CopyTargetDefinition {
        id: CopyTarget::SlackApi,
        label: Cow::Borrowed("Slack API (mrkdwn)"),
        rich: false,
        enabled_by_default: None,
    },
    CopyTargetDefinition {
        id: CopyTarget::Teams,
        label: Cow::Borrowed("Teams"),
        rich: true,
        enabled_by_default: Some(false),
    },
    CopyTargetDefinition {
        id: CopyTarget::Word,
        label: Cow::Borrowed("Word"),
        rich: true,
        enabled_by_default: Some(false),
    },
    CopyTargetDefinition {
        id: CopyTarget::Whatsapp,
        label: Cow::Borrowed("WhatsApp"),
        rich: false,
        enabled_by_default: Some(false),
    },
];

/// Resolves a canonical target ID or legacy alias (`html` -> `rich-text`, `slack` -> `slack-paste`).
pub fn resolve_target(value: &str) -> Option<CopyTarget> {
    match value {
        "html" | "rich-text" => Some(CopyTarget::RichText),
        "plain-text" => Some(CopyTarget::PlainText),
        "html-source" => Some(CopyTarget::HtmlSource),
        "slack" | "slack-paste" => Some(CopyTarget::SlackPaste),
        "discord" => Some(CopyTarget::Discord),
        "telegram" => Some(CopyTarget::Telegram),
        "signal" => Some(CopyTarget::Signal),
        "reddit" => Some(CopyTarget::Reddit),
        "facebook" => Some(CopyTarget::Facebook),
        "twitter" => Some(CopyTarget::Twitter),
        "sms" => Some(CopyTarget::Sms),
        "slack-api" => Some(CopyTarget::SlackApi),
        "teams" => Some(CopyTarget::Teams),
        "word" => Some(CopyTarget::Word),
        "whatsapp" => Some(CopyTarget::Whatsapp),
        _ => None,
    }
}

/// Options controlling Markdown conversion.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ConversionOptions {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source_url: Option<String>,
    #[serde(default)]
    pub decorative_unicode: bool,
}

/// MIME-keyed clipboard payload matching the `schemaVersion: 1` JSON contract.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct ClipboardPayload {
    #[serde(rename = "text/plain")]
    pub plain: String,
    #[serde(rename = "text/html", default, skip_serializing_if = "Option::is_none")]
    pub html: Option<String>,
    /// Legacy RTF representation produced by the TypeScript oracle.
    #[serde(rename = "text/rtf", default, skip_serializing_if = "Option::is_none")]
    pub rtf: Option<String>,
    #[serde(
        rename = "text/signal",
        default,
        skip_serializing_if = "Option::is_none"
    )]
    pub signal: Option<String>,
}

/// Measurement unit reported in [`ConversionMetrics`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum MetricUnit {
    CodePoints,
    XWeightedCharacters,
}

/// Character and length metrics for a conversion result.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ConversionMetrics {
    pub source_code_units: usize,
    pub output_length: usize,
    pub unit: MetricUnit,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub limit: Option<usize>,
}

/// Diagnostic code emitted during conversion.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum DiagnosticCode {
    UnsupportedUrl,
    RelativeUrl,
    FormattingOmitted,
    LengthExceeded,
    SpoilerOmitted,
    InputLimitExceeded,
}

/// Diagnostic severity level.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum DiagnosticSeverity {
    Warning,
    Info,
}

/// Actionable conversion diagnostic.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Diagnostic {
    pub code: DiagnosticCode,
    pub severity: DiagnosticSeverity,
    pub message: String,
}

/// Full conversion result matching the `schemaVersion: 1` JSON contract.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MarkdownConversion {
    pub schema_version: u8,
    pub target: CopyTarget,
    pub representations: ClipboardPayload,
    pub metrics: ConversionMetrics,
    pub diagnostics: Vec<Diagnostic>,
}

/// Converts Markdown into destination-specific clipboard representations.
pub fn convert_markdown(
    markdown: &str,
    target: CopyTarget,
    options: &ConversionOptions,
) -> MarkdownConversion {
    convert::convert(markdown, target, options, true)
}

/// Compatibility entry points for parity testing.
#[doc(hidden)]
pub mod oracle {
    use super::{ConversionOptions, CopyTarget, MarkdownConversion};

    /// Converts exactly like the frozen `ts-oracle-v1` TypeScript engine, without
    /// the greenfield destination improvements.
    pub fn convert_markdown_ts_v1(
        markdown: &str,
        target: CopyTarget,
        options: &ConversionOptions,
    ) -> MarkdownConversion {
        super::convert::convert(markdown, target, options, false)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn target_catalog_matches_contract() {
        assert_eq!(TARGETS.len(), 15);
        assert_eq!(resolve_target("html"), Some(CopyTarget::RichText));
        assert_eq!(resolve_target("slack"), Some(CopyTarget::SlackPaste));
        assert_eq!(resolve_target("missing"), None);
        assert_eq!(CopyTarget::Whatsapp.to_string(), "whatsapp");
        for def in TARGETS {
            assert_eq!(resolve_target(def.id.id()), Some(def.id));
            assert_eq!(def.id.definition(), def);
        }
    }

    #[test]
    fn json_schema_matches_ts_v1_contract() {
        let res = convert_markdown("hi", CopyTarget::RichText, &ConversionOptions::default());
        let val = serde_json::to_value(&res).unwrap();
        assert_eq!(val["schemaVersion"], 1);
        assert_eq!(val["target"], "rich-text");
        assert_eq!(val["representations"]["text/plain"], "hi");
        assert_eq!(val["representations"]["text/html"], "<p>hi</p>");
        assert_eq!(val["metrics"]["sourceCodeUnits"], 2);
        assert_eq!(val["metrics"]["outputLength"], 2);
        assert_eq!(val["metrics"]["unit"], "code-points");
        assert!(val["metrics"].get("limit").is_none());
    }
}
