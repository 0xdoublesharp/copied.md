import type { Node, BlockContent, PhrasingContent } from 'mdast';
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
      lines.push(this.processBlock(node as BlockContent));
    }

    const text = lines.filter((l) => l !== null).join('\n\n');
    return this.buildResult({ 'text/plain': text }, markdown.length);
  }

  private processBlock(node: BlockContent | Node): string {
    switch (node.type) {
      case 'heading': {
        const depth = 'depth' in node ? (node.depth as number) : 1;
        const content = this.processInline(
          'children' in node ? (node.children as PhrasingContent[]) : [],
        );
        const marker = depth === 1 ? '◆ ' : depth === 2 ? '▸ ' : '• ';
        return marker + content.toUpperCase();
      }
      case 'paragraph':
        return this.processInline('children' in node ? (node.children as PhrasingContent[]) : []);
      case 'blockquote':
        return (
          '| ' +
          ('children' in node ? (node.children as Node[]) : [])
            .map((child) => this.processBlock(child as BlockContent))
            .join('\n| ')
        );
      case 'code':
        return 'value' in node ? (node.value as string) : '';
      case 'list': {
        const items = 'children' in node ? (node.children as Node[]) : [];
        const ordered = 'ordered' in node ? Boolean(node.ordered) : false;
        return items
          .map((item, i) => {
            const prefix = this.getListPrefix(item, ordered, i);
            const content = 'children' in item ? (item.children as BlockContent[]) : [];
            return prefix + content.map((c) => this.processBlock(c)).join('\n');
          })
          .join('\n');
      }
      case 'html':
        return this.stripHtmlTags('value' in node ? (node.value as string) : '');
      case 'thematicBreak':
        return '---';
      case 'table': {
        const rows = 'children' in node ? (node.children as Node[]) : [];
        return rows
          .map((row) => {
            const cells = 'children' in row ? (row.children as Node[]) : [];
            return cells
              .map((cell) =>
                this.processInline('children' in cell ? (cell.children as PhrasingContent[]) : []),
              )
              .join(' | ');
          })
          .join('\n');
      }
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
          case 'emphasis':
          case 'delete':
            return this.processInline(node.children);
          case 'inlineCode':
            return node.value;
          case 'link': {
            const text = this.processInline(node.children);
            return text && text !== node.url ? `${text} (${node.url})` : node.url;
          }
          case 'image':
            return node.alt ? `${node.alt} (${node.url})` : node.url;
          case 'html':
            return this.stripHtmlTags('value' in node ? (node.value as string) : '');
          case 'break':
            return '\n';
          default:
            return 'value' in node ? (node.value as string) : '';
        }
      })
      .join('');
  }

  private getListPrefix(item: Node, ordered: boolean, index: number): string {
    if (!ordered && 'checked' in item && typeof item.checked === 'boolean') {
      return item.checked ? '☑ ' : '☐ ';
    }
    return ordered ? `${index + 1}. ` : '▸ ';
  }
}
