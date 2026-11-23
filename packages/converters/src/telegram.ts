import type { Node, PhrasingContent, BlockContent } from 'mdast';
import { BaseConverter } from './base-converter';
import { Platform } from './types';
import type { ConversionResult } from './types';
import { toBold, toItalic, toBoldItalic } from './unicode';

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
        'text/plain': mdv2,
        'text/html': html,
      },
      markdown.length,
    );
  }

  private processBlock(node: BlockContent | Node): string {
    switch (node.type) {
      case 'heading': {
        const depth = 'depth' in node ? (node.depth as number) : 1;
        const content = this.processInline(
          'children' in node ? (node.children as PhrasingContent[]) : [],
          'normal',
        );
        const marker = depth === 1 ? '🧭 ' : depth === 2 ? '✦ ' : '▸ ';
        return marker + toBold(content);
      }
      case 'paragraph':
        return this.processInline(
          'children' in node ? (node.children as PhrasingContent[]) : [],
          'normal',
        );
      case 'blockquote': {
        const children = 'children' in node ? (node.children as BlockContent[]) : [];
        return children
          .map((child) =>
            this.processBlock(child)
              .split('\n')
              .map((line) => `❝ ${line}`)
              .join('\n'),
          )
          .join('\n');
      }
      case 'code': {
        const lang = 'lang' in node ? (node.lang as string | null) : null;
        const value = 'value' in node ? (node.value as string) : '';
        const header = lang ? `┌─ code: ${lang}` : '┌─ code';
        return `${header}\n${value}\n└─`;
      }
      case 'list': {
        const items = 'children' in node ? (node.children as Node[]) : [];
        const ordered = 'ordered' in node ? (node.ordered as boolean) : false;
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
        return '────────────────';
      case 'table': {
        const rows = 'children' in node ? (node.children as Node[]) : [];
        const tableRows = rows.map((row) => {
          const cells = 'children' in row ? (row.children as Node[]) : [];
          return cells
            .map((cell) => {
              return this.processInline(
                'children' in cell ? (cell.children as PhrasingContent[]) : [],
                'normal',
              );
            })
            .join(' | ');
        });
        return '┌─ table\n' + tableRows.join('\n') + '\n└─';
      }
      default:
        return 'value' in node ? (node.value as string) : '';
    }
  }

  private processInline(
    nodes: PhrasingContent[],
    style: 'normal' | 'bold' | 'italic' | 'bolditalic',
  ): string {
    return nodes
      .map((node) => {
        switch (node.type) {
          case 'text':
            return this.applyStyle(node.value, style);
          case 'strong':
            return this.processInline(
              node.children,
              style === 'italic' ? 'bolditalic' : style === 'bolditalic' ? 'bolditalic' : 'bold',
            );
          case 'emphasis':
            return this.processInline(
              node.children,
              style === 'bold' ? 'bolditalic' : style === 'bolditalic' ? 'bolditalic' : 'italic',
            );
          case 'delete':
            return this.applyStrikethrough(this.processInline(node.children, style));
          case 'inlineCode':
            return `「${node.value}」`;
          case 'link': {
            const label = this.processInline(node.children, style).trim();
            if (!label || label === node.url) {
              return `🔗 ${node.url}`;
            }
            return `🔗 ${label} (${node.url})`;
          }
          case 'image':
            return `🖼 ${node.alt || 'image'} (${node.url})`;
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
    return ordered ? `${index + 1}. ` : '• ';
  }

  private applyStyle(text: string, style: 'normal' | 'bold' | 'italic' | 'bolditalic'): string {
    switch (style) {
      case 'bold':
        return toBold(text);
      case 'italic':
        return toItalic(text);
      case 'bolditalic':
        return toBoldItalic(text);
      default:
        return text;
    }
  }

  private applyStrikethrough(text: string): string {
    return text;
  }

  // HTML version for rich clipboard (Telegram desktop accepts HTML paste)
  private processBlockHtml(node: BlockContent | Node): string {
    switch (node.type) {
      case 'heading': {
        const content = this.processInlineHtml(
          'children' in node ? (node.children as PhrasingContent[]) : [],
        );
        return `<p><b>${content}</b></p>`;
      }
      case 'paragraph':
        return `<p>${this.processInlineHtml('children' in node ? (node.children as PhrasingContent[]) : [])}</p>`;
      case 'blockquote': {
        const children = 'children' in node ? (node.children as BlockContent[]) : [];
        return (
          '<blockquote>' + children.map((c) => this.processBlockHtml(c)).join('') + '</blockquote>'
        );
      }
      case 'code': {
        const value = 'value' in node ? (node.value as string) : '';
        return '<pre>' + this.escapeHtml(value) + '</pre>';
      }
      case 'list': {
        const items = 'children' in node ? (node.children as Node[]) : [];
        const ordered = 'ordered' in node ? (node.ordered as boolean) : false;
        const tag = ordered ? 'ol' : 'ul';
        const itemsHtml = items
          .map((item) => {
            const content = 'children' in item ? (item.children as BlockContent[]) : [];
            return '<li>' + content.map((c) => this.processBlockHtml(c)).join('') + '</li>';
          })
          .join('');
        return `<${tag}>${itemsHtml}</${tag}>`;
      }
      case 'thematicBreak':
        return '<hr>';
      case 'html':
        return this.escapeHtml(this.stripHtmlTags('value' in node ? (node.value as string) : ''));
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
          case 'html':
            return this.escapeHtml(
              this.stripHtmlTags('value' in node ? (node.value as string) : ''),
            );
          case 'break':
            return '<br>';
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
      .replace(/>/g, '&gt;')
      .replace(/"/g, '&quot;');
  }
}
