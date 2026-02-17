import type { Node, PhrasingContent, BlockContent } from 'mdast';
import { BaseConverter } from './base-converter';
import { Platform } from './types';
import type { ConversionResult } from './types';

export class RedditConverter extends BaseConverter {
  readonly platform = Platform.Reddit;
  readonly label = 'Reddit';

  convert(markdown: string): ConversionResult {
    if (!markdown.trim()) {
      return this.buildResult({ 'text/plain': '' }, markdown.length);
    }

    const tree = this.parseMarkdown(markdown);
    const lines: string[] = [];

    for (const node of tree.children) {
      lines.push(this.processBlock(node as BlockContent));
    }

    const text = lines.join('\n\n');
    return this.buildResult({ 'text/plain': text.trim() }, markdown.length);
  }

  private processBlock(node: BlockContent | Node): string {
    switch (node.type) {
      case 'heading': {
        const level = 'depth' in node ? (node.depth as number) : 1;
        const prefix = '#'.repeat(level);
        const content = this.processInline(
          'children' in node ? (node.children as PhrasingContent[]) : [],
        );
        return `${prefix} ${content}`;
      }
      case 'paragraph':
        return this.processInline(
          'children' in node ? (node.children as PhrasingContent[]) : [],
        );
      case 'blockquote': {
        const children = 'children' in node ? (node.children as BlockContent[]) : [];
        return children.map((child) => '> ' + this.processBlock(child)).join('\n');
      }
      case 'code': {
        const lang = 'lang' in node ? (node.lang as string | null) : null;
        const value = 'value' in node ? (node.value as string) : '';
        return '```' + (lang || '') + '\n' + value + '\n```';
      }
      case 'list': {
        const items = 'children' in node ? (node.children as Node[]) : [];
        const ordered = 'ordered' in node ? (node.ordered as boolean) : false;
        return items
          .map((item, i) => {
            const prefix = ordered ? `${i + 1}. ` : '- ';
            const content = 'children' in item ? (item.children as BlockContent[]) : [];
            return prefix + content.map((c) => this.processBlock(c)).join('\n');
          })
          .join('\n');
      }
      case 'table': {
        const rows = 'children' in node ? (node.children as Node[]) : [];
        if (rows.length === 0) return '';

        const headerRow = rows[0];
        const headerCells = 'children' in headerRow ? (headerRow.children as Node[]) : [];
        const header = headerCells
          .map((cell) =>
            this.processInline('children' in cell ? (cell.children as PhrasingContent[]) : []),
          )
          .join(' | ');
        const separator = headerCells.map(() => '---').join(' | ');
        const bodyRows = rows.slice(1).map((row) => {
          const cells = 'children' in row ? (row.children as Node[]) : [];
          return cells
            .map((cell) =>
              this.processInline('children' in cell ? (cell.children as PhrasingContent[]) : []),
            )
            .join(' | ');
        });

        return ['| ' + header + ' |', '| ' + separator + ' |', ...bodyRows.map((r) => '| ' + r + ' |')].join('\n');
      }
      case 'thematicBreak':
        return '---';
      default:
        return 'value' in node ? (node.value as string) : '';
    }
  }

  private processInline(nodes: PhrasingContent[]): string {
    return nodes
      .map((node) => {
        switch (node.type) {
          case 'text':
            return node.value;
          case 'strong':
            return '**' + this.processInline(node.children) + '**';
          case 'emphasis':
            return '*' + this.processInline(node.children) + '*';
          case 'delete':
            return '~~' + this.processInline(node.children) + '~~';
          case 'inlineCode':
            return '`' + node.value + '`';
          case 'link':
            return '[' + this.processInline(node.children) + '](' + node.url + ')';
          case 'image':
            return '![' + (node.alt || '') + '](' + node.url + ')';
          case 'break':
            return '  \n';
          default:
            return 'value' in node ? (node.value as string) : '';
        }
      })
      .join('');
  }
}
