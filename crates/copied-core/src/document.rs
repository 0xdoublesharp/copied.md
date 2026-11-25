//! Markdown parsing and the shared document pass (`document.ts`).

use crate::ast::{self, Node};
use crate::limits;
use crate::url::{has_scheme_prefix, js_trim, safe_url};
use crate::{Diagnostic, DiagnosticCode, DiagnosticSeverity};
use indexmap::IndexMap;
use markdown::{ParseOptions, to_mdast};
use std::collections::HashMap;

/// A parsed document with resolved links and its URL diagnostics.
#[derive(Debug, Clone)]
pub(crate) struct Document {
    pub children: Vec<Node>,
    pub diagnostics: Vec<Diagnostic>,
}

fn limit_diagnostic(limit: limits::Limit) -> Diagnostic {
    let message = match limit {
        limits::Limit::Bytes => "The Markdown is too large to format and was copied as plain text.",
        limits::Limit::Depth => "The Markdown is nested too deeply to format and was copied as plain text.",
        limits::Limit::Labels => "The Markdown has too many brackets to format and was copied as plain text.",
    };
    Diagnostic { code: DiagnosticCode::InputLimitExceeded, severity: DiagnosticSeverity::Warning, message: message.into() }
}

/// Parses GFM Markdown, resolving references and validating destinations.
pub(crate) fn read_markdown(markdown: &str, source_url: Option<&str>) -> Document {
    let limit = limits::check(markdown);
    let parsed = if limit.is_none() {
        to_mdast(markdown, &ParseOptions::gfm()).ok().map(|tree| ast::root_children(&tree))
    } else {
        None
    };
    let mut children = match parsed {
        Some(children) if ast::depth(&children) <= limits::MAX_NESTING_DEPTH * 2 => children,
        _ => {
            let limit = limit.unwrap_or(limits::Limit::Depth);
            let children =
                if markdown.is_empty() { Vec::new() } else { vec![Node::Paragraph(vec![Node::Text(markdown.to_owned())])] };
            return Document { children, diagnostics: vec![limit_diagnostic(limit)] };
        }
    };

    let mut definitions: HashMap<String, (String, Option<String>)> = HashMap::new();
    ast::walk(&children, &mut |node| {
        if let Node::Definition { identifier, url, title } = node {
            definitions.entry(identifier.clone()).or_insert_with(|| (url.clone(), title.clone()));
        }
    });

    let mut resolver = Resolver { definitions: &definitions, source_url, diagnostics: IndexMap::new() };
    children = resolver.resolve(children);
    let diagnostics = resolver.diagnostics.into_values().collect();
    strip_definitions(&mut children);
    Document { children, diagnostics }
}

struct Resolver<'a> {
    definitions: &'a HashMap<String, (String, Option<String>)>,
    source_url: Option<&'a str>,
    diagnostics: IndexMap<DiagnosticCode, Diagnostic>,
}

