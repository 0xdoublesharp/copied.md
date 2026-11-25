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
    flow.children(&dom.document, 0, Marks::default());
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
            Node::Html(value) => value.bytes().filter(|&byte| byte == b'\n').count(),
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
        "script"
            | "style"
            | "noscript"
            | "template"
            | "head"
            | "title"
            | "svg"
            | "annotation"
            | "annotation-xml"
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

/// Split only at declaration boundaries, outside CSS strings, comments, and
/// component values such as URLs and custom properties.
fn css_declarations(style: &str) -> impl Iterator<Item = &str> {
    let mut bytes = style.bytes().enumerate().peekable();
    std::iter::from_fn(move || {
        let start = bytes.peek()?.0;
        let mut quote = None;
        let mut closers = Vec::new();
        while let Some((index, byte)) = bytes.next() {
            if byte == b'\\' {
                bytes.next();
            } else if let Some(open) = quote {
                if byte == open {
                    quote = None;
                }
            } else {
                match byte {
                    b'\'' | b'"' => quote = Some(byte),
                    b'(' => closers.push(b')'),
                    b'[' => closers.push(b']'),
                    b'{' => closers.push(b'}'),
                    b'/' if bytes.next_if(|&(_, byte)| byte == b'*').is_some() => {
                        while let Some((_, byte)) = bytes.next() {
                            if byte == b'*' && bytes.next_if(|&(_, byte)| byte == b'/').is_some() {
                                break;
                            }
                        }
                    }
                    b';' if closers.is_empty() => return Some(&style[start..index]),
                    _ if closers.last() == Some(&byte) => {
                        closers.pop();
                    }
                    _ => {}
                }
            }
        }
        Some(&style[start..])
    })
}

fn trim_css(mut value: &str) -> &str {
    loop {
        value = value.trim_ascii();
        if let Some((_, rest)) = value
            .strip_prefix("/*")
            .and_then(|value| value.split_once("*/"))
        {
            value = rest;
        } else if let Some((rest, _)) = value
            .strip_suffix("*/")
            .and_then(|value| value.rsplit_once("/*"))
        {
            value = rest;
        } else {
            return value;
        }
    }
}

fn css_property<T>(
    style: &str,
    names: &[&str],
    parse_value: impl Fn(&str) -> Option<T>,
) -> Option<T> {
    css_declarations(style)
        .filter_map(|declaration| {
            let (name, value) = trim_css(declaration).split_once(':')?;
            let name = trim_css(name);
            if !names
                .iter()
                .any(|property| name.eq_ignore_ascii_case(property))
            {
                return None;
            }
            let value = trim_css(value).to_ascii_lowercase();
            let important = value
                .rsplit_once('!')
                .filter(|(_, suffix)| trim_css(suffix) == "important");
            let token = important.map_or(value.as_str(), |(token, _)| trim_css(token));
            parse_value(token).map(|parsed| (important.is_some(), parsed))
        })
        .max_by_key(|&(important, _)| important)
        .map(|(_, value)| value)
}

