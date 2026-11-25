//! Table text for clipboard destinations (`clipboard-tables.ts`) and the aligned
//! monospace layout used by the greenfield table improvements.

use super::dom::HElement;
use super::text::{TextHooks, node_text};
use crate::url::safe_url;

fn cell_text(cell: &HElement, escape_pipes: bool) -> String {
    let link = |element: &HElement, label: &str| {
        let url = safe_url(element.attr("href").unwrap_or(""), None);
        match url {
            Some(url) if !url.starts_with('#') && url != label => {
                if label.is_empty() {
                    url
                } else {
                    format!("{label} ({url})")
                }
            }
            _ => label.to_owned(),
        }
    };
    let text = node_text(
        &cell.children,
        TextHooks {
            link: Some(&link),
            ..TextHooks::default()
        },
    );
    // `.replace(/\s+/g, ' ').trim()` with JavaScript whitespace.
    let collapsed = text
        .split(crate::url::is_js_whitespace)
        .filter(|s| !s.is_empty())
        .collect::<Vec<_>>()
        .join(" ");
    if escape_pipes {
        collapsed.replace('|', "\\|")
    } else {
        collapsed
    }
}

/// Caption and rows of cell text from an HTML table element.
pub(crate) fn table_cells(
    table: &HElement,
    escape_pipes: bool,
) -> (Option<String>, Vec<Vec<String>>) {
    let caption = table
        .elements()
        .find(|e| e.name == "caption")
        .map(|c| cell_text(c, escape_pipes));
    let rows = table
        .elements()
        .filter(|e| matches!(e.name.as_str(), "thead" | "tbody" | "tfoot"))
        .flat_map(HElement::elements)
        .filter(|e| e.name == "tr")
        .map(|row| {
            row.elements()
                .filter(|cell| cell.name == "td" || cell.name == "th")
                .map(|cell| cell_text(cell, escape_pipes))
                .collect()
        })
        .collect();
    (caption, rows)
}

/// `tableAsText`: caption, then rows of cells joined with ` | `.
pub(crate) fn table_as_text(table: &HElement) -> String {
    let (caption, rows) = table_cells(table, true);
    caption
        .into_iter()
        .chain(rows.iter().map(|row| row.join(" | ")))
        .collect::<Vec<_>>()
        .join("\n")
}

/// Monospace display width (East Asian wide characters count twice, combining
/// marks and joiners count zero).
pub(crate) fn display_width(value: &str) -> usize {
    value
        .chars()
        .map(|ch| match u32::from(ch) {
            0x0300..=0x036F
            | 0x1AB0..=0x1AFF
            | 0x1DC0..=0x1DFF
            | 0x200B..=0x200F
            | 0x20D0..=0x20FF
            | 0xFE00..=0xFE0F
            | 0xFE20..=0xFE2F
            | 0x1F3FB..=0x1F3FF
            | 0xE0000..=0xE01EF => 0,
            0x1100..=0x115F
            | 0x2E80..=0x303E
            | 0x3041..=0x33FF
            | 0x3400..=0x4DBF
            | 0x4E00..=0x9FFF
            | 0xA000..=0xA4CF
            | 0xAC00..=0xD7A3
            | 0xF900..=0xFAFF
            | 0xFE30..=0xFE4F
            | 0xFF00..=0xFF60
            | 0xFFE0..=0xFFE6
            | 0x1F300..=0x1F64F
            | 0x1F900..=0x1F9FF
            | 0x20000..=0x3FFFD => 2,
            _ => 1,
        })
        .sum()
}

/// Lays out rows as aligned monospace columns.
///
/// Every column but the last is padded to its widest cell so no line has trailing
/// spaces. When `header` is set, a dashed rule separates the first row.
pub(crate) fn align_rows(rows: &[Vec<String>], header: bool) -> Vec<String> {
    let columns = rows.iter().map(Vec::len).max().unwrap_or(0);
    let mut widths = vec![0; columns];
    for row in rows {
        for (index, cell) in row.iter().enumerate() {
            widths[index] = widths[index].max(display_width(cell));
        }
    }
    let line = |cells: &[String]| {
        let last = cells.len().saturating_sub(1);
        cells
            .iter()
            .enumerate()
            .map(|(index, cell)| {
                if index == last {
                    cell.clone()
                } else {
                    format!("{cell}{}", " ".repeat(widths[index] - display_width(cell)))
                }
            })
            .collect::<Vec<_>>()
            .join(" | ")
    };
    let mut lines = Vec::with_capacity(rows.len() + 1);
    for (index, row) in rows.iter().enumerate() {
        lines.push(line(row));
        if index == 0 && header && rows.len() > 1 {
            let rule: Vec<String> = widths.iter().map(|&w| "-".repeat(w.max(1))).collect();
            lines.push(rule.join(" | "));
        }
    }
    lines
}

/// Aligned monospace text for an HTML table, keeping its caption first.
pub(crate) fn aligned_table_text(table: &HElement) -> String {
    let (caption, rows) = table_cells(table, false);
    let header = table.elements().any(|e| e.name == "thead")
        || table
            .elements()
            .filter(|e| matches!(e.name.as_str(), "thead" | "tbody" | "tfoot"))
            .flat_map(HElement::elements)
            .find(|e| e.name == "tr")
            .is_some_and(|row| row.elements().any(|cell| cell.name == "th"));
    caption
        .into_iter()
        .chain(align_rows(&rows, header))
        .collect::<Vec<_>>()
        .join("\n")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::html::dom::{HNode, parse_fragment};

    fn table(source: &str) -> HElement {
        let node = parse_fragment(source).children.remove(0);
        let HNode::Element(element) = node else {
            unreachable!("tables parse as elements")
        };
        element
    }

    #[test]
    fn extracts_caption_rows_and_links() {
        let t = table(
            "<table><caption>Details</caption><tbody><tr><td><a href=\"https://example.org/bare\"></a></td>\
             <td><a href=\"https://example.org/named\">https://example.org/named</a></td>\
             <td><a href=\"#internal\">A|B</a></td><td><a href=\"javascript:x\">J</a></td></tr></tbody></table>",
        );
        assert_eq!(
            table_as_text(&t),
            "Details\nhttps://example.org/bare | https://example.org/named | A\\|B | J"
        );
    }

    #[test]
    fn aligns_columns_without_trailing_spaces() {
        let rows = vec![
            vec!["Service".into(), "Status".into()],
            vec!["Web".into(), "漢字".into()],
        ];
        assert_eq!(
            align_rows(&rows, true),
            ["Service | Status", "------- | ------", "Web     | 漢字"]
        );
        let t = table("<table><tr><td>a</td><td>bb</td></tr><tr><td>ccc</td></tr></table>");
        assert_eq!(aligned_table_text(&t), "a   | bb\nccc");
        assert_eq!(display_width("e\u{301}👍🏽"), 3);
        assert_eq!(display_width("🤖𠀀ｱ"), 5);
    }
}
