//! Rich HTML rendering (`rich-html.ts`): mdast → HTML → sanitized HTML with
//! destination-specific post-processing.

pub(crate) mod dom;
pub(crate) mod sanitize;
pub(crate) mod serialize;
pub(crate) mod tables;
pub(crate) mod text;
pub(crate) mod to_hast;
pub(crate) mod to_markdown;

use crate::ast::Node;
use dom::{HElement, HNode};

/// Rich paste destinations with post-sanitization adjustments.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Destination {
    RichText,
    SlackPaste,
    Teams,
    Word,
}

/// Output of a rich HTML render.
#[derive(Debug, Clone)]
pub(crate) struct RichHtml {
    pub html: String,
    pub depth_limited: bool,
}

fn destination_style(destination: Destination, tag: &str, improved: bool) -> Option<&'static str> {
    if !improved {
        return match (destination, tag) {
            (Destination::Teams, "pre") => Some("font-family: monospace; white-space: pre-wrap;"),
            (Destination::Teams, "code") => Some("font-family: monospace;"),
            (Destination::Word, "pre") => {
                Some("font-family: Consolas, monospace; white-space: pre;")
            }
            (Destination::Word, "code") => Some("font-family: Consolas, monospace;"),
            (Destination::Teams, "th") => {
                Some("border: 1px solid #999; padding: 4px 8px; text-align: left;")
            }
            (Destination::Word, "th") => Some(
                "border: 1px solid #999; background-color: #eaeaea; padding: 4pt 6pt; text-align: left;",
            ),
            (_, "blockquote") => None,
            _ => destination_style(destination, tag, true),
        };
    }
    match (destination, tag) {
        (Destination::Teams, "pre") => Some(
            "font-family: monospace; white-space: pre-wrap; background-color: rgba(127, 127, 127, 0.14);",
        ),
        (Destination::Teams, "code") => {
            Some("font-family: monospace; background-color: rgba(127, 127, 127, 0.14);")
        }
        (Destination::Teams, "table") => Some("border-collapse: collapse;"),
        (Destination::Teams, "th" | "td") => Some("border: 1px solid #999; padding: 4px 8px;"),
        (Destination::Word, "pre") => Some(
            "font-family: Consolas, monospace; white-space: pre; background-color: rgba(127, 127, 127, 0.14);",
        ),
        (Destination::Word, "code") => {
            Some("font-family: Consolas, monospace; background-color: rgba(127, 127, 127, 0.14);")
        }
        (Destination::Word, "table") => Some("border-collapse: collapse; border: 1px solid #999;"),
        (Destination::Word, "th") => Some(
            "border: 1px solid #999; background-color: rgba(127, 127, 127, 0.12); padding: 4pt 6pt;",
        ),
        (Destination::Word, "td") => Some("border: 1px solid #999; padding: 4pt 6pt;"),
        (Destination::Teams | Destination::Word, "blockquote") => Some(
            "border-left: 3px solid rgba(127, 127, 127, 0.45); margin: 8px 0; padding-left: 12px; opacity: 0.85;",
        ),
        _ => None,
    }
}

fn apply_styles(nodes: &mut [HNode], destination: Destination, improved: bool) {
    for node in nodes {
        if let HNode::Element(element) = node {
            if let Some(style) = destination_style(destination, &element.name, improved) {
                let style = if improved && element.name == "th" {
                    let align = match element.attr("align") {
                        Some("center") => "center",
                        Some("right") => "right",
                        _ => "left",
                    };
                    format!("{style} text-align: {align};")
                } else {
                    style.to_owned()
                };
                element.set_attr("style", style);
            }
            apply_styles(&mut element.children, destination, improved);
        }
    }
}

fn tables_to_code(nodes: &mut [HNode], improved: bool) {
    for node in nodes {
        if let HNode::Element(element) = node {
            if element.name == "table" {
                let text = if improved {
                    tables::aligned_table_text(element)
                } else {
                    tables::table_as_text(element)
                };
                let code = HElement::new("code", vec![], vec![HNode::Text(text)]);
                *element = HElement::new("pre", vec![], vec![HNode::Element(code)]);
            } else {
                tables_to_code(&mut element.children, improved);
            }
        }
    }
}

