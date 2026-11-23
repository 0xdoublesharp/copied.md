import { fromHtml } from 'hast-util-from-html';
import type { Root as HtmlRoot } from 'hast';
import type { Root } from 'mdast';
import rehypeRaw from 'rehype-raw';
import rehypeSanitize, { defaultSchema } from 'rehype-sanitize';
import rehypeStringify from 'rehype-stringify';
import remarkRehype from 'remark-rehype';
import { unified } from 'unified';
import { SKIP, visit } from 'unist-util-visit';
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

export function renderRichHtml(tree: Root, sourceUrl?: string): string {
  return renderSafeHtml(toHtml.runSync(tree), sourceUrl);
}

export function sanitizeHtmlFragment(html: string): string {
  return renderSafeHtml(fromHtml(html, { fragment: true }));
}

function renderSafeHtml(tree: HtmlRoot, sourceUrl?: string): string {
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
  return sanitizer.stringify(clean);
}

function imageLabel(alt: unknown): string {
  return String(alt || 'Image');
}