impl Resolver<'_> {
    /// Validates a destination, recording why an unsafe one was dropped.
    fn check(&mut self, destination: &str) -> Option<String> {
        if destination.is_empty() {
            return None;
        }
        let url = safe_url(destination, self.source_url);
        if url.is_none() {
            let protocol = has_scheme_prefix(js_trim(destination));
            let (code, message) = if protocol {
                (DiagnosticCode::UnsupportedUrl, "An unsupported link destination was omitted.")
            } else {
                (DiagnosticCode::RelativeUrl, "A relative link needs an original page URL.")
            };
            self.diagnostics.insert(
                code,
                Diagnostic { code, severity: DiagnosticSeverity::Warning, message: message.into() },
            );
        }
        url
    }

    fn definition(&self, identifier: &str) -> (String, Option<String>) {
        self.definitions.get(identifier).cloned().unwrap_or_default()
    }

    fn resolve(&mut self, nodes: Vec<Node>) -> Vec<Node> {
        let mut out = Vec::with_capacity(nodes.len());
        for node in nodes {
            match node {
                Node::Link { url, title, children } => match self.check(&url) {
                    Some(url) => out.push(Node::Link { url, title, children: self.resolve(children) }),
                    None => out.extend(self.resolve(children)),
                },
                Node::LinkReference { identifier, children, .. } => {
                    let (url, title) = self.definition(&identifier);
                    match self.check(&url) {
                        Some(url) => out.push(Node::Link { url, title, children: self.resolve(children) }),
                        None => out.extend(self.resolve(children)),
                    }
                }
                Node::Image { url, title, alt } => match self.check(&url) {
                    Some(url) => out.push(Node::Image { url, title, alt }),
                    None => out.push(Node::Text(alt)),
                },
                Node::ImageReference { identifier, alt, .. } => {
                    let (url, title) = self.definition(&identifier);
                    match self.check(&url) {
                        Some(url) => out.push(Node::Image { url, title, alt }),
                        None => out.push(Node::Text(alt)),
                    }
                }
                mut other => {
                    if let Some(children) = other.children_mut() {
                        let taken = std::mem::take(children);
                        *children = self.resolve(taken);
                    }
                    out.push(other);
                }
            }
        }
        out
    }
}

fn strip_definitions(nodes: &mut Vec<Node>) {
    nodes.retain(|node| !matches!(node, Node::Definition { .. }));
    for node in nodes {
        if let Some(children) = node.children_mut() {
            strip_definitions(children);
        }
    }
}

/// Footnote numbers by order of first reference (`footnoteNumbers`).
pub(crate) fn footnote_numbers(nodes: &[Node]) -> HashMap<String, usize> {
    let mut numbers = HashMap::new();
    ast::walk(nodes, &mut |node| {
        if let Node::FootnoteReference { identifier, .. } = node {
            let next = numbers.len() + 1;
            numbers.entry(identifier.clone()).or_insert(next);
        }
    });
    numbers
}

/// Whether the tree contains an `image` node.
pub(crate) fn has_images(nodes: &[Node]) -> bool {
    let mut found = false;
    ast::walk(nodes, &mut |node| found |= matches!(node, Node::Image { .. }));
    found
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn resolves_references_first_definition_wins() {
        let doc = read_markdown("[a][r] ![i][r]\n\n[r]: https://one.test \"T\"\n[r]: https://two.test", None);
        let Node::Paragraph(children) = &doc.children[0] else { panic!() };
        assert!(matches!(&children[0], Node::Link { url, title: Some(t), .. } if url == "https://one.test" && t == "T"));
        assert!(matches!(&children[2], Node::Image { url, .. } if url == "https://one.test"));
        assert_eq!(doc.children.len(), 1, "definitions are stripped");
        assert_eq!(doc.diagnostics, Vec::new());
    }

    #[test]
    fn diagnostics_keep_first_insertion_order() {
        let doc = read_markdown("[a](./x) [b](javascript:x) [c](./y) ![d](data:x)", None);
        let codes: Vec<_> = doc.diagnostics.iter().map(|d| d.code).collect();
        assert_eq!(codes, [DiagnosticCode::RelativeUrl, DiagnosticCode::UnsupportedUrl]);
        let Node::Paragraph(children) = &doc.children[0] else { panic!() };
        assert_eq!(children.last(), Some(&Node::Text("d".into())));
    }

    #[test]
    fn numbers_footnotes_by_first_reference() {
        let doc = read_markdown("a[^b] c[^a] d[^b]\n\n[^a]: A\n[^b]: B", None);
        let numbers = footnote_numbers(&doc.children);
        assert_eq!(numbers["b"], 1);
        assert_eq!(numbers["a"], 2);
    }

    #[test]
    fn oversized_input_is_literal() {
        let source = ">".repeat(1000);
        let doc = read_markdown(&source, None);
        assert_eq!(doc.children, vec![Node::Paragraph(vec![Node::Text(source)])]);
        assert_eq!(doc.diagnostics[0].code, DiagnosticCode::InputLimitExceeded);
    }
}
