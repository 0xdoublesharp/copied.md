//! `rehype-sanitize` with the copied.md schema, plus the oracle's pre- and
//! post-sanitization passes (`rich-html.ts`).
//!
//! The schema is GitHub's `defaultSchema` with `href` limited to `http`, `https`
//! and `mailto`, and `style`, `template`, `iframe` and `object` stripped along
//! with `script`.

use super::dom::{HElement, HNode};
use crate::url::safe_url;

const TAG_NAMES: &[&str] = &[
    "a",
    "b",
    "blockquote",
    "br",
    "code",
    "dd",
    "del",
    "details",
    "div",
    "dl",
    "dt",
    "em",
    "h1",
    "h2",
    "h3",
    "h4",
    "h5",
    "h6",
    "hr",
    "i",
    "img",
    "input",
    "ins",
    "kbd",
    "li",
    "ol",
    "p",
    "picture",
    "pre",
    "q",
    "rp",
    "rt",
    "ruby",
    "s",
    "samp",
    "section",
    "source",
    "span",
    "strike",
    "strong",
    "sub",
    "summary",
    "sup",
    "table",
    "tbody",
    "td",
    "tfoot",
    "th",
    "thead",
    "tr",
    "tt",
    "ul",
    "var",
];

/// Elements removed together with their content.
const STRIP: &[&str] = &["script", "style", "template", "iframe", "object"];

const TABLE_PARTS: &[&str] = &["tbody", "td", "th", "thead", "tfoot", "tr"];

const GLOBAL_ATTRS: &[&str] = &[
    "abbr",
    "accept",
    "accept-charset",
    "accesskey",
    "action",
    "align",
    "alt",
    "axis",
    "border",
    "cellpadding",
    "cellspacing",
    "char",
    "charoff",
    "charset",
    "checked",
    "clear",
    "colspan",
    "color",
    "cols",
    "compact",
    "coords",
    "datetime",
    "dir",
    "enctype",
    "frame",
    "hspace",
    "headers",
    "height",
    "hreflang",
    "for",
    "id",
    "ismap",
    "itemprop",
    "label",
    "lang",
    "maxlength",
    "media",
    "method",
    "multiple",
    "name",
    "nohref",
    "noshade",
    "nowrap",
    "open",
    "prompt",
    "readonly",
    "rev",
    "rowspan",
    "rows",
    "rules",
    "scope",
    "selected",
    "shape",
    "size",
    "span",
    "start",
    "summary",
    "tabindex",
    "title",
    "usemap",
    "valign",
    "value",
    "width",
];

const ARIA: &[&str] = &["aria-describedby", "aria-label", "aria-labelledby"];
const CLOBBER: &[&str] = &["aria-describedby", "aria-labelledby", "id", "name"];
pub(crate) const CLOBBER_PREFIX: &str = "user-content-";

#[derive(Debug, Clone, Copy)]
enum Allowed {
    Any,
    Value(&'static str),
    True,
    LanguageClass,
}

fn specific(tag: &str, attr: &str) -> Option<Allowed> {
    let any = |names: &[&str]| names.contains(&attr).then_some(Allowed::Any);
    match tag {
        "a" => match attr {
            "class" => Some(Allowed::Value("data-footnote-backref")),
            _ => any(ARIA).or_else(|| any(&["data-footnote-backref", "data-footnote-ref", "href"])),
        },
        "blockquote" | "del" | "ins" | "q" => any(&["cite"]),
        "code" => (attr == "class").then_some(Allowed::LanguageClass),
        "div" => any(&["itemscope", "itemtype"]),
        "dl" | "summary" | "table" => any(ARIA),
        "h2" => (attr == "class").then_some(Allowed::Value("sr-only")),
        "img" => any(ARIA).or_else(|| any(&["longdesc", "src"])),
        "input" => match attr {
            "disabled" => Some(Allowed::True),
            "type" => Some(Allowed::Value("checkbox")),
            _ => None,
        },
        "li" => (attr == "class").then_some(Allowed::Value("task-list-item")),
        "ol" | "ul" => match attr {
            "class" => Some(Allowed::Value("contains-task-list")),
            _ => any(ARIA),
        },
        "section" => match attr {
            "class" => Some(Allowed::Value("footnotes")),
            _ => any(&["data-footnotes"]),
        },
        "source" => any(&["srcset"]),
        _ => None,
    }
}

fn protocols(attr: &str) -> Option<&'static [&'static str]> {
    match attr {
        "cite" | "longdesc" | "src" => Some(&["http", "https"]),
        "href" => Some(&["http", "https", "mailto"]),
        _ => None,
    }
}

