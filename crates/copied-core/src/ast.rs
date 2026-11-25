//! Owned, mutable mdast subset used by every renderer.
//!
//! `markdown-rs` produces an mdast that mirrors `mdast-util-from-markdown`; this
//! module copies the parts the converters need into a smaller enum so the
//! document pass can resolve references in place.

use markdown::mdast;

/// GFM table column alignment.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Align {
    None,
    Left,
    Right,
    Center,
}

impl Align {
    pub(crate) fn as_str(self) -> Option<&'static str> {
        match self {
            Self::None => None,
            Self::Left => Some("left"),
            Self::Right => Some("right"),
            Self::Center => Some("center"),
        }
    }
}

/// mdast node.
#[derive(Debug, Clone, PartialEq)]
pub(crate) enum Node {
    Paragraph(Vec<Node>),
    Heading {
        depth: u8,
        children: Vec<Node>,
    },
    ThematicBreak,
    Blockquote(Vec<Node>),
    List {
        ordered: bool,
        start: Option<u32>,
        spread: bool,
        children: Vec<Node>,
    },
    ListItem {
        spread: bool,
        checked: Option<bool>,
        children: Vec<Node>,
    },
    Html(String),
    Code {
        value: String,
        lang: Option<String>,
        meta: Option<String>,
    },
    Definition {
        identifier: String,
        url: String,
        title: Option<String>,
    },
    Text(String),
    Emphasis(Vec<Node>),
    Strong(Vec<Node>),
    Delete(Vec<Node>),
    InlineCode(String),
    Break,
    Link {
        url: String,
        title: Option<String>,
        children: Vec<Node>,
    },
    Image {
        url: String,
        title: Option<String>,
        alt: String,
    },
    /// Resolved into `Link` (or its children) by the document pass.
    LinkReference {
        identifier: String,
        children: Vec<Node>,
    },
    /// Resolved into `Image` (or text) by the document pass.
    ImageReference {
        identifier: String,
        alt: String,
    },
    FootnoteDefinition {
        identifier: String,
        children: Vec<Node>,
    },
    FootnoteReference {
        identifier: String,
    },
    Table {
        align: Vec<Align>,
        children: Vec<Node>,
    },
    TableRow(Vec<Node>),
    TableCell(Vec<Node>),
}

impl Node {
    pub(crate) fn children(&self) -> Option<&Vec<Node>> {
        match self {
            Self::Paragraph(c)
            | Self::Blockquote(c)
            | Self::Emphasis(c)
            | Self::Strong(c)
            | Self::Delete(c)
            | Self::TableRow(c)
            | Self::TableCell(c)
            | Self::Heading { children: c, .. }
            | Self::List { children: c, .. }
            | Self::ListItem { children: c, .. }
            | Self::Link { children: c, .. }
            | Self::LinkReference { children: c, .. }
            | Self::FootnoteDefinition { children: c, .. }
            | Self::Table { children: c, .. } => Some(c),
            _ => None,
        }
    }

    pub(crate) fn children_mut(&mut self) -> Option<&mut Vec<Node>> {
        match self {
            Self::Paragraph(c)
            | Self::Blockquote(c)
            | Self::Emphasis(c)
            | Self::Strong(c)
            | Self::Delete(c)
            | Self::TableRow(c)
            | Self::TableCell(c)
            | Self::Heading { children: c, .. }
            | Self::List { children: c, .. }
            | Self::ListItem { children: c, .. }
            | Self::Link { children: c, .. }
            | Self::LinkReference { children: c, .. }
            | Self::FootnoteDefinition { children: c, .. }
            | Self::Table { children: c, .. } => Some(c),
            _ => None,
        }
    }
}

/// Visits nodes in preorder (document order).
pub(crate) fn walk<'a>(nodes: &'a [Node], visit: &mut dyn FnMut(&'a Node)) {
    for node in nodes {
        visit(node);
        if let Some(children) = node.children() {
            walk(children, visit);
        }
    }
}

/// Maximum depth of a `markdown-rs` tree, measured iteratively before conversion.
pub(crate) fn mdast_depth(node: &mdast::Node) -> usize {
    let mut max = 0;
    let mut stack = vec![(node, 1)];
    while let Some((node, level)) = stack.pop() {
        max = max.max(level);
        if let Some(children) = node.children() {
            stack.extend(children.iter().map(|child| (child, level + 1)));
        }
    }
    max
}

/// Last content line of a node; lists and items in `markdown-rs` extend over
/// trailing blank lines, so use their last child instead.
fn content_end_line(node: &mdast::Node) -> Option<usize> {
    match node {
        mdast::Node::List(_) | mdast::Node::ListItem(_) | mdast::Node::Blockquote(_) => node
            .children()
            .and_then(|children| children.last())
            .map_or_else(|| node.position().map(|p| p.end.line), content_end_line),
        _ => node.position().map(|p| p.end.line),
    }
}

