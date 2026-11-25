//! HTML parsing into a small owned tree using `html5ever`.
//!
//! Two entry points mirror the oracle:
//!
//! * [`parse_fragment`] matches `hast-util-from-html` with `fragment: true`
//!   (parse5 fragment parsing with a `<template>` context, scripting disabled).
//! * [`RawBuilder`] matches `hast-util-raw` (`rehype-raw`): Markdown-generated
//!   elements and text are fed to the tree builder as tokens, while raw HTML
//!   strings go through a tokenizer, so stray markup in one raw node cannot
//!   swallow the Markdown that follows it.
//!
//! The tree builder runs iteratively. The owned tree is built with an explicit
//! stack and is flattened beyond [`MAX_HTML_DEPTH`](crate::limits::MAX_HTML_DEPTH)
//! so every later recursive pass is bounded.

use crate::limits::MAX_HTML_DEPTH;
use html5ever::tendril::StrTendril;
use html5ever::tokenizer::states::{RawKind, State};
use html5ever::tokenizer::{
    BufferQueue, EndTag, StartTag, Tag, Token, TokenSink, TokenSinkResult, Tokenizer, TokenizerOpts, TokenizerResult,
};
use html5ever::tree_builder::{
    ElementFlags, NodeOrText, QuirksMode, TreeBuilder, TreeBuilderOpts, TreeSink, create_element,
};
use html5ever::{Attribute, ExpandedName, LocalName, QualName, local_name, namespace_url, ns};
use std::borrow::Cow;

/// Owned HTML node.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum HNode {
    Element(HElement),
    Text(String),
}

/// Owned HTML element. Attribute names are lowercase local names.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct HElement {
    pub name: String,
    pub attrs: Vec<(String, String)>,
    pub children: Vec<HNode>,
    /// Whether the source closed this element with its own end tag (spans only).
    pub explicit_close: bool,
}

impl HElement {
    pub(crate) fn new(name: &str, attrs: Vec<(String, String)>, children: Vec<HNode>) -> Self {
        Self { name: name.to_owned(), attrs, children, explicit_close: false }
    }

    pub(crate) fn attr(&self, name: &str) -> Option<&str> {
        self.attrs.iter().find(|(key, _)| key == name).map(|(_, value)| value.as_str())
    }

    pub(crate) fn set_attr(&mut self, name: &str, value: String) {
        if let Some(entry) = self.attrs.iter_mut().find(|(key, _)| key == name) {
            entry.1 = value;
        } else {
            self.attrs.push((name.to_owned(), value));
        }
    }

    pub(crate) fn remove_attr(&mut self, name: &str) {
        self.attrs.retain(|(key, _)| key != name);
    }

    /// Child elements, skipping text.
    pub(crate) fn elements(&self) -> impl Iterator<Item = &HElement> {
        self.children.iter().filter_map(|node| match node {
            HNode::Element(element) => Some(element),
            HNode::Text(_) => None,
        })
    }
}

/// Parsed fragment.
#[derive(Debug, Clone, Default)]
pub(crate) struct Fragment {
    pub children: Vec<HNode>,
    /// Whether elements beyond the depth limit were flattened.
    pub depth_limited: bool,
}

#[derive(Debug)]
enum Data {
    Document,
    Element { name: QualName, attrs: Vec<Attribute>, template_contents: Option<usize>, integration_point: bool },
    Text(String),
    Other,
}

#[derive(Debug)]
struct DomNode {
    data: Data,
    parent: Option<usize>,
    children: Vec<usize>,
    explicit_close: bool,
}

/// Arena-backed `TreeSink`.
#[derive(Debug)]
struct Sink {
    nodes: Vec<DomNode>,
    /// Elements believed to be open, used only to detect explicit `</span>` closes.
    shadow: Vec<usize>,
}

const SPECIAL: &[&str] = &[
    "address", "applet", "area", "article", "aside", "base", "basefont", "bgsound", "blockquote", "body", "br",
    "button", "caption", "center", "col", "colgroup", "dd", "details", "dir", "div", "dl", "dt", "embed",
    "fieldset", "figcaption", "figure", "footer", "form", "frame", "frameset", "h1", "h2", "h3", "h4", "h5", "h6",
    "head", "header", "hgroup", "hr", "html", "iframe", "img", "input", "keygen", "li", "link", "listing", "main",
    "marquee", "menu", "meta", "nav", "noembed", "noframes", "noscript", "object", "ol", "p", "param", "plaintext",
    "pre", "script", "search", "section", "select", "source", "style", "summary", "table", "tbody", "td",
    "template", "textarea", "tfoot", "th", "thead", "title", "tr", "track", "ul", "wbr", "xmp",
];

