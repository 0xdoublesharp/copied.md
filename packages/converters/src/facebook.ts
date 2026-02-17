import type { Node, PhrasingContent, BlockContent } from 'mdast';
import { BaseConverter } from './base-converter';
import { Platform } from './types';
import type { ConversionResult } from './types';
import { toBold, toItalic, toBoldItalic } from './unicode';

export class FacebookConverter extends BaseConverter {
  readonly platform = Platform.Facebook;
  readonly label = 'Facebook';

  convert(markdown: string): ConversionResult {
    if (!markdown.trim()) {
      return this.buildResult({ 'text/plain': '' }, markdown.length);
    }

    const tree = this.parseMarkdown(markdown);
    const plainLines: string[] = [];
    const htmlLines: string[] = [];

    for (const node of tree.children) {
      plainLines.push(this.processBlock(node as BlockContent));
      htmlLines.push(this.processBlockHtml(node as BlockContent));
    }

    const plain = plainLines.join('\n\n');
    const html = htmlLines.join('');

    return this.buildResult(
      {
        'text/plain': plain.trim(),
        'text/html': html,
      },
      markdown.length,
    );
  }

  // Plain text with Unicode styled characters
  private processBlock(node: BlockContent | Node): string {
    switch (node.type) {
      case 'heading': {
        const content = this.processInline(
          'children' in node ? (node.children as PhrasingContent[]) : [],
          'bold',
        );
        return content;
      }
      case 'paragraph':
        return this.processInline(
          'children' in node ? (node.children as PhrasingContent[]) : [],
          'normal',
        );
      case 'blockquote': {
        const children = 'children' in node ? (node.children as BlockContent[]) : [];
        return children.map((child) => '\u275D ' + this.processBlock(child)).join('\n');
      }
      case 'code': {
        const value = 'value' in node ? (node.value as string) : '';
        return value;
      }
      case 'list': {
        const items = 'children' in node ? (node.children as Node[]) : [];
        const ordered = 'ordered' in node ? (node.ordered as boolean) : false;
        return items
          .map((item, i) => {
            const prefix = ordered ? `${i + 1}. ` : '\u2022 ';
            const content = 'children' in item ? (item.children as BlockContent[]) : [];
            return prefix + content.map((c) => this.processBlock(c)).join('\n');
          })
          .join('\n');
      }
      case 'thematicBreak':
        return '\u2500'.repeat(20);
      case 'table': {
        const rows = 'children' in node ? (node.children as Node[]) : [];
        return rows
          .map((row) => {
            const cells = 'children' in row ? (row.children as Node[]) : [];
            return cells
              .map((cell) =>
                this.processInline(
                  'children' in cell ? (cell.children as PhrasingContent[]) : [],
                  'normal',
                ),
              )
              .join(' | ');
          })
          .join('\n');
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
          case 'text': {
            switch (style) {
              case 'bold':
                return toBold(node.value);
              case 'italic':
                return toItalic(node.value);
              case 'bolditalic':
                return toBoldItalic(node.value);
              default:
                return node.value;
            }
          }
          case 'strong':
            return this.processInline(node.children, style === 'italic' ? 'bolditalic' : 'bold');
          case 'emphasis':
            return this.processInline(node.children, style === 'bold' ? 'bolditalic' : 'italic');
          case 'delete':
            return this.processInline(node.children, style);
          case 'inlineCode':
            return node.value;
          case 'link':
            return this.processInline(node.children, style) + ' (' + node.url + ')';
          case 'image':
            return (node.alt || 'image') + ' (' + node.url + ')';
          case 'break':
            return '\n';
          default:
            return 'value' in node ? (node.value as string) : '';
        }
      })
      .join('');
  }

  // Rich HTML for clipboard (Facebook accepts rich paste)
  private processBlockHtml(node: BlockContent | Node): string {
    switch (node.type) {
      case 'heading': {
        const level = 'depth' in node ? (node.depth as number) : 1;
        const content = this.processInlineHtml(
          'children' in node ? (node.children as PhrasingContent[]) : [],
        );
        return `<h${level}>${content}</h${level}>`;
      }
      case 'paragraph':
        return (
          '<p>' +
          this.processInlineHtml('children' in node ? (node.children as PhrasingContent[]) : []) +
          '</p>'
        );
      case 'blockquote': {
        const children = 'children' in node ? (node.children as BlockContent[]) : [];
        return (
          '<blockquote>' + children.map((c) => this.processBlockHtml(c)).join('') + '</blockquote>'
        );
      }
      case 'code': {
        const value = 'value' in node ? (node.value as string) : '';
        return '<pre><code>' + this.escapeHtml(value) + '</code></pre>';
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
          case 'image':
            return (
              '<img src="' +
              this.escapeHtml(node.url) +
              '" alt="' +
              this.escapeHtml(node.alt || '') +
              '">'
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