fn preformatted(style: &str) -> bool {
    css_property(style, &["white-space"], |value| match value {
        "pre" | "pre-wrap" | "break-spaces" => Some(true),
        "" => None,
        _ => Some(false),
    })
    .unwrap_or_default()
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

    fn children(&mut self, node: &Handle, depth: usize, marks: Marks) {
        if depth < MAX_HTML_DEPTH {
            for child in node.children.borrow().iter() {
                self.visit(child, depth + 1, marks);
            }
        }
    }

    fn visit(&mut self, node: &Handle, depth: usize, inherited: Marks) {
        match &node.data {
            NodeData::Text { contents } => self.inline.text(&contents.borrow(), inherited),
            NodeData::Element { name, .. } => {
                let tag = name.local.as_ref();
                if ignored(tag) {
                    return;
                }
                let style = attr(node, "style");
                let marks = inherited.styled(tag, style.as_deref().unwrap_or_default());
                match tag {
                    "div" if preformatted(style.as_deref().unwrap_or_default()) => {
                        self.block(code_block(node, depth, true));
                    }
                    tag if paragraph(tag) => {
                        self.flush();
                        self.children(node, depth, marks);
                        self.flush();
                    }
                    "h1" | "h2" | "h3" | "h4" | "h5" | "h6" => {
                        let children =
                            Inline::read(node, depth, InlineMode::SingleLine, marks).finish();
                        if !children.is_empty() {
                            self.block(Node::Heading {
                                depth: tag.as_bytes()[1] - b'0',
                                children,
                            });
                        }
                    }
                    "blockquote" => {
                        let mut quote = Self::default();
                        quote.children(node, depth, marks);
                        quote.flush();
                        if !quote.blocks.is_empty() {
                            self.block(Node::Blockquote(quote.blocks));
                        }
                    }
                    "pre" => self.block(code_block(node, depth, false)),
                    "hr" => self.block(Node::ThematicBreak),
                    "ul" | "ol" => {
                        if let Some(node) = list(node, depth, tag == "ol", marks) {
                            self.block(node);
                        }
                    }
                    "table" => {
                        let blocks = table(node, depth, marks);
                        if !blocks.is_empty() {
                            self.flush();
                            self.blocks.extend(blocks);
                        }
                    }
                    "math"
                        if attr(node, "display")
                            .is_some_and(|display| display.eq_ignore_ascii_case("block")) =>
                    {
                        self.flush();
                        self.inline.math(node, depth, InlineMode::Flow, marks, true);
                        self.flush();
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
                    "br" | "a" | "code" | "img" | "input" | "math" => {
                        self.inline
                            .visit_element(node, tag, depth, InlineMode::Flow, marks);
                    }
                    _ => self.children(node, depth, marks),
                }
            }
            _ => {}
        }
    }
}

#[derive(Clone, Copy)]
enum InlineMode {
    Flow,
    SingleLine,
    TableCell,
}

#[derive(Default)]
struct Inline {
    nodes: Vec<Node>,
    runs: Vec<MarkedRun>,
    leading_space: Option<Marks>,
    space: Option<Marks>,
}

struct MarkedRun {
    mark: Mark,
    children: Vec<Node>,
}

impl Inline {
    fn read(node: &Handle, depth: usize, mode: InlineMode, marks: Marks) -> Self {
        let mut inline = Self::default();
        inline.children(node, depth, mode, marks);
        inline
    }

    fn finish(mut self) -> Vec<Node> {
        self.close_to(0);
        while matches!(self.nodes.last(), Some(Node::Break)) {
            self.nodes.pop();
        }
        self.nodes
    }

    fn target(&mut self) -> &mut Vec<Node> {
        self.runs
            .last_mut()
            .map_or(&mut self.nodes, |run| &mut run.children)
    }

    fn shared_depth(&self, marks: Marks) -> usize {
        marks
            .iter()
            .zip(&self.runs)
            .take_while(|(mark, run)| *mark == run.mark)
            .count()
    }

    fn close_to(&mut self, depth: usize) {
        while self.runs.len() > depth {
            let MarkedRun { mark, children } = self.runs.pop().expect("run above depth");
            let node = match mark {
                Mark::Bold => Node::Strong(children),
                Mark::Italic => Node::Emphasis(children),
                Mark::Strike => Node::Delete(children),
            };
            self.target().push(node);
        }
    }

    fn whitespace(&mut self, marks: Marks) {
        let space = self.space.map_or(marks, |previous| previous.common(marks));
        self.space = Some(space);
        if self.runs.is_empty() && self.nodes.is_empty() {
            self.leading_space = Some(space);
        }
    }

    fn text(&mut self, text: &str, marks: Marks) {
        for part in text.split_inclusive(is_js_whitespace) {
            let word = part.trim_end_matches(is_js_whitespace);
            if !word.is_empty() {
                self.prepare(marks);
                self.push_text(word);
            }
            if word.len() != part.len() {
                self.whitespace(marks);
            }
        }
    }

    fn prepare(&mut self, marks: Marks) {
        // Only keep whitespace inside a mark when the whitespace and the
        // content on both sides share that mark.
        if let Some(space) = self.space.take()
            && (!self.runs.is_empty()
                || self.nodes.last().is_some_and(|node| {
                    !matches!(node, Node::Break)
                        && !matches!(node, Node::Html(value) if value == "<br>")
                }))
        {
            self.close_to(self.shared_depth(space.common(marks)));
            self.push_text(" ");
        }
        let shared = self.shared_depth(marks);
        self.close_to(shared);
        self.runs
            .extend(marks.iter().skip(shared).map(|mark| MarkedRun {
                mark,
                children: Vec::new(),
            }));
    }

    fn push_text(&mut self, text: &str) {
        let target = self.target();
        if let Some(Node::Text(previous)) = target.last_mut() {
            previous.push_str(text);
        } else {
            target.push(Node::Text(text.to_owned()));
        }
    }

    fn push(&mut self, node: Node, marks: Marks) {
        self.prepare(marks);
        self.target().push(node);
    }

    fn children(&mut self, node: &Handle, depth: usize, mode: InlineMode, marks: Marks) {
        if depth < MAX_HTML_DEPTH {
            for child in node.children.borrow().iter() {
                match &child.data {
                    NodeData::Text { contents } => self.text(&contents.borrow(), marks),
                    NodeData::Element { name, .. } if !ignored(name.local.as_ref()) => {
                        let tag = name.local.as_ref();
                        let style = attr(child, "style");
                        self.visit_element(
                            child,
                            tag,
                            depth + 1,
                            mode,
                            marks.styled(tag, style.as_deref().unwrap_or_default()),
                        );
                    }
                    _ => {}
                }
            }
        }
    }

    fn visit_element(
        &mut self,
        node: &Handle,
        tag: &str,
        depth: usize,
        mode: InlineMode,
        marks: Marks,
    ) {
        match tag {
            "br" => self.line_break(mode, marks),
            "code" => {
                let value = raw_text(node, depth, false);
                let value = value.split_whitespace().collect::<Vec<_>>().join(" ");
                if !value.is_empty() {
                    self.push(Node::InlineCode(value), marks);
                }
            }
            "a" => self.link(node, depth, mode, marks),
            "math" => self.math(node, depth, mode, marks, false),
            "img" => {
                if let Some(alt) = attr(node, "alt") {
                    self.text(&alt, marks);
                }
            }
            "input" if is_checkbox(node) => {
                self.text(
                    if attr(node, "checked").is_some() {
                        "☑ "
                    } else {
                        "☐ "
                    },
                    marks,
                );
            }
            tag if paragraph(tag) => {
                self.whitespace(marks);
                self.children(node, depth, mode, marks);
                self.whitespace(marks);
            }
            _ => self.children(node, depth, mode, marks),
        }
    }

    fn line_break(&mut self, mode: InlineMode, marks: Marks) {
        match mode {
            InlineMode::Flow => {
                self.space = None;
                self.close_to(0);
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
            InlineMode::SingleLine => self.whitespace(marks),
            InlineMode::TableCell => {
                self.space = None;
                self.push(Node::Html("<br>".to_owned()), Marks::default());
            }
        }
    }

    fn link(&mut self, node: &Handle, depth: usize, mode: InlineMode, marks: Marks) {
        let mode = match mode {
            InlineMode::TableCell => InlineMode::TableCell,
            _ => InlineMode::SingleLine,
        };
        let url = attr(node, "href")
            .and_then(|href| safe_url(href.trim(), None))
            .map(|url| normalize_uri(&url));
        let Some(url) = url else {
            self.children(node, depth, mode, marks);
            return;
        };
        let inline = Self::read(node, depth, mode, marks);
        let (leading, trailing) = (inline.leading_space, inline.space);
        let mut children = inline.finish();
        if let Some(leading) = leading {
            self.whitespace(leading);
        }
        if !children.is_empty() {
            let marks = marks.hoist(&mut children);
            self.push(
                Node::Link {
                    url,
                    title: None,
                    children,
                },
                marks,
            );
        }
        if let Some(trailing) = trailing {
            self.whitespace(trailing);
        }
    }

    fn math(&mut self, node: &Handle, depth: usize, mode: InlineMode, marks: Marks, block: bool) {
        if let Some(tex) = tex_annotation(node, depth) {
            let marker = if block { "$$" } else { "$" };
            let mut math = marker.to_owned();
            let mut chars = tex.chars().peekable();
            while let Some(ch) = chars.next() {
                match ch {
                    // Keep raw TeX commands, but prevent an annotation from
                    // reintroducing HTML when rendered without math support.
                    '<' if chars.peek().is_some_and(|next| {
                        next.is_ascii_alphabetic() || matches!(next, '!' | '/' | '?')
                    }) =>
                    {
                        math.push_str("&#x3C;");
                    }
                    '|' if matches!(mode, InlineMode::TableCell) => math.push_str("\\|"),
                    '\n' if !block => math.push(' '),
                    _ => math.push(ch),
                }
            }
            math.push_str(marker);
            self.push(Node::Html(math), marks);
        } else {
            self.text(&raw_text(node, depth, false), marks);
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Mark {
    Bold,
    Italic,
    Strike,
}

/// Effective marks in their HTML nesting order.
#[derive(Clone, Copy, Default)]
struct Marks([Option<Mark>; 3]);

impl Marks {
    fn iter(self) -> impl Iterator<Item = Mark> {
        self.0.into_iter().flatten()
    }

    fn common(self, other: Self) -> Self {
        let mut common = Self::default();
        for (index, (left, right)) in self.iter().zip(other.iter()).enumerate() {
            if left != right {
                break;
            }
            common.0[index] = Some(left);
        }
        common
    }

    fn set(&mut self, mark: Mark, enabled: bool) {
        if let Some(index) = self.iter().position(|current| current == mark) {
            if !enabled {
                self.0[index..].rotate_left(1);
                self.0[2] = None;
            }
        } else if enabled && let Some(slot) = self.0.iter_mut().find(|current| current.is_none()) {
            *slot = Some(mark);
        }
    }

    fn styled(mut self, tag: &str, style: &str) -> Self {
        let bold = css_property(style, &["font-weight"], |value| match value {
            "bold" | "bolder" => Some(true),
            "normal" | "lighter" => Some(false),
            _ => value
                .parse::<f32>()
                .ok()
                .filter(|weight| (1.0..=1000.0).contains(weight))
                .map(|weight| weight >= 600.0),
        });
        let italic = css_property(style, &["font-style"], |value| match value {
            "italic" | "oblique" => Some(true),
            "normal" => Some(false),
            _ => None,
        });
        let strike = css_property(
            style,
            &["text-decoration", "text-decoration-line"],
            |value| {
                let mut line = None;
                for part in value.split_ascii_whitespace() {
                    match part {
                        "line-through" => return Some(true),
                        "none" | "underline" | "overline" | "blink" => line = Some(false),
                        _ => {}
                    }
                }
                line
            },
        );
        for (mark, css, semantic) in [
            (Mark::Bold, bold, matches!(tag, "b" | "strong")),
            (Mark::Italic, italic, matches!(tag, "em" | "i")),
            (Mark::Strike, strike, matches!(tag, "s" | "del" | "strike")),
        ] {
            if let Some(enabled) = css.or(semantic.then_some(true)) {
                self.set(mark, enabled);
            }
        }
        self
    }

    /// Move marks shared by the whole label outside its link while keeping
    /// styles originating within the label on the inside.
    fn hoist(self, children: &mut Vec<Node>) -> Self {
        let mut hoisted = Self::default();
        for (index, mark) in self.iter().enumerate() {
            let (([Node::Strong(inner)], Mark::Bold)
            | ([Node::Emphasis(inner)], Mark::Italic)
            | ([Node::Delete(inner)], Mark::Strike)) = (&mut children[..], mark)
            else {
                break;
            };
            *children = std::mem::take(inner);
            hoisted.0[index] = Some(mark);
        }
        hoisted
    }
}

fn is_checkbox(node: &Handle) -> bool {
    attr(node, "type").is_some_and(|value| value.eq_ignore_ascii_case("checkbox"))
}

fn list(node: &Handle, depth: usize, ordered: bool, marks: Marks) -> Option<Node> {
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
        let style = attr(item, "style");
        flow.children(
            item,
            depth + 1,
            marks.styled("li", style.as_deref().unwrap_or_default()),
        );
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

fn raw_text(node: &Handle, depth: usize, editor_lines: bool) -> String {
    fn append(node: &Handle, depth: usize, text: &mut String, editor_lines: bool) {
        match &node.data {
            NodeData::Text { contents } => text.push_str(&contents.borrow()),
            NodeData::Element { name, .. } if name.local.as_ref() == "br" => text.push('\n'),
            NodeData::Element { name, .. }
                if !ignored(name.local.as_ref()) && depth < MAX_HTML_DEPTH =>
            {
                let line = editor_lines && matches!(name.local.as_ref(), "div" | "p");
                if line && !text.is_empty() && !text.ends_with('\n') {
                    text.push('\n');
                }
                let start = text.len();
                for child in node.children.borrow().iter() {
                    append(child, depth + 1, text, editor_lines);
                }
                if line && (text.len() == start || !text.ends_with('\n')) {
                    text.push('\n');
                }
            }
            _ => {}
        }
    }
    let mut text = String::new();
    if depth < MAX_HTML_DEPTH {
        for child in node.children.borrow().iter() {
            append(child, depth + 1, &mut text, editor_lines);
        }
    }
    text
}

fn tex_annotation(node: &Handle, depth: usize) -> Option<String> {
    if depth >= MAX_HTML_DEPTH {
        return None;
    }
    node.children.borrow().iter().find_map(|child| {
        let tag = element(child)?;
        if tag == "annotation" {
            let encoding = attr(child, "encoding")?;
            if ["application/x-tex", "text/x-latex", "tex"]
                .iter()
                .any(|kind| encoding.trim().eq_ignore_ascii_case(kind))
            {
                let value = raw_text(child, depth + 1, false);
                let tex = value.trim();
                if !tex.is_empty() {
                    return Some(tex.to_owned());
                }
            }
            None
        } else if ignored(tag) {
            None
        } else {
            tex_annotation(child, depth + 1)
        }
    })
}

fn language(node: &Handle) -> Option<String> {
    let class = attr(node, "class")?;
    class
        .split_ascii_whitespace()
        .filter_map(|name| name.strip_prefix("language-"))
        .find(|name| !name.is_empty() && !name.contains('`'))
        .map(str::to_owned)
}

fn code_block(node: &Handle, depth: usize, editor_lines: bool) -> Node {
    let lang = node
        .children
        .borrow()
        .iter()
        .find(|child| element(child) == Some("code"))
        .and_then(language)
        .or_else(|| language(node));
    let value = raw_text(node, depth, editor_lines);
    Node::Code {
        value,
        lang,
        meta: None,
    }
}

struct TableRow {
    group: usize,
    header: bool,
    cells: Vec<TableCell>,
}

struct TableCell {
    children: Vec<Node>,
    align: Align,
    colspan: usize,
    rowspan: usize,
}

fn table(node: &Handle, depth: usize, marks: Marks) -> Vec<Node> {
    if depth >= MAX_HTML_DEPTH {
        return Vec::new();
    }
    let mut rows = Vec::new();
    let groups = node.children.borrow();
    // Render a real header first, even if the source puts it after the body.
    let ordered = groups
        .iter()
        .filter(|node| element(node) == Some("thead"))
        .chain(groups.iter().filter(|node| element(node) != Some("thead")));
    for (group, section) in ordered.enumerate() {
        match element(section) {
            Some("tr") => rows.extend(table_row(section, depth + 1, group, false, marks)),
            Some(tag @ ("thead" | "tbody" | "tfoot")) if depth + 1 < MAX_HTML_DEPTH => {
                let style = attr(section, "style");
                let marks = marks.styled(tag, style.as_deref().unwrap_or_default());
                for row in section
                    .children
                    .borrow()
                    .iter()
                    .filter(|node| element(node) == Some("tr"))
                {
                    rows.extend(table_row(row, depth + 2, group, tag == "thead", marks));
                }
            }
            _ => {}
        }
    }
    let Some(first) = rows.first() else {
        return Vec::new();
    };
    let header = first.header;
    let Some((positions, align)) = table_columns(&rows) else {
        return table_paragraphs(rows);
    };
    if !header && align.len() == 1 {
        return table_paragraphs(rows);
    }

    let mut children = Vec::with_capacity(rows.len() + usize::from(!header));
    if !header {
        // GFM requires a header. Leave it blank so the first data row stays
        // in the body.
        children.push(Node::TableRow(Vec::new()));
    }
    for (row, positions) in rows.into_iter().zip(positions) {
        let mut cells = Vec::new();
        for (cell, column) in row.cells.into_iter().zip(positions) {
            cells.resize_with(column, empty_table_cell);
            cells.push(Node::TableCell(cell.children));
            cells.resize_with(column + cell.colspan, empty_table_cell);
        }
        children.push(Node::TableRow(cells));
    }
    vec![Node::Table { align, children }]
}

fn empty_table_cell() -> Node {
    Node::TableCell(Vec::new())
}

fn table_row(
    row: &Handle,
    depth: usize,
    group: usize,
    mut header: bool,
    marks: Marks,
) -> Option<TableRow> {
    if depth >= MAX_HTML_DEPTH {
        return None;
    }
    let style = attr(row, "style");
    let marks = marks.styled("tr", style.as_deref().unwrap_or_default());
    let mut cells = Vec::new();
    for cell in row.children.borrow().iter() {
        let Some(tag @ ("td" | "th")) = element(cell) else {
            continue;
        };
        header |= tag == "th";
        let style = attr(cell, "style");
        let style = style.as_deref().unwrap_or_default();
        let align = css_property(style, &["text-align"], alignment)
            .or_else(|| alignment(&attr(cell, "align")?.trim().to_ascii_lowercase()))
            .unwrap_or(Align::None);
        let children = Inline::read(
            cell,
            depth + 1,
            InlineMode::TableCell,
            marks.styled(tag, style),
        )
        .finish();
        let span = |name| {
            attr(cell, name)
                .and_then(|value| value.trim().parse::<usize>().ok())
                .unwrap_or(1)
        };
        cells.push(TableCell {
            children,
            align,
            colspan: span("colspan").clamp(1, 1000),
            rowspan: span("rowspan").min(65534),
        });
    }
    (!cells.is_empty()).then_some(TableRow {
        group,
        header,
        cells,
    })
}

fn alignment(value: &str) -> Option<Align> {
    match value {
        "left" => Some(Align::Left),
        "center" => Some(Align::Center),
        "right" => Some(Align::Right),
        _ => None,
    }
}

fn table_columns(rows: &[TableRow]) -> Option<(Vec<Vec<usize>>, Vec<Align>)> {
    let mut align = Vec::new();
    let mut occupied_until = Vec::new();
    let mut positions = Vec::with_capacity(rows.len());
    // Bound padding work relative to the number of real cells. A table with
    // very wide spans stays readable as separate paragraphs, one per row.
    let mut budget = rows.iter().map(|row| row.cells.len()).sum::<usize>() * 8 + 32;
    let mut group = rows.first()?.group;
    for (index, row) in rows.iter().enumerate() {
        if row.group != group {
            occupied_until.clear();
            group = row.group;
        }
        let mut row_positions = Vec::with_capacity(row.cells.len());
        let mut column = 0;
        for cell in &row.cells {
            while let Some(blocked) = occupied_until
                .get(column..)
                .into_iter()
                .flatten()
                .take(cell.colspan)
                .position(|&end| end > index)
            {
                column += blocked + 1;
            }
            let end = column + cell.colspan;
            if end > budget {
                return None;
            }
            if end > occupied_until.len() {
                occupied_until.resize(end, 0);
            }
            if end > align.len() {
                align.resize(end, Align::None);
            }
            if align[column] == Align::None {
                align[column] = cell.align;
            }
            let until = if cell.rowspan == 0 {
                usize::MAX
            } else {
                index + cell.rowspan
            };
            occupied_until[column..end].fill(until);
            row_positions.push(column);
            column = end;
        }
        budget -= column;
        positions.push(row_positions);
    }
    Some((positions, align))
}

fn table_paragraphs(rows: Vec<TableRow>) -> Vec<Node> {
    rows.into_iter()
        .filter_map(|row| {
            let mut children = Vec::new();
            for (index, cell) in row.cells.into_iter().enumerate() {
                if index != 0 {
                    children.push(Node::Text(" | ".to_owned()));
                }
                children.extend(cell.children);
            }
            (!children.is_empty()).then_some(Node::Paragraph(children))
        })
        .collect()
}

#[cfg(test)]
mod tests;