pub(crate) const VOID: &[&str] = &[
    "area", "base", "basefont", "bgsound", "br", "col", "command", "embed", "frame", "hr", "image", "img", "input",
    "keygen", "link", "meta", "param", "source", "track", "wbr",
];

impl Sink {
    fn new() -> Self {
        Self {
            nodes: vec![DomNode { data: Data::Document, parent: None, children: Vec::new(), explicit_close: false }],
            shadow: Vec::new(),
        }
    }

    fn push(&mut self, data: Data) -> usize {
        self.nodes.push(DomNode { data, parent: None, children: Vec::new(), explicit_close: false });
        self.nodes.len() - 1
    }

    fn local(&self, id: usize) -> Option<&str> {
        match &self.nodes[id].data {
            Data::Element { name, .. } => Some(&name.local),
            _ => None,
        }
    }

    fn detach(&mut self, id: usize) {
        if let Some(parent) = self.nodes[id].parent.take() {
            self.nodes[parent].children.retain(|&child| child != id);
        }
    }

    fn observe_insertion(&mut self, parent: usize, child: Option<usize>) {
        if let Some(position) = self.shadow.iter().rposition(|&open| open == parent) {
            self.shadow.truncate(position + 1);
        }
        if let Some(child) = child
            && let Some(name) = self.local(child)
            && !VOID.contains(&name)
        {
            self.shadow.push(child);
        }
    }

    /// Mirrors "any other end tag" closely enough to know which span an end tag closes.
    fn observe_end_tag(&mut self, name: &str) {
        let Some(position) = self.shadow.iter().rposition(|&open| self.local(open) == Some(name)) else {
            return;
        };
        if name == "span"
            && self.shadow[position + 1..]
                .iter()
                .any(|&open| self.local(open).is_some_and(|local| SPECIAL.contains(&local)))
        {
            return;
        }
        if name == "span" {
            let id = self.shadow[position];
            self.nodes[id].explicit_close = true;
        }
        self.shadow.truncate(position);
    }

    fn append_text_to(&mut self, parent: usize, before: Option<usize>, text: &str) {
        let siblings = &self.nodes[parent].children;
        let index = before.and_then(|b| siblings.iter().position(|&c| c == b)).unwrap_or(siblings.len());
        if index > 0 {
            let previous = siblings[index - 1];
            if let Data::Text(existing) = &mut self.nodes[previous].data {
                existing.push_str(text);
                return;
            }
        }
        let id = self.push(Data::Text(text.to_owned()));
        self.nodes[id].parent = Some(parent);
        self.nodes[parent].children.insert(index, id);
    }

    fn insert(&mut self, parent: usize, before: Option<usize>, child: NodeOrText<usize>) {
        match child {
            NodeOrText::AppendText(text) => self.append_text_to(parent, before, &text),
            NodeOrText::AppendNode(id) => {
                self.detach(id);
                let siblings = &self.nodes[parent].children;
                let index = before.and_then(|b| siblings.iter().position(|&c| c == b)).unwrap_or(siblings.len());
                self.nodes[id].parent = Some(parent);
                self.nodes[parent].children.insert(index, id);
            }
        }
    }
}

impl TreeSink for Sink {
    type Handle = usize;
    type Output = Self;

    fn finish(self) -> Self {
        self
    }

