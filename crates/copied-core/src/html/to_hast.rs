//! mdast → HTML tree (`mdast-util-to-hast` with `allowDangerousHtml` and an
//! empty clobber prefix, followed by `hast-util-raw`).

use super::dom::{Fragment, RawBuilder};
use crate::ast::Node;
use crate::url::normalize_uri;
use std::collections::HashMap;

/// Intermediate hast node before raw HTML is parsed.
#[derive(Debug, Clone)]
enum Hast {
    Element { name: String, attrs: Vec<(&'static str, String)>, children: Vec<Hast> },
    Text(String),
    Raw(String),
}

fn el(name: &str, attrs: Vec<(&'static str, String)>, children: Vec<Hast>) -> Hast {
    Hast::Element { name: name.to_owned(), attrs, children }
}

fn text(value: impl Into<String>) -> Hast {
    Hast::Text(value.into())
}

fn wrap(nodes: Vec<Hast>, loose: bool) -> Vec<Hast> {
    let mut result = Vec::with_capacity(nodes.len() * 2 + 1);
    if loose {
        result.push(text("\n"));
    }
    let any = !nodes.is_empty();
    for (index, node) in nodes.into_iter().enumerate() {
        if index > 0 {
            result.push(text("\n"));
        }
        result.push(node);
    }
    if loose && any {
        result.push(text("\n"));
    }
    result
}

/// `trim-lines`: strips spaces and tabs around line endings.
fn trim_lines(value: &str) -> String {
    let mut out = String::with_capacity(value.len());
    let mut rest = value;
    let mut first = true;
    loop {
        let (line, ending, next) = match rest.find(['\r', '\n']) {
            Some(i) => {
                let len = if rest[i..].starts_with("\r\n") { 2 } else { 1 };
                (&rest[..i], &rest[i..i + len], Some(&rest[i + len..]))
            }
            None => (rest, "", None),
        };
        let mut trimmed = line;
        if !first {
            trimmed = trimmed.trim_start_matches([' ', '\t']);
        }
        if next.is_some() {
            trimmed = trimmed.trim_end_matches([' ', '\t']);
        }
        out.push_str(trimmed);
        out.push_str(ending);
        first = false;
        match next {
            Some(next) => rest = next,
            None => return out,
        }
    }
}

fn footnote_safe_id(identifier: &str) -> String {
    normalize_uri(&identifier.to_uppercase().to_lowercase())
}

/// Footnote bookkeeping (`footnoteById`, `footnoteOrder`, `footnoteCounts`).
struct State<'a> {
    by_id: HashMap<String, &'a [Node]>,
    order: Vec<String>,
    counts: HashMap<String, usize>,
}

impl<'a> State<'a> {
    fn all(&mut self, nodes: &'a [Node], parent: Option<&'a Node>) -> Vec<Hast> {
        let mut values = Vec::new();
        for (index, node) in nodes.iter().enumerate() {
            let mut result = self.one(node, parent);
            if index > 0 && matches!(nodes[index - 1], Node::Break)
                && let Some(first) = result.first_mut()
            {
                match first {
                    Hast::Text(value) => *value = value.trim_start_matches([' ', '\t']).to_owned(),
                    Hast::Element { children, .. } => {
                        if let Some(Hast::Text(value)) = children.first_mut() {
                            *value = value.trim_start_matches([' ', '\t']).to_owned();
                        }
                    }
                    Hast::Raw(_) => {}
                }
            }
            values.extend(result);
        }
        values
    }

    #[allow(clippy::too_many_lines)]
    fn one(&mut self, node: &'a Node, parent: Option<&'a Node>) -> Vec<Hast> {
        vec![match node {
            Node::Paragraph(children) => el("p", vec![], self.all(children, Some(node))),
            Node::Heading { depth, children } => el(&format!("h{depth}"), vec![], self.all(children, Some(node))),
            Node::ThematicBreak => el("hr", vec![], vec![]),
            Node::Blockquote(children) => {
                let content = self.all(children, Some(node));
                el("blockquote", vec![], wrap(content, true))
            }
            Node::List { ordered, start, children, .. } => {
                let results = self.all(children, Some(node));
                let mut attrs = Vec::new();
                if let Some(start) = start
                    && *start != 1
                {
                    attrs.push(("start", start.to_string()));
                }
                let task = children.iter().any(|c| matches!(c, Node::ListItem { checked: Some(_), .. }));
                if task {
                    attrs.push(("class", "contains-task-list".to_owned()));
                }
                el(if *ordered { "ol" } else { "ul" }, attrs, wrap(results, true))
            }
            Node::ListItem { checked, children, spread } => {
                let mut results = self.all(children, Some(node));
                let loose = match parent {
                    Some(Node::List { spread: list_spread, children: siblings, .. }) => {
                        *list_spread || siblings.iter().any(|s| matches!(s, Node::ListItem { spread: true, .. }))
                    }
                    _ => *spread,
                };
                let mut attrs = Vec::new();
                if let Some(checked) = checked {
                    let head_is_p = matches!(results.first(), Some(Hast::Element { name, .. }) if name == "p");
                    if !head_is_p {
                        results.insert(0, el("p", vec![], vec![]));
                    }
                    if let Some(Hast::Element { children: p, .. }) = results.first_mut() {
                        if !p.is_empty() {
                            p.insert(0, text(" "));
                        }
                        let mut input = vec![("type", "checkbox".to_owned())];
                        if *checked {
                            input.push(("checked", String::new()));
                        }
                        input.push(("disabled", String::new()));
                        p.insert(0, el("input", input, vec![]));
                    }
                    attrs.push(("class", "task-list-item".to_owned()));
                }
                let mut out = Vec::new();
                let count = results.len();
                let tail_is_p = matches!(results.last(), Some(Hast::Element { name, .. }) if name == "p");
                for (index, child) in results.into_iter().enumerate() {
                    let is_p = matches!(&child, Hast::Element { name, .. } if name == "p");
                    if loose || index != 0 || !is_p {
                        out.push(text("\n"));
                    }
                    match child {
                        Hast::Element { name, children, .. } if name == "p" && !loose => out.extend(children),
                        other => out.push(other),
                    }
                }
                if count > 0 && (loose || !tail_is_p) {
                    out.push(text("\n"));
                }
                el("li", attrs, out)
            }
            Node::Html(value) => Hast::Raw(value.clone()),
            Node::Code { value, lang, .. } => {
                let content = if value.is_empty() { String::new() } else { format!("{value}\n") };
                let mut attrs = Vec::new();
                if let Some(lang) = lang.as_deref().filter(|l| !l.is_empty()) {
                    let first = lang.split(char::is_whitespace).next().unwrap_or("");
                    attrs.push(("class", format!("language-{first}")));
                }
                el("pre", vec![], vec![el("code", attrs, vec![text(content)])])
            }
            Node::Text(value) => text(trim_lines(value)),
            Node::Emphasis(children) => el("em", vec![], self.all(children, Some(node))),
            Node::Strong(children) => el("strong", vec![], self.all(children, Some(node))),
            Node::Delete(children) => el("del", vec![], self.all(children, Some(node))),
            Node::InlineCode(value) => {
                let value = value.replace("\r\n", " ").replace(['\r', '\n'], " ");
                el("code", vec![], vec![text(value)])
            }
            Node::Break => return vec![el("br", vec![], vec![]), text("\n")],
            Node::Link { url, title, children } => {
                let mut attrs = vec![("href", normalize_uri(url))];
                if let Some(title) = title {
                    attrs.push(("title", title.clone()));
                }
                el("a", attrs, self.all(children, Some(node)))
            }
            Node::Image { url, title, alt } => {
                let mut attrs = vec![("src", normalize_uri(url)), ("alt", alt.clone())];
                if let Some(title) = title {
                    attrs.push(("title", title.clone()));
                }
                el("img", attrs, vec![])
            }
            Node::LinkReference { children, .. } => return self.all(children, Some(node)),
            Node::FootnoteReference { identifier, .. } => {
                let id = identifier.to_uppercase();
                let safe_id = footnote_safe_id(identifier);
                let reuse = self.counts.get(&id).copied();
                let counter = match reuse {
                    None => {
                        self.order.push(id.clone());
                        self.order.len()
                    }
                    Some(_) => self.order.iter().position(|o| *o == id).map_or(0, |i| i + 1),
                };
                let count = reuse.unwrap_or(0) + 1;
                self.counts.insert(id, count);
                let suffix = if count > 1 { format!("-{count}") } else { String::new() };
                let link = el(
                    "a",
                    vec![
                        ("href", format!("#fn-{safe_id}")),
                        ("id", format!("fnref-{safe_id}{suffix}")),
                        ("data-footnote-ref", String::new()),
                        ("aria-describedby", "footnote-label".to_owned()),
                    ],
                    vec![text(counter.to_string())],
                );
                el("sup", vec![], vec![link])
            }
            Node::Table { align, children } => {
                let mut rows: Vec<Hast> = children
                    .iter()
                    .enumerate()
                    .map(|(row_index, row)| self.table_row(row, row_index, align))
                    .collect();
                let mut content = Vec::new();
                if !rows.is_empty() {
                    let first = rows.remove(0);
                    content.push(el("thead", vec![], wrap(vec![first], true)));
                }
                if !rows.is_empty() {
                    content.push(el("tbody", vec![], wrap(rows, true)));
                }
                el("table", vec![], wrap(content, true))
            }
            Node::TableRow(_) => self.table_row(node, 1, &[]),
            Node::TableCell(children) => el("td", vec![], self.all(children, Some(node))),
            Node::ImageReference { alt, .. } => text(alt.clone()),
            Node::Definition { .. } | Node::FootnoteDefinition { .. } => return Vec::new(),
        }]
    }

    fn table_row(&mut self, row: &'a Node, row_index: usize, align: &[crate::ast::Align]) -> Hast {
        let cells: &[Node] = row.children().map_or(&[], Vec::as_slice);
        let tag = if row_index == 0 { "th" } else { "td" };
        let length = if align.is_empty() { cells.len() } else { align.len() };
        let mut out = Vec::with_capacity(length);
        for index in 0..length {
            let mut attrs = Vec::new();
            if let Some(value) = align.get(index).and_then(|a| a.as_str()) {
                attrs.push(("align", value.to_owned()));
            }
            let children = match cells.get(index) {
                Some(cell) => self.all(cell.children().map_or(&[], Vec::as_slice), Some(cell)),
                None => Vec::new(),
            };
            out.push(el(tag, attrs, children));
        }
        el("tr", vec![], wrap(out, true))
    }

    fn footer(&mut self) -> Option<Hast> {
        let mut items = Vec::new();
        let mut reference_index = 0;
        while reference_index < self.order.len() {
            let id = self.order[reference_index].clone();
            let Some(definition) = self.by_id.get(&id).copied() else {
                reference_index += 1;
                continue;
            };
            let mut content = self.all(definition, None);
            let safe_id = footnote_safe_id(&id);
            let counts = self.counts.get(&id).copied().unwrap_or(0);
            let mut back = Vec::new();
            for rereference in 1..=counts {
                if !back.is_empty() {
                    back.push(text(" "));
                }
                let suffix = if rereference > 1 { format!("-{rereference}") } else { String::new() };
                let mut children = vec![text("↩")];
                if rereference > 1 {
                    children.push(el("sup", vec![], vec![text(rereference.to_string())]));
                }
                back.push(el(
                    "a",
                    vec![
                        ("href", format!("#fnref-{safe_id}{suffix}")),
                        ("data-footnote-backref", String::new()),
                        ("aria-label", format!("Back to reference {}{suffix}", reference_index + 1)),
                        ("class", "data-footnote-backref".to_owned()),
                    ],
                    children,
                ));
            }
            match content.last_mut() {
                Some(Hast::Element { name, children, .. }) if name == "p" => {
                    match children.last_mut() {
                        Some(Hast::Text(value)) => value.push(' '),
                        _ => children.push(text(" ")),
                    }
                    children.extend(back);
                }
                _ => content.extend(back),
            }
            items.push(el("li", vec![("id", format!("fn-{safe_id}"))], wrap(content, true)));
            reference_index += 1;
        }
        if items.is_empty() {
            return None;
        }
        Some(el(
            "section",
            vec![("data-footnotes", String::new()), ("class", "footnotes".to_owned())],
            vec![
                el("h2", vec![("class", "sr-only".to_owned()), ("id", "footnote-label".to_owned())], vec![text("Footnotes")]),
                text("\n"),
                el("ol", vec![], wrap(items, true)),
                text("\n"),
            ],
        ))
    }
}

fn collect_footnotes<'a>(nodes: &'a [Node], map: &mut HashMap<String, &'a [Node]>) {
    for node in nodes {
        if let Node::FootnoteDefinition { identifier, children } = node {
            map.entry(identifier.to_uppercase()).or_insert(children.as_slice());
        }
        if let Some(children) = node.children() {
            collect_footnotes(children, map);
        }
    }
}

fn feed(builder: &mut RawBuilder, nodes: &[Hast]) {
    for node in nodes {
        match node {
            Hast::Text(value) => builder.text(value),
            Hast::Raw(value) => builder.raw(value),
            Hast::Element { name, attrs, children } => {
                builder.start(name, attrs);
                feed(builder, children);
                builder.end(name);
            }
        }
    }
}

/// Converts mdast into a parsed, not yet sanitized, HTML fragment.
pub(crate) fn to_html_tree(children: &[Node]) -> Fragment {
    let mut by_id = HashMap::new();
    collect_footnotes(children, &mut by_id);
    let mut state = State { by_id, order: Vec::new(), counts: HashMap::new() };
    let content = state.all(children, None);
    let mut root = wrap(content, false);
    if let Some(footer) = state.footer() {
        root.push(text("\n"));
        root.push(footer);
    }
    let mut builder = RawBuilder::new();
    feed(&mut builder, &root);
    builder.finish()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn trims_line_edges() {
        assert_eq!(trim_lines("  a  \n  b  \r\n c "), "  a\nb\r\nc ");
    }
}
