import type { PhrasingContent, Root, RootContent } from 'mdast';
import { visit } from 'unist-util-visit';
import { footnoteNumbers, htmlText } from './document';
import { toBold, toBoldItalic, toItalic } from './unicode';

type Style = { bold?: boolean; italic?: boolean };
interface TextOptions {
  slackApi?: boolean;
  decorativeUnicode?: boolean;
}

export function renderText(tree: Root, options: TextOptions = {}): string {
  const notes = footnoteNumbers(tree);
  const { slackApi, decorativeUnicode } = options;

  function escape(value: string): string {
    return slackApi
      ? value.replaceAll('&', '&amp;').replaceAll('<', '&lt;').replaceAll('>', '&gt;')
      : value;
  }

  function styled(value: string, style: Style): string {
    if (!decorativeUnicode) return value;
    if (style.bold && style.italic) return toBoldItalic(value);
    if (style.bold) return toBold(value);
    return style.italic ? toItalic(value) : value;
  }

  function link(label: string, url: string): string {
    if (!label || label === url) return escape(url);
    if (slackApi && !url.startsWith('#'))
      return '<' + escape(url.replaceAll('|', '%7C')) + '|' + label + '>';
    return url.startsWith('#') ? label : label + ' (' + url + ')';
  }

  function inline(nodes: PhrasingContent[], style: Style = {}): string {
    return nodes
      .map((node) => {
        switch (node.type) {
          case 'text':
            return escape(styled(node.value, style));
          case 'strong': {
            const value = inline(node.children, { ...style, bold: true });
            return slackApi ? '*' + value + '*' : value;
          }
          case 'emphasis': {
            const value = inline(node.children, { ...style, italic: true });
            return slackApi ? '_' + value + '_' : value;
          }
          case 'delete': {
            const value = inline(node.children, style);
            return slackApi ? '~' + value + '~' : value;
          }
          case 'inlineCode':
            return slackApi && !node.value.includes('`')
              ? '`' + escape(node.value) + '`'
              : escape(node.value);
          case 'link':
            return link(inline(node.children, style), node.url);
          case 'image':
            return link(escape(node.alt || 'Image'), node.url);
          case 'footnoteReference':
            return '[' + notes.get(node.identifier) + ']';
          case 'html':
            return escape(htmlText(node.value));
          case 'break':
            return '\n';
          default:
            return '';
        }
      })
      .join('');
  }

  function blocks(nodes: RootContent[], separator = '\n\n'): string {
    return nodes
      .map(block)
      .filter((value) => value !== '')
      .join(separator);
  }

  function block(node: RootContent): string {
    switch (node.type) {
      case 'paragraph':
        return inline(node.children);
      case 'heading': {
        const value = inline(node.children, { bold: true });
        return slackApi ? '*' + value + '*' : value;
      }
      case 'blockquote':
        return blocks(node.children)
          .split('\n')
          .map((line) => (line ? '> ' + line : '>'))
          .join('\n');
      case 'code':
        return slackApi && !node.value.includes('```')
          ? '```\n' + escape(node.value) + '\n```'
          : escape(node.value);
      case 'list':
        return node.children
          .map((item, index) => {
            const marker = node.ordered ? (node.start ?? 1) + index + '. ' : '• ';
            const task = item.checked == null ? '' : item.checked ? '[x] ' : '[ ] ';
            const lines = blocks(item.children, item.spread ? '\n\n' : '\n').split('\n');
            return (
              marker +
              task +
              (lines.shift() ?? '') +
              lines.map((line) => '\n' + (line ? ' '.repeat(marker.length) + line : '')).join('')
            );
          })
          .join('\n');
      case 'table': {
        const rows = node.children
          .map((row) =>
            row.children.map((cell) => inline(cell.children).replaceAll('|', '\\|')).join(' | '),
          )
          .join('\n');
        return slackApi ? '```\n' + rows + '\n```' : rows;
      }
      case 'thematicBreak':
        return '———';
      case 'html':
        return escape(htmlText(node.value)).trimEnd();
      default:
        return '';
    }
  }

  const definitions = new Map<number, string>();
  visit(tree, 'footnoteDefinition', (node) => {
    const number = notes.get(node.identifier);
    if (number !== undefined && !definitions.has(number)) {
      definitions.set(number, '[' + number + '] ' + blocks(node.children));
    }
  });
  return [
    blocks(tree.children),
    ...[...definitions].sort(([a], [b]) => a - b).map(([, value]) => value),
  ]
    .filter(Boolean)
    .join('\n\n');
}