/// hast property value derived from an attribute.
#[derive(Debug, Clone, PartialEq)]
enum Value {
    True,
    Str(String),
    Num(f64),
    List(Vec<Value>, bool),
}

const NUMBER_ATTRS: &[&str] = &[
    "cols",
    "height",
    "maxlength",
    "rows",
    "rowspan",
    "size",
    "span",
    "start",
    "tabindex",
    "width",
    "border",
    "hspace",
];
const SPACE_ATTRS: &[&str] = &[
    "accept-charset",
    "accesskey",
    "class",
    "headers",
    "for",
    "itemprop",
    "itemtype",
    "aria-describedby",
    "aria-labelledby",
];

/// JavaScript `Number(string)` for the forms HTML attributes use.
fn js_number(value: &str) -> Option<f64> {
    let trimmed = crate::url::js_trim(value);
    if trimmed.is_empty() {
        return Some(0.0);
    }
    let (sign, body) = match trimmed.as_bytes()[0] {
        b'-' => (-1.0, &trimmed[1..]),
        b'+' => (1.0, &trimmed[1..]),
        _ => (1.0, trimmed),
    };
    if body == "Infinity" {
        return Some(sign * f64::INFINITY);
    }
    let radix = |prefix: &[&str], radix| {
        prefix
            .iter()
            .find_map(|p| trimmed.strip_prefix(p))
            .and_then(|digits| u64::from_str_radix(digits, radix).ok())
    };
    #[allow(clippy::cast_precision_loss)]
    if let Some(n) = radix(&["0x", "0X"], 16)
        .or_else(|| radix(&["0o", "0O"], 8))
        .or_else(|| radix(&["0b", "0B"], 2))
    {
        return Some(n as f64);
    }
    if body.is_empty()
        || !body
            .bytes()
            .all(|b| b.is_ascii_digit() || matches!(b, b'.' | b'e' | b'E' | b'+' | b'-'))
    {
        return None;
    }
    body.parse::<f64>().ok().map(|n| sign * n)
}

/// JavaScript `String(number)` for finite and infinite values.
pub(crate) fn js_number_string(n: f64) -> String {
    if n.is_infinite() {
        return if n > 0.0 {
            "Infinity".into()
        } else {
            "-Infinity".into()
        };
    }
    if n == 0.0 {
        return "0".into();
    }
    if n.fract() == 0.0 && n.abs() < 1e21 {
        return format!("{n:.0}");
    }
    format!("{n}")
}

fn primitive(attr: &str, value: &str) -> Value {
    if super::serialize::BOOLEAN_ATTRS.contains(&attr)
        && (value.is_empty() || value.eq_ignore_ascii_case(attr))
    {
        return Value::True;
    }
    if NUMBER_ATTRS.contains(&attr)
        && !value.is_empty()
        && let Some(n) = js_number(value)
    {
        return Value::Num(n);
    }
    Value::Str(value.to_owned())
}

fn parse_value(attr: &str, value: &str) -> Value {
    if SPACE_ATTRS.contains(&attr) {
        let items = value
            .split(['\t', '\n', '\x0c', '\r', ' '])
            .filter(|s| !s.is_empty());
        return Value::List(items.map(|s| Value::Str(s.to_owned())).collect(), false);
    }
    if matches!(attr, "accept" | "coords") {
        let parts: Vec<&str> = value.split(',').collect();
        let last = parts.len() - 1;
        let items = parts
            .iter()
            .enumerate()
            .map(|(i, part)| (i, part.trim_matches(|c: char| c.is_ascii_whitespace())))
            .filter(|(i, token)| !token.is_empty() || *i != last)
            .map(|(_, token)| match (attr, js_number(token)) {
                ("coords", Some(n)) if !token.is_empty() => Value::Num(n),
                _ => Value::Str(token.to_owned()),
            })
            .collect();
        return Value::List(items, true);
    }
    primitive(attr, value)
}

