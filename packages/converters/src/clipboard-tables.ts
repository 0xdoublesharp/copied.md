import type { Element } from 'hast';
import { htmlNodeText, safeUrl } from './document';

function elements(parent: Element): Element[] {
  return parent.children.filter((node) => node.type === 'element');
}

function cellText(cell: Element): string {
  return htmlNodeText(cell, {
    renderLink(node, label) {
      const url = safeUrl(String(node.properties.href ?? ''));
      if (!url || url.startsWith('#') || url === label) return label;
      return label ? label + ' (' + url + ')' : url;
    },
  })
    .replace(/\s+/g, ' ')
    .trim()
    .replaceAll('|', '\\|');
}

export function tableAsText(table: Element): string {
  const children = elements(table);
  const caption = children.find((node) => node.tagName === 'caption');
  const rows = children
    .filter((node) => ['thead', 'tbody', 'tfoot'].includes(node.tagName))
    .flatMap(elements)
    .filter((node) => node.tagName === 'tr')
    .map((row) =>
      elements(row)
        .filter((cell) => cell.tagName === 'td' || cell.tagName === 'th')
        .map(cellText)
        .join(' | '),
    );
  return (caption ? [cellText(caption), ...rows] : rows).join('\n');
}
