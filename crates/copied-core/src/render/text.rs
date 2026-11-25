//! Plain, chat-markup, Telegram (plain/RTF/HTML) and Signal text rendering
//! (`render-text.ts`).

use super::rtf::{document as rtf_document, escape_rtf_utf16};
use super::spoilers::{self, CLOSE_SPAN, OPEN_SPAN, OPEN_SPOILER, SPOILER_OPEN, signal_span_open};
use super::unicode::{to_bold, to_bold_italic, to_italic};
use crate::ast::Node;
use crate::document::footnote_numbers;
use crate::html::dom::HElement;
use crate::html::tables::{align_rows, table_as_text};
use crate::html::text::{TextHooks, html_text};
use crate::url::{is_js_whitespace, js_trim, js_trim_end};
use std::collections::{BTreeMap, HashMap};
use std::fmt::Write as _;

/// Telegram representation being rendered.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Telegram {
    Plain,
    Rtf,
    Html,
}

/// Rendering switches (`TextOptions`).
#[allow(clippy::struct_excessive_bools)]
#[derive(Debug, Clone, Copy, Default)]
pub(crate) struct TextOptions {
    pub slack_api: bool,
    pub slack_paste: bool,
    pub whatsapp: bool,
    pub decorative_unicode: bool,
    pub telegram: Option<Telegram>,
    pub signal: bool,
    /// Greenfield improvements (nested emphasis, aligned `WhatsApp` tables).
    pub improved: bool,
    /// Emit spoiler sentinels where Signal emits spoiler spans.
    pub spoilers: bool,
}

#[allow(clippy::struct_excessive_bools)]
#[derive(Debug, Clone, Copy, Default)]
struct Style {
    bold: bool,
    italic: bool,
    strike: bool,
    inside_link: bool,
}

fn html_escape(value: &str) -> String {
    value
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
}

/// `telegramHtmlLineBreaks`.
pub(crate) fn telegram_html_line_breaks(text: &str) -> String {
    let mut collapsed = String::with_capacity(text.len());
    let mut rest = text;
    while let Some(start) = rest.find('\n') {
        collapsed.push_str(&rest[..start]);
        let run = rest[start..].bytes().take_while(|&b| b == b'\n').count();
        let after = &rest[start + run..];
        let before_block = after.starts_with("<pre>") || after.starts_with("<blockquote>");
        let after_block = collapsed.ends_with("</pre>") || collapsed.ends_with("</blockquote>");
        if run >= 2 && (before_block || after_block) {
            collapsed.push('\n');
        } else {
            collapsed.push_str(&rest[start..start + run]);
        }
        rest = after;
    }
    collapsed.push_str(rest);
    collapsed.replace('\n', "<br>")
}

struct Renderer {
    opts: TextOptions,
    notes: HashMap<String, usize>,
}

impl Renderer {
    fn chat_markup(&self) -> bool {
        self.opts.slack_api || self.opts.whatsapp
    }

    fn rtf(&self) -> bool {
        self.opts.telegram == Some(Telegram::Rtf)
    }

    fn telegram_html(&self) -> bool {
        self.opts.telegram == Some(Telegram::Html)
    }

    fn html(&self) -> bool {
        self.telegram_html() || self.opts.signal
    }

    fn escape(&self, value: &str) -> String {
        if self.rtf() {
            escape_rtf_utf16(value)
        } else if self.opts.slack_api || self.html() {
            html_escape(value)
        } else {
            value.to_owned()
        }
    }

    fn styled(&self, value: &str, style: Style) -> String {
        if self.rtf() {
            let mut controls = String::new();
            if style.bold {
                controls.push_str("\\b");
            }
            if style.italic {
                controls.push_str("\\i");
            }
            if style.strike {
                controls.push_str("\\strike");
            }
            let text = self.escape(value);
            return if controls.is_empty() {
                text
            } else {
                format!("{{{controls} {text}}}")
            };
        }
        let value = self.escape(value);
        if !self.opts.decorative_unicode {
            return value;
        }
        match (style.bold, style.italic) {
            (true, true) => to_bold_italic(&value),
            (true, false) => to_bold(&value),
            (false, true) => to_italic(&value),
            (false, false) => value,
        }
    }