    fn parse_error(&mut self, _msg: Cow<'static, str>) {}

    fn get_document(&mut self) -> usize {
        0
    }

    fn elem_name<'a>(&'a self, target: &'a usize) -> ExpandedName<'a> {
        match &self.nodes[*target].data {
            Data::Element { name, .. } => name.expanded(),
            _ => unreachable!("elem_name called on a non-element"),
        }
    }

    fn create_element(&mut self, name: QualName, attrs: Vec<Attribute>, flags: ElementFlags) -> usize {
        let template_contents = flags.template.then(|| self.push(Data::Document));
        self.push(Data::Element { name, attrs, template_contents, integration_point: flags.mathml_annotation_xml_integration_point })
    }

    fn create_comment(&mut self, _text: StrTendril) -> usize {
        self.push(Data::Other)
    }

    fn create_pi(&mut self, _target: StrTendril, _data: StrTendril) -> usize {
        self.push(Data::Other)
    }

    fn append(&mut self, parent: &usize, child: NodeOrText<usize>) {
        let element = match &child {
            NodeOrText::AppendNode(id) => Some(*id),
            NodeOrText::AppendText(_) => None,
        };
        self.insert(*parent, None, child);
        self.observe_insertion(*parent, element);
    }

    fn append_based_on_parent_node(&mut self, element: &usize, prev_element: &usize, child: NodeOrText<usize>) {
        if self.nodes[*element].parent.is_some() {
            self.append_before_sibling(element, child);
        } else {
            self.append(prev_element, child);
        }
    }

    fn append_doctype_to_document(&mut self, _name: StrTendril, _public_id: StrTendril, _system_id: StrTendril) {}

    fn get_template_contents(&mut self, target: &usize) -> usize {
        match &self.nodes[*target].data {
            Data::Element { template_contents: Some(contents), .. } => *contents,
            _ => unreachable!("template contents requested for a non-template"),
        }
    }

    fn same_node(&self, x: &usize, y: &usize) -> bool {
        x == y
    }

    fn set_quirks_mode(&mut self, _mode: QuirksMode) {}

    fn append_before_sibling(&mut self, sibling: &usize, new_node: NodeOrText<usize>) {
        if let Some(parent) = self.nodes[*sibling].parent {
            self.insert(parent, Some(*sibling), new_node);
        }
    }

    fn add_attrs_if_missing(&mut self, target: &usize, attrs: Vec<Attribute>) {
        if let Data::Element { attrs: existing, .. } = &mut self.nodes[*target].data {
            for attr in attrs {
                if !existing.iter().any(|e| e.name == attr.name) {
                    existing.push(attr);
                }
            }
        }
    }

    fn remove_from_parent(&mut self, target: &usize) {
        self.detach(*target);
    }

    fn reparent_children(&mut self, node: &usize, new_parent: &usize) {
        let children = std::mem::take(&mut self.nodes[*node].children);
        for &child in &children {
            self.nodes[child].parent = Some(*new_parent);
        }
        self.nodes[*new_parent].children.extend(children);
    }

    fn is_mathml_annotation_xml_integration_point(&self, handle: &usize) -> bool {
        matches!(self.nodes[*handle].data, Data::Element { integration_point: true, .. })
    }
}

type Builder = TreeBuilder<usize, Sink>;

/// Tokenizer sink that forwards to the tree builder, swallows EOF, and records
/// which raw-text state the tree builder requested.
struct Feed<'a> {
    builder: &'a mut Builder,
    mode: &'a mut TextMode,
}

/// Tokenizer state carried between raw nodes.
#[derive(Debug, Clone, Default)]
struct TextMode {
    raw: Option<(RawKind, String)>,
    plaintext: bool,
}

impl TokenSink for Feed<'_> {
    type Handle = usize;

    fn process_token(&mut self, token: Token, line: u64) -> TokenSinkResult<usize> {
        if matches!(token, Token::EOFToken) {
            return TokenSinkResult::Continue;
        }
        let start_name = match &token {
            Token::TagToken(tag) if tag.kind == StartTag => Some(tag.name.to_string()),
            _ => None,
        };
        if let Token::TagToken(tag) = &token
            && tag.kind == EndTag
        {
            if self.mode.raw.is_some() {
                self.mode.raw = None;
            } else {
                self.builder.sink.observe_end_tag(&tag.name);
            }
        }
        let result = self.builder.process_token(token, line);
        match &result {
            TokenSinkResult::RawData(kind) => self.mode.raw = Some((*kind, start_name.unwrap_or_default())),
            TokenSinkResult::Plaintext => self.mode.plaintext = true,
            _ => {}
        }
        result
    }

    fn adjusted_current_node_present_but_not_in_html_namespace(&self) -> bool {
        self.builder.adjusted_current_node_present_but_not_in_html_namespace()
    }
}

fn new_builder() -> Builder {
    let mut sink = Sink::new();
    let context = create_element(&mut sink, QualName::new(None, ns!(html), local_name!("template")), Vec::new());
    let opts = TreeBuilderOpts { scripting_enabled: false, ..TreeBuilderOpts::default() };
    TreeBuilder::new_for_fragment(sink, context, None, opts)
}

