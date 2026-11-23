import type { Parent, Root } from 'mdast';
import remarkGfm from 'remark-gfm';
import remarkStringify from 'remark-stringify';
import { unified } from 'unified';
import { SKIP, visit } from 'unist-util-visit';
import { footnoteNumbers, htmlText } from './document';
import { renderText } from './render-text';

const serializer = unified().use(remarkGfm).use(remarkStringify, {
  bullet: '-',
  emphasis: '*',
  fences: true,
  listItemIndent: 'one',
});

export function renderMarkdown(tree: Root, target: 'reddit' | 'discord'): string {
  const numbers = footnoteNumbers(tree);
  visit(tree, (node, index, parent) => {
    if (!parent || index === undefined) return;
    const siblings = (parent as Parent).children;
    switch (node.type) {
      case 'html':
        siblings[index] = { type: 'text', value: htmlText(node.value) };
        return [SKIP, index + 1];
      case 'footnoteReference':
        siblings[index] = { type: 'text', value: '[' + numbers.get(node.identifier) + ']' };
        return [SKIP, index + 1];
      case 'footnoteDefinition': {
        const number = numbers.get(node.identifier);
        if (number === undefined) {
          siblings.splice(index, 1);
          return [SKIP, index];
        }
        const note = renderText({ type: 'root', children: node.children });
        siblings[index] = {
          type: 'paragraph',
          children: [{ type: 'text', value: '[' + number + '] ' + note }],
        };
        return [SKIP, index + 1];
      }
      case 'heading':
        if (target === 'discord' && node.depth > 3) node.depth = 3;
        break;
      case 'table':
        if (target === 'discord') {
          siblings[index] = { type: 'code', value: renderText({ type: 'root', children: [node] }) };
          return [SKIP, index + 1];
        }
        break;
      case 'image':
        if (target === 'discord') {
          siblings[index] = {
            type: 'link',
            url: node.url,
            children: [{ type: 'text', value: node.alt || 'Image' }],
          };
          return [SKIP, index + 1];
        }
    }
  });
  return serializer.stringify(tree).trimEnd();
}
