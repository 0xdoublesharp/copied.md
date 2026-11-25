//! Clipboard HTML to a GFM tree, using the same serializer as Markdown targets.

use std::cell::Cell;

use html5ever::{
    tendril::StrTendril,
    tokenizer::{
        BufferQueue, Token, TokenSink, TokenSinkResult, Tokenizer, TokenizerOpts, TokenizerResult,
    },
    tree_builder::{Tracer, TreeBuilder, TreeBuilderOpts},
};
use markup5ever_rcdom::{Handle, NodeData, RcDom};

use crate::{
    ast::{Align, Node},
    limits::{MAX_HTML_DEPTH, MAX_INPUT_BYTES},
    render::markdown::serialize_clipboard_markdown,
    url::{is_js_whitespace, normalize_uri, safe_url},
};

const BOLD: u8 = 1;
const ITALIC: u8 = 2;
const STRIKE: u8 = 4;
const MAX_PREFIX_BYTES: usize = 4 * MAX_INPUT_BYTES;

/// Converts rich clipboard HTML to GitHub Flavored Markdown.
///
/// Input beyond the size, parser, or indentation limits produces an empty string. Content
/// beyond the HTML nesting limit is omitted, along with scripts, styles and
/// document metadata.
pub fn html_to_markdown(html: &str) -> String {
    if html.len() > MAX_INPUT_BYTES || html.trim().is_empty() {
        return String::new();
    }
    let Some(dom) = parse(html) else {
        return String::new();
    };
    let mut flow = Flow::default();
    flow.children(&dom.document, 0);
    flow.flush();
    let mut prefix_budget = MAX_PREFIX_BYTES;
    if !bounded_prefixes(&flow.blocks, 0, &mut prefix_budget) {
        return String::new();
    }
    serialize_clipboard_markdown(&flow.blocks)
}

/// Bounds the extra indentation that nested lists and quotes can add to every line.
fn bounded_prefixes(nodes: &[Node], width: usize, remaining: &mut usize) -> bool {
    nodes.iter().all(|node| {
        let lines = match node {
            Node::Code { value, .. } => 3 + value.bytes().filter(|&byte| byte == b'\n').count(),
            Node::Paragraph(_)
            | Node::Heading { .. }
            | Node::ThematicBreak
            | Node::Blockquote(_)
            | Node::List { .. }
            | Node::ListItem { .. }
            | Node::TableRow(_)
            | Node::Break => 2,
            _ => 0,
        };
        let Some(budget) = remaining.checked_sub(lines * width) else {
            return false;
        };
        *remaining = budget;
        let indent = match node {
            Node::Blockquote(_) => 2,
            Node::ListItem { .. } => 11, // A nine-digit marker, period, and space.
            _ => 0,
        };
        node.children()
            .is_none_or(|children| bounded_prefixes(children, width + indent, remaining))
    })
}

fn parse(html: &str) -> Option<RcDom> {
    let sink = BoundedDom {
        tree: TreeBuilder::new(RcDom::default(), TreeBuilderOpts::default()),
        tags: 0,
        exceeded: false,
    };
    let mut tokenizer = Tokenizer::new(sink, TokenizerOpts::default());
    let mut input = BufferQueue::default();
    input.push_back(StrTendril::from_slice(html));
    while let TokenizerResult::Script(_) = tokenizer.feed(&mut input) {}
    tokenizer.end();
    (!tokenizer.sink.exceeded).then_some(tokenizer.sink.tree.sink)
}

struct BoundedDom {
    tree: TreeBuilder<Handle, RcDom>,
    tags: usize,
    exceeded: bool,
}

#[derive(Default)]
struct HandleCount(Cell<usize>);

impl Tracer for HandleCount {
    type Handle = Handle;

    fn trace_handle(&self, _: &Handle) {
        self.0.set(self.0.get() + 1);
    }
}

impl TokenSink for BoundedDom {
    type Handle = Handle;

    fn process_token(&mut self, token: Token, line: u64) -> TokenSinkResult<Handle> {
        if self.exceeded {
            return TokenSinkResult::Continue;
        }
        self.tags += usize::from(matches!(token, Token::TagToken(_)));
        if self.tags == 32 {
            self.tags = 0;
            // Bound the parser's open-element and active-formatting stacks.
            // A formatting element can be in both, and the parser keeps a
            // few additional handles for the document and its metadata.
            let count = HandleCount::default();
            self.tree.trace_handles(&count);
            self.exceeded = count.0.get() > 2 * MAX_HTML_DEPTH + 8;
        }
        self.tree.process_token(token, line)
    }

    fn adjusted_current_node_present_but_not_in_html_namespace(&self) -> bool {
        self.tree
            .adjusted_current_node_present_but_not_in_html_namespace()
    }