fn tokenize(builder: &mut Builder, mode: &mut TextMode, source: &str) {
    let initial_state = if mode.plaintext {
        Some(State::Plaintext)
    } else {
        mode.raw.as_ref().map(|(kind, _)| State::RawData(*kind))
    };
    let last_start_tag_name = mode.raw.as_ref().map(|(_, name)| name.clone());
    let opts = TokenizerOpts { initial_state, last_start_tag_name, ..TokenizerOpts::default() };
    let mut tokenizer = Tokenizer::new(Feed { builder, mode }, opts);
    let mut input = BufferQueue::default();
    input.push_back(StrTendril::from_slice(source));
    while let TokenizerResult::Script(_) = tokenizer.feed(&mut input) {}
    tokenizer.end();
}

fn finish(mut builder: Builder, source_for_eof: Option<&str>) -> Fragment {
    let _ = builder.process_token(Token::EOFToken, 0);
    let mut sink = builder.sink;
    if let Some(source) = source_for_eof
        && ends_with_span_close(source)
    {
        for id in std::mem::take(&mut sink.shadow) {
            if sink.local(id) == Some("span") {
                sink.nodes[id].explicit_close = true;
            }
        }
    }
    let root = sink.nodes[0].children.iter().copied().find(|&id| sink.local(id) == Some("html"));
    let mut fragment = Fragment::default();
    if let Some(root) = root {
        fragment.children = to_owned_tree(&sink, root, &mut fragment.depth_limited);
    }
    fragment
}

/// Whether `source` ends with `</span\s*>` (ASCII case-insensitive).
pub(crate) fn ends_with_span_close(source: &str) -> bool {
    let trimmed = source.strip_suffix('>').map(|s| s.trim_end_matches(|c: char| c.is_ascii_whitespace()));
    trimmed.is_some_and(|s| s.len() >= 6 && s[s.len() - 6..].eq_ignore_ascii_case("</span"))
}

fn to_owned_tree(sink: &Sink, root: usize, limited: &mut bool) -> Vec<HNode> {
    // Each frame: (arena id, owned children collected so far, element shell).
    struct Frame {
        pending: std::vec::IntoIter<usize>,
        element: Option<HElement>,
        children: Vec<HNode>,
    }
    let mut stack = vec![Frame { pending: sink.nodes[root].children.clone().into_iter(), element: None, children: Vec::new() }];
    loop {
        let next = stack.last_mut().and_then(|frame| frame.pending.next());
        let Some(id) = next else {
            let frame = stack.pop().expect("frame");
            let (Some(mut element), Some(parent)) = (frame.element, stack.last_mut()) else {
                return frame.children;
            };
            element.children = frame.children;
            parent.children.push(HNode::Element(element));
            continue;
        };
        match &sink.nodes[id].data {
            Data::Text(text) => {
                let frame = stack.last_mut().expect("frame");
                if let Some(HNode::Text(previous)) = frame.children.last_mut() {
                    previous.push_str(text);
                } else {
                    frame.children.push(HNode::Text(text.clone()));
                }
            }
            Data::Element { name, attrs, template_contents, .. } => {
                let child_ids = if template_contents.is_some() { Vec::new() } else { sink.nodes[id].children.clone() };
                if stack.len() > MAX_HTML_DEPTH {
                    *limited = true;
                    // Flatten: splice this element's children into the current frame.
                    let frame = stack.last_mut().expect("frame");
                    let rest: Vec<usize> = child_ids.into_iter().chain(frame.pending.by_ref()).collect();
                    frame.pending = rest.into_iter();
                    continue;
                }
                let element = HElement {
                    name: name.local.to_string(),
                    attrs: attrs.iter().map(|a| (a.name.local.to_string(), a.value.to_string())).collect(),
                    children: Vec::new(),
                    explicit_close: sink.nodes[id].explicit_close,
                };
                stack.push(Frame { pending: child_ids.into_iter(), element: Some(element), children: Vec::new() });
            }
            Data::Document | Data::Other => {}
        }
    }
}

/// Parses an HTML fragment like `fromHtml(source, {fragment: true})`.
pub(crate) fn parse_fragment(source: &str) -> Fragment {
    let mut builder = new_builder();
    let mut mode = TextMode::default();
    tokenize(&mut builder, &mut mode, source);
    finish(builder, Some(source))
}

/// Token-level builder matching `hast-util-raw`.
pub(crate) struct RawBuilder {
    builder: Builder,
    mode: TextMode,
}

impl std::fmt::Debug for RawBuilder {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("RawBuilder").finish_non_exhaustive()
    }
}

