//! HTML5 serializer matching `hast-util-to-html` defaults.
//!
//! Text escapes `<` and `&`; double-quoted attribute values escape NUL, `"`, `&`,
//! `'` and `` ` ``, all as uppercase hexadecimal character references. Boolean
//! attributes serialize as bare names and void elements have no end tag.

use super::dom::{HElement, HNode, VOID};
use std::fmt::Write as _;

/// HTML attributes whose hast property is a boolean.
pub(crate) const BOOLEAN_ATTRS: &[&str] = &[
    "checked",
    "compact",
    "disabled",
    "ismap",
    "itemscope",
    "multiple",
    "nohref",
    "noshade",
    "nowrap",
    "open",
    "readonly",
    "selected",
];

fn escape_into(out: &mut String, value: &str, attribute: bool) {
    for ch in value.chars() {
        let escaped = if attribute {
            matches!(ch, '\0' | '"' | '&' | '\'' | '`')
        } else {
            matches!(ch, '<' | '&')
        };
        if escaped {
            let _ = write!(out, "&#x{:X};", u32::from(ch));
        } else {
            out.push(ch);
        }
    }
}

/// Serializes nodes.
pub(crate) fn serialize(nodes: &[HNode]) -> String {
    let mut out = String::new();
    for node in nodes {
        serialize_node(&mut out, node);
    }
    out
}

fn serialize_node(out: &mut String, node: &HNode) {
    match node {
        HNode::Text(text) => escape_into(out, text, false),
        HNode::Element(element) => serialize_element(out, element),
    }
}

fn serialize_element(out: &mut String, element: &HElement) {
    out.push('<');
    out.push_str(&element.name);
    for (name, value) in &element.attrs {
        out.push(' ');
        out.push_str(name);
        if BOOLEAN_ATTRS.contains(&name.as_str()) {
            continue;
        }
        out.push_str("=\"");
        escape_into(out, value, true);
        out.push('"');
    }
    out.push('>');
    if VOID.contains(&element.name.as_str()) {
        return;
    }
    for child in &element.children {
        serialize_node(out, child);
    }
    out.push_str("</");
    out.push_str(&element.name);
    out.push('>');
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn escapes_like_hast_util_to_html() {
        let element = HElement::new(
            "a",
            vec![
                ("href".into(), "a&b\"c'd`".into()),
                ("disabled".into(), String::new()),
            ],
            vec![HNode::Text("<x> & y".into())],
        );
        assert_eq!(
            serialize(&[HNode::Element(element)]),
            "<a href=\"a&#x26;b&#x22;c&#x27;d&#x60;\" disabled>&#x3C;x> &#x26; y</a>"
        );
        assert_eq!(
            serialize(&[HNode::Element(HElement::new("br", vec![], vec![]))]),
            "<br>"
        );
    }
}