    fn end(&mut self) {
        self.tree.end();
    }
}

fn element(node: &Handle) -> Option<&str> {
    match &node.data {
        NodeData::Element { name, .. } => Some(name.local.as_ref()),
        _ => None,
    }
}

fn ignored(tag: &str) -> bool {
    matches!(
        tag,
        "script" | "style" | "noscript" | "template" | "head" | "title" | "svg" | "math"
    )
}

fn attr(node: &Handle, key: &str) -> Option<String> {
    if let NodeData::Element { attrs, .. } = &node.data {
        attrs
            .borrow()
            .iter()
            .find(|attr| attr.name.local.as_ref() == key && attr.name.ns.is_empty())
            .map(|attr| attr.value.to_string())
    } else {
        None
    }
}

fn paragraph(tag: &str) -> bool {
    matches!(
        tag,
        "p" | "div"
            | "section"
            | "article"
            | "main"
            | "header"
            | "footer"
            | "figure"
            | "figcaption"
            | "address"
            | "dd"
            | "dt"
    )
}

#[derive(Default)]
struct Flow {
    blocks: Vec<Node>,
    inline: Inline,
    task_item: bool,
    checked: Option<bool>,
}

impl Flow {
    fn flush(&mut self) {
        let children = std::mem::take(&mut self.inline).finish();
        if !children.is_empty() {
            self.blocks.push(Node::Paragraph(children));
        }
    }

    fn block(&mut self, block: Node) {
        self.flush();
        self.blocks.push(block);
    }

    fn children(&mut self, node: &Handle, depth: usize) {
        if depth < MAX_HTML_DEPTH {
            for child in node.children.borrow().iter() {
                self.visit(child, depth + 1);
            }
        }
    }

    fn visit(&mut self, node: &Handle, depth: usize) {
        match &node.data {
            NodeData::Text { contents } => self.inline.text(&contents.borrow()),
            NodeData::Element { name, .. } => {
                let tag = name.local.as_ref();
                match tag {
                    tag if ignored(tag) => {}
                    tag if paragraph(tag) => {
                        self.flush();
                        self.children(node, depth);
                        self.flush();
                    }
                    "h1" | "h2" | "h3" | "h4" | "h5" | "h6" => {
                        let children = Inline::read(node, depth, true, 0).finish();
                        if !children.is_empty() {
                            self.block(Node::Heading {
                                depth: tag.as_bytes()[1] - b'0',
                                children,
                            });
                        }
                    }
                    "blockquote" => {
                        let mut quote = Self::default();
                        quote.children(node, depth);
                        quote.flush();
                        if !quote.blocks.is_empty() {
                            self.block(Node::Blockquote(quote.blocks));
                        }
                    }
                    "pre" => self.block(code_block(node, depth)),
                    "hr" => self.block(Node::ThematicBreak),
                    "ul" | "ol" => {
                        if let Some(node) = list(node, depth, tag == "ol") {
                            self.block(node);
                        }
                    }
                    "table" => {
                        if let Some(table) = table(node, depth) {
                            self.block(table);
                        }
                    }
                    "input"
                        if self.task_item
                            && self.blocks.is_empty()
                            && self.inline.nodes.is_empty()
                            && self.checked.is_none()
                            && is_checkbox(node) =>
                    {
                        self.checked = Some(attr(node, "checked").is_some());
                    }
                    "br" | "a" | "code" | "img" | "input" => {
                        self.inline.visit(node, depth, false, 0);
                    }
                    _ => {
                        let formatting = styles(node, tag);
                        if formatting == 0 {
                            self.children(node, depth);
                        } else {
                            self.inline.formatted(node, depth, false, formatting, 0);
                        }
                    }
                }
            }
            _ => {}
        }
    }
}

#[derive(Default)]
struct Inline {
    nodes: Vec<Node>,
    leading_space: bool,
    space: bool,
}

impl Inline {
    fn read(node: &Handle, depth: usize, single_line: bool, active_styles: u8) -> Self {
        let mut inline = Self::default();
        inline.children(node, depth, single_line, active_styles);
        inline
    }

    fn finish(mut self) -> Vec<Node> {
        while matches!(self.nodes.last(), Some(Node::Break)) {
            self.nodes.pop();
        }
        self.nodes
    }

    fn whitespace(&mut self) {
        self.leading_space |= self.nodes.is_empty();
        self.space = true;
    }

    fn text(&mut self, text: &str) {
        for part in text.split_inclusive(is_js_whitespace) {
            let word = part.trim_end_matches(is_js_whitespace);
            if !word.is_empty() {
                self.pending_space();
                self.push_text(word);
            }
            if word.len() != part.len() {
                self.whitespace();
            }
        }
    }