impl RawBuilder {
    pub(crate) fn new() -> Self {
        Self { builder: new_builder(), mode: TextMode::default() }
    }

    fn in_text_mode(&self) -> bool {
        self.mode.raw.is_some()
    }

    pub(crate) fn start(&mut self, name: &str, attrs: &[(&str, String)]) {
        if self.mode.plaintext || self.in_text_mode() {
            return;
        }
        let tag = Tag {
            kind: StartTag,
            name: LocalName::from(name),
            self_closing: false,
            attrs: attrs
                .iter()
                .map(|(key, value)| Attribute {
                    name: QualName::new(None, ns!(), LocalName::from(*key)),
                    value: StrTendril::from_slice(value),
                })
                .collect(),
        };
        let mut feed = Feed { builder: &mut self.builder, mode: &mut self.mode };
        let _ = feed.process_token(Token::TagToken(tag), 0);
    }

    pub(crate) fn end(&mut self, name: &str) {
        if self.mode.plaintext || VOID.contains(&name) {
            return;
        }
        // In a raw-text element any end tag closes it, as in parse5's `endTagInText`.
        let name = match &self.mode.raw {
            Some((_, open)) => LocalName::from(open.as_str()),
            None => LocalName::from(name),
        };
        let tag = Tag { kind: EndTag, name, self_closing: false, attrs: Vec::new() };
        let mut feed = Feed { builder: &mut self.builder, mode: &mut self.mode };
        let _ = feed.process_token(Token::TagToken(tag), 0);
    }

    pub(crate) fn text(&mut self, value: &str) {
        if value.is_empty() {
            return;
        }
        let mut feed = Feed { builder: &mut self.builder, mode: &mut self.mode };
        let _ = feed.process_token(Token::CharacterTokens(StrTendril::from_slice(value)), 0);
    }

    pub(crate) fn raw(&mut self, value: &str) {
        tokenize(&mut self.builder, &mut self.mode, value);
    }

    pub(crate) fn finish(self) -> Fragment {
        finish(self.builder, None)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn names(nodes: &[HNode]) -> Vec<String> {
        nodes
            .iter()
            .map(|n| match n {
                HNode::Element(e) => format!("<{}>{}", e.name, names(&e.children).join("")),
                HNode::Text(t) => t.clone(),
            })
            .collect()
    }

    #[test]
    fn parses_stray_table_rows_with_template_context() {
        let fragment = parse_fragment("<tr><td>a</td></tr>");
        assert_eq!(names(&fragment.children), ["<tr><td>a"]);
    }

    #[test]
    fn detects_explicit_span_closes() {
        let explicit = parse_fragment("<div>a <span>b</span> c</div>");
        let HNode::Element(div) = &explicit.children[0] else { panic!() };
        let span = div.elements().next().unwrap();
        assert!(span.explicit_close);

        let by_div = parse_fragment("<div>a <span>b</div>");
        let HNode::Element(div) = &by_div.children[0] else { panic!() };
        assert!(!div.elements().next().unwrap().explicit_close);

        let eof = parse_fragment("<span>a<span>b</span>");
        let HNode::Element(outer) = &eof.children[0] else { panic!() };
        assert!(outer.explicit_close, "source ends with </span>, like the parse5 position check");

        let open = parse_fragment("<span>a");
        let HNode::Element(span) = &open.children[0] else { panic!() };
        assert!(!span.explicit_close);
    }

    #[test]
    fn raw_builder_isolates_raw_nodes() {
        let mut builder = RawBuilder::new();
        builder.start("p", &[]);
        builder.raw("<span data-x=\"true");
        builder.text(" after");
        builder.end("p");
        builder.raw("<textarea>");
        builder.start("strong", &[]);
        builder.text("x");
        builder.end("strong");
        builder.raw("<plaintext>");
        builder.start("em", &[]);
        builder.end("em");
        let fragment = builder.finish();
        assert_eq!(names(&fragment.children), ["<p> after", "<textarea>x", "<plaintext>"]);
    }

    #[test]
    fn deep_nesting_is_flattened() {
        let fragment = parse_fragment(&"<b>".repeat(5000));
        assert!(fragment.depth_limited);
        let mut depth = 0;
        let mut nodes = &fragment.children;
        while let Some(HNode::Element(e)) = nodes.first() {
            depth += 1;
            nodes = &e.children;
        }
        assert!(depth <= MAX_HTML_DEPTH + 1);
    }
}