/// `mdast-util-from-markdown` marks an item spread only when a blank line sits
/// between two of its direct children; `markdown-rs` also counts trailing blank
/// lines, so recompute it from positions.
fn item_spread(item: &mdast::ListItem) -> bool {
    let lines: Option<Vec<(usize, usize)>> = item
        .children
        .iter()
        .map(|c| Some((c.position()?.start.line, content_end_line(c)?)))
        .collect();
    lines.map_or(item.spread, |lines| {
        lines.windows(2).any(|pair| pair[1].0 > pair[0].1 + 1)
    })
}

fn convert_all(nodes: &[mdast::Node]) -> Vec<Node> {
    nodes.iter().filter_map(convert).collect()
}

/// Converts a `markdown-rs` node into the internal representation.
pub(crate) fn convert(node: &mdast::Node) -> Option<Node> {
    Some(match node {
        mdast::Node::Paragraph(n) => Node::Paragraph(convert_all(&n.children)),
        mdast::Node::Heading(n) => Node::Heading {
            depth: n.depth,
            children: convert_all(&n.children),
        },
        mdast::Node::ThematicBreak(_) => Node::ThematicBreak,
        mdast::Node::Blockquote(n) => Node::Blockquote(convert_all(&n.children)),
        mdast::Node::List(n) => Node::List {
            ordered: n.ordered,
            start: n.start,
            spread: n.spread,
            children: convert_all(&n.children),
        },
        mdast::Node::ListItem(n) => Node::ListItem {
            spread: item_spread(n),
            checked: n.checked,
            children: convert_all(&n.children),
        },
        mdast::Node::Html(n) => Node::Html(n.value.clone()),
        mdast::Node::Code(n) => Node::Code {
            value: n.value.clone(),
            lang: n.lang.clone(),
            meta: n.meta.clone(),
        },
        mdast::Node::Definition(n) => Node::Definition {
            identifier: n.identifier.clone(),
            url: n.url.clone(),
            title: n.title.clone(),
        },
        mdast::Node::Text(n) => Node::Text(n.value.clone()),
        mdast::Node::Emphasis(n) => Node::Emphasis(convert_all(&n.children)),
        mdast::Node::Strong(n) => Node::Strong(convert_all(&n.children)),
        mdast::Node::Delete(n) => Node::Delete(convert_all(&n.children)),
        mdast::Node::InlineCode(n) => Node::InlineCode(n.value.clone()),
        mdast::Node::Break(_) => Node::Break,
        mdast::Node::Link(n) => Node::Link {
            url: n.url.clone(),
            title: n.title.clone(),
            children: convert_all(&n.children),
        },
        mdast::Node::Image(n) => Node::Image {
            url: n.url.clone(),
            title: n.title.clone(),
            alt: n.alt.clone(),
        },
        mdast::Node::LinkReference(n) => Node::LinkReference {
            identifier: n.identifier.clone(),
            children: convert_all(&n.children),
        },
        mdast::Node::ImageReference(n) => Node::ImageReference {
            identifier: n.identifier.clone(),
            alt: n.alt.clone(),
        },
        mdast::Node::FootnoteDefinition(n) => Node::FootnoteDefinition {
            identifier: n.identifier.clone(),
            children: convert_all(&n.children),
        },
        mdast::Node::FootnoteReference(n) => Node::FootnoteReference {
            identifier: n.identifier.clone(),
        },
        mdast::Node::Table(n) => Node::Table {
            align: n
                .align
                .iter()
                .map(|a| match a {
                    mdast::AlignKind::Left => Align::Left,
                    mdast::AlignKind::Right => Align::Right,
                    mdast::AlignKind::Center => Align::Center,
                    mdast::AlignKind::None => Align::None,
                })
                .collect(),
            children: convert_all(&n.children),
        },
        mdast::Node::TableRow(n) => Node::TableRow(convert_all(&n.children)),
        mdast::Node::TableCell(n) => Node::TableCell(convert_all(&n.children)),
        _ => return None,
    })
}

/// Parses a document root.
pub(crate) fn root_children(node: &mdast::Node) -> Vec<Node> {
    node.children()
        .map(|children| convert_all(children))
        .unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn helpers_cover_every_variant() {
        assert_eq!(
            [Align::None, Align::Left, Align::Right, Align::Center].map(Align::as_str),
            [None, Some("left"), Some("right"), Some("center")]
        );
        let mut reference = Node::LinkReference {
            identifier: "a".into(),
            children: vec![Node::Break],
        };
        assert_eq!(reference.children_mut().map(|c| c.len()), Some(1));
        assert_eq!(Node::Break.children(), None);
        let yaml = mdast::Node::Yaml(mdast::Yaml {
            value: String::new(),
            position: None,
        });
        assert_eq!(convert(&yaml), None);
        assert_eq!(root_children(&yaml), Vec::new());
        assert_eq!(
            mdast_depth(&markdown::to_mdast("*a*", &markdown::ParseOptions::gfm()).unwrap()),
            4
        );
    }
}