    fn pending_space(&mut self) {
        if self.space && !matches!(self.nodes.last(), None | Some(Node::Break)) {
            self.push_text(" ");
        }
        self.space = false;
    }

    fn push_text(&mut self, text: &str) {
        if let Some(Node::Text(previous)) = self.nodes.last_mut() {
            previous.push_str(text);
        } else {
            self.nodes.push(Node::Text(text.to_owned()));
        }
    }

    fn push(&mut self, node: Node) {
        self.pending_space();
        self.nodes.push(node);
    }

    fn children(&mut self, node: &Handle, depth: usize, single_line: bool, active_styles: u8) {
        if depth < MAX_HTML_DEPTH {
            for child in node.children.borrow().iter() {
                self.visit(child, depth + 1, single_line, active_styles);
            }
        }
    }

    fn visit(&mut self, node: &Handle, depth: usize, single_line: bool, active_styles: u8) {
        match &node.data {
            NodeData::Text { contents } => self.text(&contents.borrow()),
            NodeData::Element { name, .. } => {
                let tag = name.local.as_ref();
                match tag {
                    tag if ignored(tag) => {}
                    "br" if !single_line => {
                        self.space = false;
                        if !self.nodes.is_empty()
                            && !self
                                .nodes
                                .iter()
                                .rev()
                                .take(2)
                                .eq([&Node::Break, &Node::Break])
                        {
                            self.nodes.push(Node::Break);
                        }
                    }
                    "br" => self.whitespace(),
                    "code" => {
                        let value = raw_text(node, depth);
                        let value = value.split_whitespace().collect::<Vec<_>>().join(" ");
                        if !value.is_empty() {
                            self.push(Node::InlineCode(value));
                        }
                    }
                    "a" => self.link(node, depth, active_styles),
                    "img" => {
                        if let Some(alt) = attr(node, "alt") {
                            self.text(&alt);
                        }
                    }
                    "input" if is_checkbox(node) => {
                        self.text(if attr(node, "checked").is_some() {
                            "☑ "
                        } else {
                            "☐ "
                        });
                    }
                    tag if paragraph(tag) => {
                        self.whitespace();
                        self.children(node, depth, single_line, active_styles);
                        self.whitespace();
                    }
                    _ => self.formatted(node, depth, single_line, styles(node, tag), active_styles),
                }
            }
            _ => {}
        }
    }

    fn link(&mut self, node: &Handle, depth: usize, active_styles: u8) {
        let url = attr(node, "href")
            .and_then(|href| safe_url(href.trim(), None))
            .map(|url| normalize_uri(&url));
        let Some(url) = url else {
            self.children(node, depth, true, active_styles);
            return;
        };
        let inline = Self::read(node, depth, true, active_styles);
        let (leading, trailing) = (inline.leading_space, inline.space);
        let children = inline.finish();
        if leading {
            self.whitespace();
        }
        if !children.is_empty() {
            self.push(Node::Link {
                url,
                title: None,
                children,
            });
        }
        if trailing {
            self.whitespace();
        }
    }

    fn formatted(
        &mut self,
        node: &Handle,
        depth: usize,
        single_line: bool,
        formatting: u8,
        active: u8,
    ) {
        let formatting = formatting & !active;
        if formatting == 0 {
            self.children(node, depth, single_line, active);
            return;
        }
        let inline = Self::read(node, depth, single_line, active | formatting);
        let (leading, trailing) = (inline.leading_space, inline.space);
        let mut children = inline.finish();
        if leading {
            self.whitespace();
        }
        if !children.is_empty() {
            if formatting & STRIKE != 0 {
                children = vec![Node::Delete(children)];
            }
            if formatting & ITALIC != 0 {
                children = vec![Node::Emphasis(children)];
            }
            if formatting & BOLD != 0 {
                children = vec![Node::Strong(children)];
            }
            self.pending_space();
            self.nodes.extend(children);
        }
        if trailing {
            self.whitespace();
        }
    }
}

fn styles(node: &Handle, tag: &str) -> u8 {
    let mut flags = match tag {
        "b" | "strong" => BOLD,
        "em" | "i" => ITALIC,
        "s" | "del" | "strike" => STRIKE,
        _ => 0,
    };
    // Google Docs, Word and browser editors frequently use inline styles.
    if let Some(style) = attr(node, "style") {
        for (property, value) in style
            .split(';')
            .filter_map(|declaration| declaration.split_once(':'))
        {
            let value = value.trim().to_ascii_lowercase();
            match property.trim().to_ascii_lowercase().as_str() {
                "font-weight" => {
                    flags &= !BOLD;
                    if matches!(value.as_str(), "bold" | "bolder")
                        || value.parse::<u16>().is_ok_and(|weight| weight >= 600)
                    {
                        flags |= BOLD;
                    }
                }
                "font-style" => {
                    flags &= !ITALIC;
                    if matches!(value.as_str(), "italic" | "oblique") {
                        flags |= ITALIC;
                    }
                }
                "text-decoration" | "text-decoration-line" => {
                    flags &= !STRIKE;
                    if value
                        .split_ascii_whitespace()
                        .any(|part| part == "line-through")
                    {
                        flags |= STRIKE;
                    }
                }
                _ => {}
            }
        }
    }
    flags
}