    fn link(&self, label: &str, url: &str) -> String {
        if label.is_empty() || label == url {
            return self.escape(url);
        }
        let fragment = url.starts_with('#');
        if self.telegram_html() && !fragment {
            let href = self.escape(url).replace('"', "&quot;");
            return format!("<a href=\"{href}\">{label}</a>");
        }
        if self.rtf() && !fragment {
            let href =
                ::url::Url::parse(url).map_or_else(|_| url.to_owned(), |u| u.as_str().to_owned());
            let destination = href
                .replace('"', "%22")
                .replace('\\', "%5C")
                .replace(' ', "%20");
            return format!(
                "{{\\field{{\\*\\fldinst HYPERLINK \"{}\"}}{{\\fldrslt {label}}}}}",
                self.escape(&destination)
            );
        }
        if self.opts.slack_api && !fragment {
            return format!("<{}|{label}>", self.escape(&url.replace('|', "%7C")));
        }
        if fragment {
            label.to_owned()
        } else {
            format!("{label} ({})", self.escape(url))
        }
    }

    fn span_markers(&self) -> bool {
        self.opts.signal || self.opts.spoilers
    }

    fn span_open(&self, element: &HElement) -> String {
        if self.opts.signal {
            signal_span_open(element).to_owned()
        } else if spoilers::is_spoiler_span(element) {
            OPEN_SPOILER.to_string()
        } else {
            OPEN_SPAN.to_string()
        }
    }

    fn span_close(&self) -> String {
        if self.opts.signal {
            "</span>".to_owned()
        } else {
            CLOSE_SPAN.to_string()
        }
    }

    fn raw_inline(&self, source: &str) -> String {
        if self.span_markers() {
            if is_span_open_tag(source) {
                let span = |element: &HElement, _: &str| self.span_open(element);
                return html_text(
                    source,
                    TextHooks {
                        span: Some(&span),
                        ..TextHooks::default()
                    },
                );
            }
            if is_span_close_tag(source) {
                return self.span_close();
            }
        }
        self.escape(&html_text(source, TextHooks::default()))
    }

    fn raw_block(&self, source: &str) -> String {
        if !self.span_markers() {
            let table = |element: &HElement| table_as_text(element);
            let hooks = if self.opts.slack_paste {
                TextHooks {
                    table: Some(&table),
                    ..TextHooks::default()
                }
            } else {
                TextHooks::default()
            };
            return js_trim_end(&self.escape(&html_text(source, hooks))).to_owned();
        }
        if is_span_close_tag(js_trim(source)) {
            return self.span_close();
        }
        let escape = |value: &str| self.escape(value);
        let span = |element: &HElement, content: &str| {
            let close = if element.explicit_close {
                self.span_close()
            } else {
                String::new()
            };
            format!("{}{content}{close}", self.span_open(element))
        };
        let text = html_text(
            source,
            TextHooks {
                escape: Some(&escape),
                span: Some(&span),
                ..TextHooks::default()
            },
        );
        js_trim_end(&text).to_owned()
    }

    fn text_node(&self, value: &str, style: Style) -> String {
        if self.span_markers()
            && let Some(start) = spoilers::partial_spoiler_start(value)
        {
            let open = if self.opts.signal {
                SPOILER_OPEN.to_owned()
            } else {
                OPEN_SPOILER.to_string()
            };
            return self.styled(&value[..start], style) + &open;
        }
        self.styled(value, style)
    }

    fn wrap_style(&self, value: String, already: bool, html_tag: &str, marker: char) -> String {
        if self.opts.improved && already && (self.html() || self.chat_markup()) {
            return value;
        }
        if self.html() {
            format!("<{html_tag}>{value}</{html_tag}>")
        } else if self.chat_markup() {
            format!("{marker}{value}{marker}")
        } else {
            value
        }
    }

    fn inline(&self, nodes: &[Node], style: Style) -> String {
        let mut out = String::new();
        for node in nodes {
            match node {
                Node::Text(value) => out.push_str(&self.text_node(value, style)),
                Node::Strong(children) => {
                    let value = self.inline(
                        children,
                        Style {
                            bold: true,
                            ..style
                        },
                    );
                    out.push_str(&self.wrap_style(value, style.bold, "strong", '*'));
                }
                Node::Emphasis(children) => {
                    let value = self.inline(
                        children,
                        Style {
                            italic: true,
                            ..style
                        },
                    );
                    out.push_str(&self.wrap_style(value, style.italic, "em", '_'));
                }
                Node::Delete(children) => {
                    let value = self.inline(
                        children,
                        Style {
                            strike: true,
                            ..style
                        },
                    );
                    out.push_str(&self.wrap_style(value, style.strike, "s", '~'));
                }
                Node::InlineCode(value) => out.push_str(&self.inline_code(value)),
                Node::Link { url, children, .. } => {
                    let label = self.inline(
                        children,
                        Style {
                            inside_link: true,
                            ..style
                        },
                    );
                    out.push_str(&self.link(&label, url));
                }
                Node::Image { url, alt, .. } => {
                    let label = self.escape(if alt.is_empty() { "Image" } else { alt });
                    if style.inside_link {
                        out.push_str(&label);
                    } else {
                        out.push_str(&self.link(&label, url));
                    }
                }
                Node::FootnoteReference { identifier, .. } => {
                    let number = self
                        .notes
                        .get(identifier)
                        .map_or_else(|| "undefined".to_owned(), ToString::to_string);
                    let _ = write!(out, "[{number}]");
                }
                Node::Html(value) => out.push_str(&self.raw_inline(value)),
                Node::Break => out.push('\n'),
                _ => {}
            }
        }
        out
    }

