import { fromHtml } from 'hast-util-from-html';
import type { Element, Parent as HtmlParent } from 'hast';
import type { Definition, Parent, Root } from 'mdast';
import remarkGfm from 'remark-gfm';
import remarkParse from 'remark-parse';
import { unified } from 'unified';
import { SKIP, visit } from 'unist-util-visit';
import type { Diagnostic } from './types';

const parser = unified().use(remarkParse).use(remarkGfm);
const allowedProtocols = new Set(['http:', 'https:', 'mailto:']);

export function safeUrl(value: string, sourceUrl?: string): string | undefined {
  if (value.startsWith('#')) return value;
  try {
    const url = new URL(value, sourceUrl);
    if (!allowedProtocols.has(url.protocol)) return undefined;
    return /^[a-z][a-z\d+.-]*:/i.test(value) ? value : url.href;
  } catch {
    return undefined;
  }
}

export function readMarkdown(markdown: string, sourceUrl?: string) {
  const tree = parser.parse(markdown);
  const definitions = new Map<string, Definition>();
  const diagnostics = new Map<Diagnostic['code'], Diagnostic>();

  visit(tree, 'definition', (node) => {
    if (!definitions.has(node.identifier)) definitions.set(node.identifier, node);
  });

  // Remark supplies a definition for reference nodes and a string alt for image nodes.
  visit(
    tree,
    ['link', 'image', 'linkReference', 'imageReference'] as const,
    (node, index, parent) => {
      const definition = 'identifier' in node ? definitions.get(node.identifier)! : node;
      const destination = definition.url;
      const url = destination && safeUrl(destination, sourceUrl);

      if (!url && destination) {
        const hasProtocol = /^[a-z][a-z\d+.-]*:/i.test(destination.trim());
        const code = hasProtocol ? 'unsupported-url' : 'relative-url';
        diagnostics.set(code, {
          code,
          severity: 'warning',
          message: hasProtocol
            ? 'An unsupported link destination was omitted.'
            : 'A relative link needs an original page URL.',
        });
      }
      if (url && 'url' in node) {
        node.url = url;
        return;
      }
      const replacement =
        'children' in node
          ? url
            ? [{ type: 'link' as const, url, title: definition.title, children: node.children }]
            : node.children
          : url
            ? [{ type: 'image' as const, url, title: definition.title, alt: node.alt }]
            : [{ type: 'text' as const, value: node.alt! }];
      (parent as Parent).children.splice(index!, 1, ...replacement);
      if (!url) return [SKIP, index!];
      return index! + 1;
    },
  );

  visit(tree, 'definition', (_node, index, parent) => {
    (parent as Parent).children.splice(index!, 1);
    return [SKIP, index!];
  });
  return { tree, diagnostics: [...diagnostics.values()] };
}

interface HtmlTextOptions {
  escapeText?: (value: string) => string;
  renderSpan?: (element: Element, content: string) => string;
}

export function htmlText(
  source: string,
  { escapeText = (value) => value, renderSpan }: HtmlTextOptions = {},
): string {
  const ignored = new Set(['script', 'style', 'template', 'iframe', 'object']);
  const blocks = new Set(['p', 'div', 'section', 'article', 'li', 'tr', 'br']);
  function text(parent: HtmlParent): string {
    return parent.children
      .map((node) => {
        if (node.type === 'text') return escapeText(node.value);
        if (node.type !== 'element' || ignored.has(node.tagName)) return '';
        const value =
          node.tagName === 'img' ? escapeText(String(node.properties.alt ?? '')) : text(node);
        if (node.tagName === 'span' && renderSpan) return renderSpan(node, value);
        return blocks.has(node.tagName) ? `${value}\n` : value;
      })
      .join('');
  }
  return text(fromHtml(source, { fragment: true }));
}

export function footnoteNumbers(tree: Root): Map<string, number> {
  const numbers = new Map<string, number>();
  visit(tree, 'footnoteReference', (node) => {
    if (!numbers.has(node.identifier)) numbers.set(node.identifier, numbers.size + 1);
  });
  return numbers;
}
