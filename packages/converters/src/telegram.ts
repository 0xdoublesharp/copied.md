import type { Node, PhrasingContent, BlockContent } from 'mdast';
import { BaseConverter } from './base-converter';
import { Platform } from './types';
import type { ConversionResult } from './types';

// Characters that must be escaped in Telegram MarkdownV2 text content
const TELEGRAM_SPECIAL_CHARS = /([_*\[\]()~`>#+\-=|{}.!\\])/g;

export class TelegramConverter extends BaseConverter {
  readonly platform = Platform.Telegram;
  readonly label = 'Telegram';

  convert(markdown: string): ConversionResult {
    if (!markdown.trim()) {
      return this.buildResult({ 'text/plain': '' }, markdown.length);
    }

    const tree = this.parseMarkdown(markdown);
    const mdv2Lines: string[] = [];
    const htmlLines: string[] = [];

    for (const node of tree.children) {
      mdv2Lines.push(this.processBlock(node as BlockContent));
      htmlLines.push(this.processBlockHtml(node as BlockContent));
    }

    const mdv2 = mdv2Lines.join('\n\n');
    const html = htmlLines.join('');

    return this.buildResult(
      {
        'text/plain': mdv2.trim(),
        'text/html': html,
      },
      markdown.length,
    );
  }

  private escapeText(text: string): string {
    return text.replace(TELEGRAM_SPECIAL_CHARS, '\\$1');
  }

  private processBlock(node: BlockContent | Node): string {
    switch (node.type) {
      case 'heading': {
        const content = this.processInline(
          'children' in node ? (node.children as PhrasingContent[]) : [],
        );
        return `*${content}*`;
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
        // Code blocks are not escaped in Telegram MarkdownV2
        return '```' + (lang || '') + '\n' + value + '\n```';
      }
      case 'list': {
        const items = 'children' in node ? (node.children as Node[]) : [];
        const ordered = 'ordered' in node ? (node.ordered as boolean) : false;
        return items
          .map((item, i) => {
            const prefix = ordered
              ? this.escapeText(`${i + 1}.`) + ' '
              : this.escapeText('-') + ' ';
            const content = 'children' in item ? (item.children as BlockContent[]) : [];
            return prefix + content.map((c) => this.processBlock(c)).join('\n');
          })
          .join('\n');
      }
      case 'thematicBreak':
        return this.escapeText('---');
      case 'table': {
        // Telegram doesn't support tables — format as preformatted
        const rows = 'children' in node ? (node.children as Node[]) : [];
        const tableRows = rows.map((row) => {
          const cells = 'children' in row ? (row.children as Node[]) : [];
          return cells
            .map((cell) => {
              const content = 'children' in cell ? (cell.children as PhrasingContent[]) : [];
              return content
                .map((n) => ('value' in n ? (n.value as string) : ''))
                .join('');
            })
            .join(' | ');
        });
        return '```\n' + tableRows.join('\n') + '\n```';
      }
      default:
        return 'value' in node ? this.escapeText(node.value as string) : '';
    }
  }

  private processInline(nodes: PhrasingContent[]): string {
    return nodes
      .map((node) => {
        switch (node.type) {
          case 'text':
            return this.escapeText(node.value);
          case 'strong':
            return '*' + this.processInline(node.children) + '*';
          case 'emphasis':
            return '_' + this.processInline(node.children) + '_';
          case 'delete':
            return '~' + this.processInline(node.children) + '~';
          case 'inlineCode':
            return '`' + node.value + '`';
          case 'link': {
            const text = this.processInline(node.children);
            return '[' + text + '](' + this.escapeText(node.url) + ')';
          }
          case 'image':
            return '[' + this.escapeText(node.alt || 'image') + '](' + this.escapeText(node.url) + ')';
          case 'break':
            return '\n';
          default:
            return 'value' in node ? this.escapeText(node.value as string) : '';
        }
      })
      .join('');
  }

  // HTML version for rich clipboard (Telegram desktop accepts HTML paste)
  private processBlockHtml(node: BlockContent | Node): string {
    switch (node.type) {
      case 'heading': {
        const content = this.processInlineHtml(
          'children' in node ? (node.children as PhrasingContent[]) : [],
        );
        return `<b>${content}</b>\n`;
      }
      case 'paragraph':
        return (
          this.processInlineHtml(
            'children' in node ? (node.children as PhrasingContent[]) : [],
          ) + '\n'
        );
      case 'blockquote': {
        const children = 'children' in node ? (node.children as BlockContent[]) : [];
        return '<blockquote>' + children.map((c) => this.processBlockHtml(c)).join('') + '</blockquote>\n';
      }
      case 'code': {
        const value = 'value' in node ? (node.value as string) : '';
        return '<pre>' + this.escapeHtml(value) + '</pre>\n';
      }
      case 'list': {
        const items = 'children' in node ? (node.children as Node[]) : [];
        const ordered = 'ordered' in node ? (node.ordered as boolean) : false;
        return items
          .map((item, i) => {
            const prefix = ordered ? `${i + 1}. ` : '\u2022 ';
            const content = 'children' in item ? (item.children as BlockContent[]) : [];
            return prefix + content.map((c) => this.processBlockHtml(c)).join('');
          })
          .join('');
      }
      default:
        return 'value' in node ? this.escapeHtml(node.value as string) : '';
    }
  }

  private processInlineHtml(nodes: PhrasingContent[]): string {
    return nodes
      .map((node) => {
        switch (node.type) {
          case 'text':
            return this.escapeHtml(node.value);
          case 'strong':
            return '<b>' + this.processInlineHtml(node.children) + '</b>';
          case 'emphasis':
            return '<i>' + this.processInlineHtml(node.children) + '</i>';
          case 'delete':
            return '<s>' + this.processInlineHtml(node.children) + '</s>';
          case 'inlineCode':
            return '<code>' + this.escapeHtml(node.value) + '</code>';
          case 'link':
            return (
              '<a href="' +
              this.escapeHtml(node.url) +
              '">' +
              this.processInlineHtml(node.children) +
              '</a>'
            );
          case 'break':
            return '\n';
          default:
            return 'value' in node ? this.escapeHtml(node.value as string) : '';
        }
      })
      .join('');
  }

  private escapeHtml(text: string): string {
    return text
      .replace(/&/g, '&amp;')
      .replace(/</g, '&lt;')
      .replace(/>/g, '&gt;');
  }
}