fn value_string(value: &Value) -> String {
    match value {
        Value::True => String::new(),
        Value::Str(s) => s.clone(),
        Value::Num(n) => js_number_string(*n),
        Value::List(items, comma) => {
            let mut parts: Vec<String> = items.iter().map(value_string).collect();
            if *comma {
                if parts.last().is_some_and(String::is_empty) {
                    parts.push(String::new());
                }
                parts.join(", ").trim().to_owned()
            } else {
                parts.join(" ")
            }
        }
    }
}

fn safe_protocol(attr: &str, value: &Value) -> bool {
    let Some(allowed) = protocols(attr) else {
        return true;
    };
    let url = value_string(value);
    let Some(colon) = url.find(':') else {
        return true;
    };
    let before = |c: char| url.find(c).is_some_and(|i| colon > i);
    if before('/') || before('?') || before('#') {
        return true;
    }
    allowed
        .iter()
        .any(|p| colon == p.len() && url.starts_with(p))
}

fn primitive_ok(attr: &str, allowed: Allowed, value: Value) -> Option<Value> {
    if matches!(value, Value::List(..)) || !safe_protocol(attr, &value) {
        return None;
    }
    let ok = match allowed {
        Allowed::Any => true,
        Allowed::Value(expected) => value == Value::Str(expected.to_owned()),
        Allowed::True => value == Value::True,
        Allowed::LanguageClass => match &value {
            Value::Str(s) => s.strip_prefix("language-").is_some_and(|rest| {
                !rest.is_empty() && !rest.starts_with(['\n', '\r', '\u{2028}', '\u{2029}'])
            }),
            _ => false,
        },
    };
    if !ok {
        return None;
    }
    if CLOBBER.contains(&attr) {
        return Some(Value::Str(format!(
            "{CLOBBER_PREFIX}{}",
            value_string(&value)
        )));
    }
    Some(value)
}

fn property(attr: &str, allowed: Allowed, value: &Value) -> Option<Value> {
    match value {
        Value::List(items, comma) => Some(Value::List(
            items
                .iter()
                .filter_map(|item| primitive_ok(attr, allowed, item.clone()))
                .filter(|v| !matches!(v, Value::True))
                .collect(),
            *comma,
        )),
        other => primitive_ok(attr, allowed, other.clone()),
    }
}

fn clean_attrs(tag: &str, attrs: &[(String, String)]) -> Vec<(String, String)> {
    let mut result: Vec<(String, String)> = Vec::new();
    for (name, raw) in attrs {
        let value = parse_value(name, raw);
        let safe = specific(tag, name)
            .and_then(|allowed| property(name, allowed, &value))
            .or_else(|| {
                GLOBAL_ATTRS
                    .contains(&name.as_str())
                    .then_some(())
                    .and_then(|()| property(name, Allowed::Any, &value))
            });
        if let Some(safe) = safe {
            result.push((name.clone(), value_string(&safe)));
        }
    }
    if tag == "input" {
        for (name, value) in [("disabled", ""), ("type", "checkbox")] {
            if !result.iter().any(|(key, _)| key == name) {
                result.push((name.to_owned(), value.to_owned()));
            }
        }
    }
    result
}

