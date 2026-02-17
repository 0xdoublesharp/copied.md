import type { Node } from 'mdast';
import { toString } from 'mdast-util-to-string';
import { BaseConverter } from './base-converter';
import { Platform } from './types';
import type { ConversionResult } from './types';

export class SmsConverter extends BaseConverter {
  readonly platform = Platform.SMS;
  readonly label = 'SMS';

  convert(markdown: string): ConversionResult {
    if (!markdown.trim()) {
      return this.buildResult({ 'text/plain': '' }, markdown.length);
    }

    const tree = this.parseMarkdown(markdown);
    const lines: string[] = [];

    for (const node of tree.children) {
      lines.push(this.processBlock(node));
    }

    const text = lines.filter((l) => l !== null).join('\n\n');
    return this.buildResult({ 'text/plain': text.trim() }, markdown.length);
  }

  private processBlock(node: Node): string {
    switch (node.type) {
      case 'heading':
        return toString(node).toUpperCase();
      case 'paragraph':
        return toString(node);
      case 'blockquote':
        return (
          '| ' +
          ('children' in node ? (node.children as Node[]) : [])
            .map((child) => toString(child))
            .join('\n| ')
        );
      case 'code':
        return toString(node);
      case 'list': {
        const items = 'children' in node ? (node.children as Node[]) : [];
        const ordered = 'ordered' in node ? node.ordered : false;
        return items
          .map((item, i) => {
            const prefix = ordered ? `${i + 1}. ` : '- ';
            return prefix + toString(item);
          })
          .join('\n');
      }
      case 'thematicBreak':
        return '---';
      case 'table': {
        const rows = 'children' in node ? (node.children as Node[]) : [];
        return rows
          .map((row) => {
            const cells = 'children' in row ? (row.children as Node[]) : [];
            return cells.map((cell) => toString(cell)).join(' | ');
          })
          .join('\n');
      }
      default:
        return toString(node);
    }
  }
}
