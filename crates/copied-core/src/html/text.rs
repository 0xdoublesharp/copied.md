//! Plain-text extraction from HTML (`htmlText` / `htmlNodeText`).

use super::dom::{HElement, HNode, parse_fragment};

const IGNORED: &[&str] = &["script", "style", "template", "iframe", "object"];
const BLOCKS: &[&str] = &["p", "div", "section", "article", "li", "tr", "br"];

type Escape<'a> = &'a dyn Fn(&str) -> String;
type RenderLink<'a> = &'a dyn Fn(&HElement, &str) -> String;
type RenderSpan<'a> = &'a dyn Fn(&HElement, &str) -> String;
type RenderTable<'a> = &'a dyn Fn(&HElement) -> String;

/// Hooks matching `HtmlTextOptions`.
#[derive(Default, Clone, Copy)]
pub(crate) struct TextHooks<'a> {
    pub escape: Option<Escape<'a>>,
    pub link: Option<RenderLink<'a>>,
    pub span: Option<RenderSpan<'a>>,
    pub table: Option<RenderTable<'a>>,
}

impl std::fmt::Debug for TextHooks<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("TextHooks").finish_non_exhaustive()
    }
}

/// Extracts text from an HTML fragment string.
pub(crate) fn html_text(source: &str, hooks: TextHooks<'_>) -> String {
    node_text(&parse_fragment(source).children, hooks)
}

/// Extracts text from parsed nodes.
pub(crate) fn node_text(nodes: &[HNode], hooks: TextHooks<'_>) -> String {
    let mut out = String::new();
    for node in nodes {
        match node {
            HNode::Text(text) => match hooks.escape {
                Some(escape) => out.push_str(&escape(text)),
                None => out.push_str(text),
            },
            HNode::Element(element) => {
                let name = element.name.as_str();
                if IGNORED.contains(&name) {
                    continue;
                }
                if name == "table"
                    && let Some(table) = hooks.table
                {
                    out.push_str(&table(element));
                    out.push('\n');
                    continue;
                }
                let value = if name == "img" {
                    let alt = element.attr("alt").unwrap_or("");
                    hooks
                        .escape
                        .map_or_else(|| alt.to_owned(), |escape| escape(alt))
                } else {
                    node_text(&element.children, hooks)
                };
                if name == "a"
                    && let Some(link) = hooks.link
                {
                    out.push_str(&link(element, &value));
                } else if name == "span"
                    && let Some(span) = hooks.span
                {
                    out.push_str(&span(element, &value));
                } else {
                    out.push_str(&value);
                    if BLOCKS.contains(&name) {
                        out.push('\n');
                    }
                }
            }
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extracts_text_with_block_breaks() {
        let text = html_text(
            "<p>a<br>b</p><script>x</script><img alt=\"I\"><div>c</div>",
            TextHooks::default(),
        );
        assert_eq!(text, "a\nb\nIc\n");
    }

    #[test]
    fn hooks_are_debuggable() {
        assert!(format!("{:?}", TextHooks::default()).contains("TextHooks"));
    }

    #[test]
    fn applies_hooks() {
        let escape = |s: &str| s.to_uppercase();
        let link = |_: &HElement, label: &str| format!("[{label}]");
        let span = |_: &HElement, content: &str| format!("({content})");
        let table = |_: &HElement| "T".to_owned();
        let hooks = TextHooks {
            escape: Some(&escape),
            link: Some(&link),
            span: Some(&span),
            table: Some(&table),
        };
        assert_eq!(
            html_text(
                "<a>x</a><span>y</span><table><tr><td>z</td></tr></table><img alt=\"q\">",
                hooks
            ),
            "[X](Y)T\nQ"
        );
    }
}