fn sanitize_children(nodes: Vec<HNode>, stack: &mut Vec<String>) -> Vec<HNode> {
    let mut out = Vec::with_capacity(nodes.len());
    for node in nodes {
        match node {
            HNode::Text(text) => out.push(HNode::Text(text)),
            HNode::Element(element) => {
                let name = element.name;
                stack.push(name.clone());
                let children = sanitize_children(element.children, stack);
                stack.pop();
                let mut safe = TAG_NAMES.contains(&name.as_str());
                if safe && TABLE_PARTS.contains(&name.as_str()) {
                    safe = stack.iter().any(|open| open == "table");
                }
                if safe {
                    let attrs = clean_attrs(&name, &element.attrs);
                    out.push(HNode::Element(HElement {
                        name,
                        attrs,
                        children,
                        explicit_close: element.explicit_close,
                    }));
                } else if !STRIP.contains(&name.as_str()) {
                    out.extend(children);
                }
            }
        }
    }
    out
}

/// Sanitizes a tree with the copied.md schema.
pub(crate) fn sanitize(nodes: Vec<HNode>) -> Vec<HNode> {
    sanitize_children(nodes, &mut Vec::new())
}

fn image_label(element: &HElement) -> String {
    match element.attr("alt") {
        Some(alt) if !alt.is_empty() => alt.to_owned(),
        _ => "Image".to_owned(),
    }
}

fn replace_images_with_labels(nodes: &mut [HNode]) {
    for node in nodes {
        if let HNode::Element(element) = node {
            if element.name == "img" {
                *node = HNode::Text(image_label(element));
            } else {
                replace_images_with_labels(&mut element.children);
            }
        }
    }
}

/// Pre-sanitization pass: images become links or labels and every `href` is
/// re-validated against the URL policy.
pub(crate) fn prepare_links(nodes: &mut [HNode], source_url: Option<&str>) {
    for node in nodes.iter_mut() {
        let HNode::Element(element) = node else {
            continue;
        };
        if element.name == "a" {
            replace_images_with_labels(&mut element.children);
        }
        if element.name == "img" {
            let alt = image_label(element);
            let href = safe_url(element.attr("src").unwrap_or(""), source_url);
            *node = match href {
                Some(href) => HNode::Element(HElement::new(
                    "a",
                    vec![("href".into(), href)],
                    vec![HNode::Text(alt)],
                )),
                None => HNode::Text(alt),
            };
            continue;
        }
        if let Some(href) = element.attr("href") {
            match safe_url(href, source_url) {
                Some(href) => element.set_attr("href", href),
                None => element.remove_attr("href"),
            }
        }
        prepare_links(&mut element.children, source_url);
    }
}

fn collect_ids(nodes: &[HNode], ids: &mut std::collections::HashSet<String>) {
    for node in nodes {
        if let HNode::Element(element) = node {
            if let Some(id) = element.attr("id") {
                ids.insert(id.to_owned());
            }
            collect_ids(&element.children, ids);
        }
    }
}

fn repair_fragments(nodes: &mut [HNode], ids: &std::collections::HashSet<String>) {
    for node in nodes {
        if let HNode::Element(element) = node {
            if let Some(fragment) = element.attr("href").and_then(|h| h.strip_prefix('#')) {
                let prefixed = format!("{CLOBBER_PREFIX}{fragment}");
                if !ids.contains(fragment) && ids.contains(&prefixed) {
                    element.set_attr("href", format!("#{prefixed}"));
                }
            }
            repair_fragments(&mut element.children, ids);
        }
    }
}

