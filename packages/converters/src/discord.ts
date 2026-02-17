import type { Node, PhrasingContent, BlockContent } from 'mdast';
import { BaseConverter } from './base-converter';
import { Platform } from './types';
import type { ConversionResult } from './types';

export class DiscordConverter extends BaseConverter {
  readonly platform = Platform.Discord;
  readonly label = 'Discord';

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
        // Discord doesn't support headings — use bold as fallback
        const content = this.processInline(
          'children' in node ? (node.children as PhrasingContent[]) : [],
        );
        return `**${content}**`;
      }
      case 'paragraph':
        return this.processInline('children' in node ? (node.children as PhrasingContent[]) : []);
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
      case 'html':
        return this.stripHtmlTags('value' in node ? (node.value as string) : '');
      case 'thematicBreak':
        return '---';
      case 'table': {
        // Discord doesn't support tables — format as code block
        const rows = 'children' in node ? (node.children as Node[]) : [];
        const tableRows = rows.map((row) => {
          const cells = 'children' in row ? (row.children as Node[]) : [];
          return cells
            .map((cell) =>
              this.processInline('children' in cell ? (cell.children as PhrasingContent[]) : []),
            )
            .join(' | ');
        });
        return '```\n' + tableRows.join('\n') + '\n```';
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
            return '**' + this.processInline(node.children) + '**';
          case 'emphasis':
            return '*' + this.processInline(node.children) + '*';
          case 'delete':
            return '~~' + this.processInline(node.children) + '~~';
          case 'inlineCode':
            return '`' + node.value + '`';
          case 'link':
            // Discord renders links but shows URL, not anchor text
            return this.processInline(node.children) + ' (<' + node.url + '>)';
          case 'image':
            return node.url;
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
}