    fn inline_code(&self, value: &str) -> String {
        if self.opts.signal {
            format!(
                "<span data-monospace=\"true\">{}</span>",
                self.escape(value)
            )
        } else if self.html() {
            format!("<code>{}</code>", self.escape(value))
        } else if self.rtf() {
            format!("{{\\f1 {}}}", self.escape(value))
        } else if self.chat_markup() && !value.contains('`') {
            format!("`{}`", self.escape(value))
        } else {
            self.escape(value)
        }
    }

    fn blocks(&self, nodes: &[Node], separator: &str, top_level: bool) -> String {
        nodes
            .iter()
            .map(|node| self.block(node, top_level))
            .filter(|v| !v.is_empty())
            .collect::<Vec<_>>()
            .join(separator)
    }

    fn block(&self, node: &Node, top_level: bool) -> String {
        match node {
            Node::Paragraph(children) => self.inline(children, Style::default()),
            Node::Heading { children, .. } => {
                let value = self.inline(
                    children,
                    Style {
                        bold: true,
                        ..Style::default()
                    },
                );
                self.wrap_style(value, false, "strong", '*')
            }
            Node::Blockquote(children) => {
                let content = self.blocks(children, "\n\n", false);
                if self.telegram_html() {
                    return format!(
                        "<blockquote>{}</blockquote>",
                        telegram_html_line_breaks(&content)
                    );
                }
                let marker = if self.opts.signal { "&gt;" } else { ">" };
                content
                    .split('\n')
                    .map(|line| {
                        if line.is_empty() {
                            marker.to_owned()
                        } else {
                            format!("{marker} {line}")
                        }
                    })
                    .collect::<Vec<_>>()
                    .join("\n")
            }
            Node::Code { value, .. } => self.code(value),
            Node::List {
                ordered,
                start,
                children,
                ..
            } => self.list(*ordered, *start, children, top_level),
            Node::Table { children, .. } => self.table(children),
            Node::ThematicBreak => self.escape("———"),
            Node::Html(value) => self.raw_block(value),
            _ => String::new(),
        }
    }

    fn code(&self, value: &str) -> String {
        if self.opts.signal {
            format!(
                "<span data-monospace=\"true\">{}</span>",
                self.escape(value)
            )
        } else if self.html() {
            format!("<pre>{}</pre>", self.escape(value).replace('\n', "&#10;"))
        } else if self.rtf() {
            format!("{{\\f1 {}}}", self.escape(value))
        } else if self.opts.whatsapp && !value.contains("```") {
            format!("```{value}```")
        } else if self.opts.slack_api && !value.contains("```") {
            format!("```\n{}\n```", self.escape(value))
        } else {
            self.escape(value)
        }
    }

    fn list(&self, ordered: bool, start: Option<u32>, items: &[Node], top_level: bool) -> String {
        let mut rendered = Vec::with_capacity(items.len());
        for (index, item) in items.iter().enumerate() {
            let Node::ListItem {
                spread,
                checked,
                children,
            } = item
            else {
                continue;
            };
            let marker = if ordered {
                format!("{}. ", u64::from(start.unwrap_or(1)) + index as u64)
            } else if self.opts.whatsapp {
                "- ".to_owned()
            } else {
                "• ".to_owned()
            };
            let task = match checked {
                None => "",
                Some(true) => "[x] ",
                Some(false) => "[ ] ",
            };
            let content = self.blocks(children, if *spread { "\n\n" } else { "\n" }, false);
            let mut lines = content.split('\n');
            let first = lines.next().unwrap_or("");
            let indent =
                (if self.telegram_html() { "&nbsp;" } else { " " }).repeat(marker.chars().count());
            let mut value = format!("{}{task}{first}", self.escape(&marker));
            for line in lines {
                value.push('\n');
                let block_line = self.telegram_html()
                    && (line.starts_with("<pre>") || line.starts_with("<blockquote>"));
                if !line.is_empty() && !block_line {
                    value.push_str(&indent);
                }
                value.push_str(line);
            }
            rendered.push(value);
        }
        rendered.join(if self.opts.telegram.is_some() && top_level {
            "\n\n"
        } else {
            "\n"
        })
    }