/// Points `#fragment` links at clobber-prefixed ids when only those exist.
pub(crate) fn repair_clobbered_fragments(nodes: &mut [HNode]) {
    let mut ids = std::collections::HashSet::new();
    collect_ids(nodes, &mut ids);
    repair_fragments(nodes, &ids);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::html::dom::parse_fragment;
    use crate::html::serialize::serialize;

    fn clean(source: &str) -> String {
        let mut nodes = parse_fragment(source).children;
        prepare_links(&mut nodes, None);
        let mut nodes = sanitize(nodes);
        repair_clobbered_fragments(&mut nodes);
        serialize(&nodes)
    }

    #[test]
    fn strips_dangerous_markup() {
        assert_eq!(
            clean(
                "<script>x</script><style>y</style><div onclick=\"bad()\"><b>Clean</b> <a href=\"vbscript:x\">U</a></div>"
            ),
            "<div><b>Clean</b> <a>U</a></div>"
        );
        assert_eq!(
            clean("<form><input type=text value=1></form>"),
            "<input value=\"1\" disabled type=\"checkbox\">"
        );
        assert_eq!(
            clean("<table><tr><td start=03>y</td></tr></table>"),
            "<table><tbody><tr><td start=\"3\">y</td></tr></tbody></table>"
        );
        assert_eq!(clean("<tr><td>x</td></tr>"), "x");
    }

    #[test]
    fn filters_classes_and_clobbers_ids() {
        assert_eq!(
            clean("<code class=\"language-rs x\">a</code>"),
            "<code class=\"language-rs\">a</code>"
        );
        assert_eq!(
            clean("<p class=x id=a aria-describedby=\"b c\">t</p>"),
            "<p id=\"user-content-a\">t</p>"
        );
        assert_eq!(
            clean("<a href=\"#a\">x</a><b id=\"a\">y</b>"),
            "<a href=\"#user-content-a\">x</a><b id=\"user-content-a\">y</b>"
        );
        assert_eq!(
            clean("<ul aria-describedby=\"b c\" class=y></ul>"),
            "<ul aria-describedby=\"user-content-b user-content-c\" class=\"\"></ul>"
        );
    }

    #[test]
    fn converts_images() {
        assert_eq!(
            clean("<img src=\"https://x.test/i.png\" alt=\"I\">"),
            "<a href=\"https://x.test/i.png\">I</a>"
        );
        assert_eq!(
            clean("<a href=\"https://x.test\"><img src=\"./a.png\"></a>"),
            "<a href=\"https://x.test\">Image</a>"
        );
        assert_eq!(clean("<img src=\"javascript:x\" alt=\"\">"), "Image");
    }

    #[test]
    fn numbers_format_like_javascript() {
        assert_eq!(js_number_string(3.0), "3");
        assert_eq!(js_number_string(1.5), "1.5");
        assert_eq!(js_number(" 0x10 "), Some(16.0));
        assert_eq!(js_number("1e2"), Some(100.0));
        assert_eq!(js_number("10%"), None);
        assert_eq!(js_number("-Infinity"), Some(f64::NEG_INFINITY));
        assert_eq!(js_number_string(f64::INFINITY), "Infinity");
        assert_eq!(js_number_string(f64::NEG_INFINITY), "-Infinity");
        assert_eq!(js_number_string(0.0), "0");
        assert_eq!(js_number("+1"), Some(1.0));
        assert_eq!(js_number("0o7"), Some(7.0));
        assert_eq!(js_number("0b11"), Some(3.0));
        assert_eq!(js_number(" "), Some(0.0));
        assert_eq!(js_number("-"), None);
    }

    #[test]
    fn normalizes_list_and_protocol_attributes() {
        assert_eq!(
            clean("<input accept=\"a, b,\">"),
            "<input accept=\"a, b\" disabled type=\"checkbox\">"
        );
        assert_eq!(
            clean("<input accept=\"a,,\" disabled=false type=CHECKBOX>"),
            "<input accept=\"a, ,\" disabled type=\"checkbox\">"
        );
        assert_eq!(
            clean("<span coords=\"1, 2.50,x\">c</span>"),
            "<span coords=\"1, 2.5, x\">c</span>"
        );
        assert_eq!(
            clean("<q cite=\"/x:y\">a</q><q cite=\"javascript:x\">b</q>"),
            "<q cite=\"/x:y\">a</q><q>b</q>"
        );
        assert_eq!(
            clean("<input checked=yes disabled=\"\">"),
            "<input checked disabled type=\"checkbox\">"
        );
        assert_eq!(clean("<td width=\"10%\" colspan=2>x</td>"), "x");
        assert_eq!(
            clean("<table><tr><td width=\"10%\" rowspan=\" 2 \">x</td></tr></table>"),
            "<table><tbody><tr><td width=\"10%\" rowspan=\"2\">x</td></tr></tbody></table>"
        );
    }
}