/// Unwraps links nested inside links. Foreign content (an SVG `<a>`) is not
/// auto-closed by the HTML parser, so an unclosed raw `<a>` could otherwise
/// wrap another anchor after sanitization unwraps the `<svg>`.
fn unnest_links(nodes: &mut Vec<HNode>, inside_link: bool) {
    let mut index = 0;
    while index < nodes.len() {
        if let HNode::Element(element) = &mut nodes[index] {
            let is_link = element.name == "a";
            unnest_links(&mut element.children, inside_link || is_link);
            if inside_link && is_link {
                let children = std::mem::take(&mut element.children);
                let count = children.len();
                nodes.splice(index..=index, children);
                index += count;
                continue;
            }
        }
        index += 1;
    }
}

/// Replaces task-list checkboxes with visible glyphs (apps strip `<input>` on paste).
fn checkboxes_to_glyphs(nodes: &mut Vec<HNode>) {
    let mut index = 0;
    while index < nodes.len() {
        if let HNode::Element(element) = &mut nodes[index] {
            if element.name == "input" {
                let glyph = if element.attr("checked").is_some() {
                    "☑"
                } else {
                    "☐"
                };
                let spaced = matches!(nodes.get(index + 1), Some(HNode::Text(t)) if t.starts_with(' '))
                    || index + 1 == nodes.len();
                let value = if spaced {
                    glyph.to_owned()
                } else {
                    format!("{glyph} ")
                };
                nodes[index] = HNode::Text(value);
                // Merge adjacent text so the serializer sees one run.
                if let Some(HNode::Text(next)) = nodes.get(index + 1).cloned() {
                    nodes.remove(index + 1);
                    if let HNode::Text(current) = &mut nodes[index] {
                        current.push_str(&next);
                    }
                }
                if index > 0
                    && let (HNode::Text(_), HNode::Text(current)) =
                        (&nodes[index - 1], nodes[index].clone())
                {
                    nodes.remove(index);
                    index -= 1;
                    if let HNode::Text(previous) = &mut nodes[index] {
                        previous.push_str(&current);
                    }
                }
            } else {
                checkboxes_to_glyphs(&mut element.children);
            }
        }
        index += 1;
    }
}

/// Renders sanitized rich HTML for a document.
pub(crate) fn render_rich_html(
    children: &[Node],
    source_url: Option<&str>,
    destination: Option<Destination>,
    improved: bool,
) -> RichHtml {
    let fragment = to_hast::to_html_tree(children);
    render_safe(
        fragment.children,
        source_url,
        destination,
        improved,
        fragment.depth_limited,
    )
}

fn render_safe(
    mut nodes: Vec<HNode>,
    source_url: Option<&str>,
    destination: Option<Destination>,
    improved: bool,
    depth_limited: bool,
) -> RichHtml {
    sanitize::prepare_links(&mut nodes, source_url);
    // Converting Slack tables before sanitizing keeps captions, which the schema
    // would otherwise unwrap into loose text.
    if improved && destination == Some(Destination::SlackPaste) {
        tables_to_code(&mut nodes, true);
    }
    let mut clean = sanitize::sanitize(nodes);
    sanitize::repair_clobbered_fragments(&mut clean);
    if improved {
        unnest_links(&mut clean, false);
    }
    match destination {
        Some(Destination::SlackPaste) => tables_to_code(&mut clean, improved),
        Some(destination @ (Destination::Teams | Destination::Word)) => {
            apply_styles(&mut clean, destination, improved);
        }
        Some(Destination::RichText) | None => {}
    }
    if improved && destination.is_some() {
        checkboxes_to_glyphs(&mut clean);
    }
    RichHtml {
        html: serialize::serialize(&clean),
        depth_limited,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn checkbox_glyphs_do_not_double_spaces() {
        let mut nodes = vec![
            HNode::Text("a".into()),
            HNode::Element(HElement::new(
                "input",
                vec![("checked".into(), String::new())],
                vec![],
            )),
            HNode::Text(" Done".into()),
        ];
        checkboxes_to_glyphs(&mut nodes);
        assert_eq!(nodes, vec![HNode::Text("a☑ Done".into())]);
        let mut lone = vec![
            HNode::Element(HElement::new("input", vec![], vec![])),
            HNode::Element(HElement::new("b", vec![], vec![])),
        ];
        checkboxes_to_glyphs(&mut lone);
        assert_eq!(lone[0], HNode::Text("☐ ".into()));
    }
}