    fn cells(&self, rows: &[Node], escape_pipes: bool) -> Vec<Vec<String>> {
        rows.iter()
            .map(|row| {
                row.children()
                    .map(|cells| {
                        cells
                            .iter()
                            .map(|cell| {
                                let content = self.inline(
                                    cell.children().map_or(&[], Vec::as_slice),
                                    Style::default(),
                                );
                                if escape_pipes {
                                    content.replace('|', "\\|")
                                } else {
                                    content
                                }
                            })
                            .collect()
                    })
                    .unwrap_or_default()
            })
            .collect()
    }

    fn table(&self, rows: &[Node]) -> String {
        if self.opts.improved && self.opts.whatsapp {
            let plain = Renderer {
                opts: TextOptions {
                    whatsapp: false,
                    improved: true,
                    ..TextOptions::default()
                },
                notes: self.notes.clone(),
            };
            let cells = plain.cells(rows, false);
            if !cells.iter().flatten().any(|cell| cell.contains("```")) {
                return format!("```{}```", align_rows(&cells, true).join("\n"));
            }
        }
        let escape_pipes = !(self.rtf() || self.telegram_html());
        let text = self
            .cells(rows, escape_pipes)
            .iter()
            .map(|row| row.join(" | "))
            .collect::<Vec<_>>()
            .join("\n");
        if self.opts.slack_api {
            format!("```\n{text}\n```")
        } else {
            text
        }
    }
}

fn is_span_open_tag(source: &str) -> bool {
    source.len() >= 5
        && source.is_char_boundary(5)
        && source[..5].eq_ignore_ascii_case("<span")
        && source[5..]
            .chars()
            .next()
            .is_some_and(|c| c == '>' || is_js_whitespace(c))
}

fn is_span_close_tag(source: &str) -> bool {
    source.len() >= 7
        && source.is_char_boundary(6)
        && source[..6].eq_ignore_ascii_case("</span")
        && source.ends_with('>')
        && source[6..source.len() - 1].chars().all(is_js_whitespace)
}

/// `renderText`.
pub(crate) fn render_text(children: &[Node], opts: TextOptions) -> String {
    let renderer = Renderer {
        opts,
        notes: footnote_numbers(children),
    };
    let mut definitions: BTreeMap<usize, String> = BTreeMap::new();
    crate::ast::walk(children, &mut |node| {
        if let Node::FootnoteDefinition {
            identifier,
            children,
        } = node
            && let Some(&number) = renderer.notes.get(identifier)
            && !definitions.contains_key(&number)
        {
            definitions.insert(
                number,
                format!("[{number}] {}", renderer.blocks(children, "\n\n", false)),
            );
        }
    });
    let text = std::iter::once(renderer.blocks(children, "\n\n", true))
        .chain(definitions.into_values())
        .filter(|v| !v.is_empty())
        .collect::<Vec<_>>()
        .join("\n\n");
    if opts.signal {
        return text.replace('\n', "<br>");
    }
    match opts.telegram {
        Some(Telegram::Html) => telegram_html_line_breaks(&text),
        Some(Telegram::Rtf) => rtf_document(&text),
        _ => text,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn collapses_telegram_block_breaks() {
        assert_eq!(
            telegram_html_line_breaks("a\n\n<pre>x</pre>\n\n\nb\nc"),
            "a<br><pre>x</pre><br>b<br>c"
        );
        assert_eq!(
            telegram_html_line_breaks("<blockquote>q</blockquote>\n\n<blockquote>r</blockquote>"),
            "<blockquote>q</blockquote><br><blockquote>r</blockquote>"
        );
    }

    #[test]
    fn recognizes_span_tags() {
        assert!(is_span_open_tag("<SPAN class=x>"));
        assert!(is_span_open_tag("<span>"));
        assert!(!is_span_open_tag("<spanx>"));
        assert!(is_span_close_tag("</SPAN >"));
        assert!(!is_span_close_tag("</span x>"));
        assert!(!is_span_close_tag("</sp"));
    }
}