fn is_checkbox(node: &Handle) -> bool {
    attr(node, "type").is_some_and(|value| value.eq_ignore_ascii_case("checkbox"))
}

fn list(node: &Handle, depth: usize, ordered: bool) -> Option<Node> {
    if depth >= MAX_HTML_DEPTH {
        return None;
    }
    let mut children = Vec::new();
    for item in node
        .children
        .borrow()
        .iter()
        .filter(|node| element(node) == Some("li"))
    {
        let mut flow = Flow {
            task_item: true,
            ..Flow::default()
        };
        flow.children(item, depth + 1);
        flow.flush();
        if !flow.blocks.is_empty() {
            let spread = flow
                .blocks
                .windows(2)
                .any(|pair| matches!(pair, [Node::Paragraph(_), Node::Paragraph(_)]));
            children.push(Node::ListItem {
                checked: flow.checked,
                spread,
                children: flow.blocks,
            });
        }
    }
    let start = attr(node, "start")
        .and_then(|value| value.trim().parse::<u32>().ok())
        .filter(|&value| value <= 999_999_999);
    let spread = children
        .iter()
        .any(|item| matches!(item, Node::ListItem { spread: true, .. }));
    (!children.is_empty()).then_some(Node::List {
        ordered,
        start,
        spread,
        children,
    })
}

fn raw_text(node: &Handle, depth: usize) -> String {
    fn append(node: &Handle, depth: usize, text: &mut String) {
        match &node.data {
            NodeData::Text { contents } => text.push_str(&contents.borrow()),
            NodeData::Element { name, .. } if name.local.as_ref() == "br" => text.push('\n'),
            NodeData::Element { name, .. }
                if !ignored(name.local.as_ref()) && depth < MAX_HTML_DEPTH =>
            {
                for child in node.children.borrow().iter() {
                    append(child, depth + 1, text);
                }
            }
            _ => {}
        }
    }
    let mut text = String::new();
    append(node, depth, &mut text);
    text
}

fn language(node: &Handle) -> Option<String> {
    let class = attr(node, "class")?;
    class
        .split_ascii_whitespace()
        .filter_map(|name| name.strip_prefix("language-"))
        .find(|name| !name.is_empty() && !name.contains('`'))
        .map(str::to_owned)
}

fn code_block(node: &Handle, depth: usize) -> Node {
    let lang = node
        .children
        .borrow()
        .iter()
        .find(|child| element(child) == Some("code"))
        .and_then(language)
        .or_else(|| language(node));
    let value = raw_text(node, depth);
    Node::Code {
        value,
        lang,
        meta: None,
    }
}

fn table(node: &Handle, depth: usize) -> Option<Node> {
    let mut rows = Vec::new();
    let mut align = Vec::new();
    if depth < MAX_HTML_DEPTH {
        for section in node.children.borrow().iter() {
            match element(section) {
                Some("tr") => table_row(section, depth + 1, &mut rows, &mut align),
                Some("thead" | "tbody" | "tfoot") if depth + 1 < MAX_HTML_DEPTH => {
                    for row in section
                        .children
                        .borrow()
                        .iter()
                        .filter(|node| element(node) == Some("tr"))
                    {
                        table_row(row, depth + 2, &mut rows, &mut align);
                    }
                }
                _ => {}
            }
        }
    }
    (!rows.is_empty()).then_some(Node::Table {
        align,
        children: rows,
    })
}

fn table_row(row: &Handle, depth: usize, rows: &mut Vec<Node>, align: &mut Vec<Align>) {
    if depth >= MAX_HTML_DEPTH {
        return;
    }
    let cells = row.children.borrow();
    let cells = cells
        .iter()
        .filter(|cell| matches!(element(cell), Some("td" | "th")));
    let header = rows.is_empty();
    let mut children = Vec::new();
    for cell in cells {
        if header {
            align.push(match attr(cell, "align").as_deref() {
                Some("left") => Align::Left,
                Some("right") => Align::Right,
                Some("center") => Align::Center,
                _ => Align::None,
            });
        }
        children.push(Node::TableCell(
            Inline::read(cell, depth + 1, true, 0).finish(),
        ));
    }
    if !children.is_empty() {
        if children.len() > align.len() {
            align.resize(children.len(), Align::None);
        }
        rows.push(Node::TableRow(children));
    }
}

#[cfg(test)]
mod tests;
