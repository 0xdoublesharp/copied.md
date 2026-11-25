import { fromHtml } from 'hast-util-from-html';
import type { Root as HtmlRoot } from 'hast';
import type { Root } from 'mdast';
import rehypeRaw from 'rehype-raw';
import rehypeSanitize, { defaultSchema } from 'rehype-sanitize';
import rehypeStringify from 'rehype-stringify';
import remarkRehype from 'remark-rehype';
import { unified } from 'unified';
import { SKIP, visit } from 'unist-util-visit';
import { tableAsText } from './clipboard-tables';
import { safeUrl } from './document';

const schema = {
  ...defaultSchema,
  protocols: { ...defaultSchema.protocols, href: ['http', 'https', 'mailto'] },
  strip: [...defaultSchema.strip!, 'style', 'template', 'iframe', 'object'],
};

const toHtml = unified()
  .use(remarkRehype, { allowDangerousHtml: true, clobberPrefix: '' })
  .use(rehypeRaw);
const sanitizer = unified().use(rehypeSanitize, schema).use(rehypeStringify);

type Destination = 'slack-paste' | 'teams' | 'word';

// Add only fixed, app-specific styles after sanitization has stripped source styles.
const destinationStyles: Record<'teams' | 'word', Record<string, string>> = {
  teams: {
    pre: 'font-family: monospace; white-space: pre-wrap;',
    code: 'font-family: monospace;',
    table: 'border-collapse: collapse;',
    th: 'border: 1px solid #999; padding: 4px 8px; text-align: left;',
    td: 'border: 1px solid #999; padding: 4px 8px;',
  },
  word: {
    pre: 'font-family: Consolas, monospace; white-space: pre;',
    code: 'font-family: Consolas, monospace;',
    table: 'border-collapse: collapse; border: 1px solid #999;',
    th: 'border: 1px solid #999; background-color: #eaeaea; padding: 4pt 6pt; text-align: left;',
    td: 'border: 1px solid #999; padding: 4pt 6pt;',
  },
};

export function renderRichHtml(tree: Root, sourceUrl?: string, destination?: Destination): string {
  return renderSafeHtml(toHtml.runSync(tree), sourceUrl, destination);
}

export function sanitizeHtmlFragment(html: string): string {
  return renderSafeHtml(fromHtml(html, { fragment: true }));
}

function renderSafeHtml(tree: HtmlRoot, sourceUrl?: string, destination?: Destination): string {
  visit(tree, 'element', (node, index, parent) => {
    if (node.tagName === 'a') {
      visit(node, { type: 'element', tagName: 'img' }, (image, childIndex, owner) => {
        owner!.children[childIndex!] = { type: 'text', value: imageLabel(image.properties.alt) };
      });
    }
    if (node.tagName === 'img') {
      const alt = imageLabel(node.properties.alt);
      const href = safeUrl(String(node.properties.src ?? ''), sourceUrl);
      parent!.children[index!] = href
        ? {
            type: 'element',
            tagName: 'a',
            properties: { href },
            children: [{ type: 'text', value: alt }],
          }
        : { type: 'text', value: alt };
      return [SKIP, index! + 1];
    }
    if (typeof node.properties.href === 'string') {
      const href = safeUrl(node.properties.href, sourceUrl);
      if (href) node.properties.href = href;
      else delete node.properties.href;
    }
  });

  const clean = sanitizer.runSync(tree) as HtmlRoot;
  const ids = new Set<string>();
  visit(clean, 'element', (node) => {
    if (typeof node.properties.id === 'string') ids.add(node.properties.id);
  });
  visit(clean, 'element', (node) => {
    const href = node.properties.href;
    if (typeof href !== 'string' || !href.startsWith('#')) return;
    const fragment = href.slice(1);
    const prefixed = schema.clobberPrefix! + fragment;
    if (!ids.has(fragment) && ids.has(prefixed)) node.properties.href = '#' + prefixed;
  });
  if (destination === 'slack-paste') {
    visit(clean, { type: 'element', tagName: 'table' }, (table, index, parent) => {
      parent!.children[index!] = {
        type: 'element',
        tagName: 'pre',
        properties: {},
        children: [
          {
            type: 'element',
            tagName: 'code',
            properties: {},
            children: [{ type: 'text', value: tableAsText(table) }],
          },
        ],
      };
      return [SKIP, index! + 1];
    });
  } else if (destination) {
    const styles = destinationStyles[destination];
    visit(clean, 'element', (node) => {
      if (Object.hasOwn(styles, node.tagName)) node.properties.style = styles[node.tagName];
    });
  }
  return sanitizer.stringify(clean);
}

function imageLabel(alt: unknown): string {
  return String(alt || 'Image');
}
