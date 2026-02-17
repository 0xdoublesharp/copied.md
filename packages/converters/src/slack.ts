import type { Node, PhrasingContent, BlockContent } from 'mdast';
import { BaseConverter } from './base-converter';
import { Platform } from './types';
import type { ConversionResult } from './types';

export class SlackConverter extends BaseConverter {
  readonly platform = Platform.Slack;
  readonly label = 'Slack';

  convert(markdown: string): ConversionResult {
    if (!markdown.trim()) {
      return this.buildResult({ 'text/plain': '' }, markdown.length);
    }

    const tree = this.parseMarkdown(markdown);
    const mrkdwnLines: string[] = [];
    const htmlLines: string[] = [];

    for (const node of tree.children) {
      mrkdwnLines.push(this.processBlock(node as BlockContent));
      htmlLines.push(this.processBlockHtml(node as BlockContent));
    }

    const mrkdwn = mrkdwnLines.join('\n\n');
    const html = htmlLines.join('');

    return this.buildResult(
      {
        'text/plain': mrkdwn.trim(),
        'text/html': html,
      },
      markdown.length,
    );
  }

  private processBlock(node: BlockContent | Node): string {
    switch (node.type) {
      case 'heading': {
        // Slack has no headings — use bold
        const content = this.processInline(
          'children' in node ? (node.children as PhrasingContent[]) : [],
        );
        return `*${content}*`;
      }
      case 'paragraph':
        return this.processInline('children' in node ? (node.children as PhrasingContent[]) : []);
      case 'blockquote': {
        const children = 'children' in node ? (node.children as BlockContent[]) : [];
        return children.map((child) => '> ' + this.processBlock(child)).join('\n');
      }
      case 'code': {
        const value = 'value' in node ? (node.value as string) : '';
        return '```\n' + value + '\n```';
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
        // Slack doesn't support tables — format as preformatted text
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
            return '*' + this.processInline(node.children) + '*';
          case 'emphasis':
            return '_' + this.processInline(node.children) + '_';
          case 'delete':
            return '~' + this.processInline(node.children) + '~';
          case 'inlineCode':
            return '`' + node.value + '`';
          case 'link':
            return '<' + node.url + '|' + this.processInline(node.children) + '>';
          case 'image':
            return '<' + node.url + '|' + (node.alt || 'image') + '>';
          case 'break':
            return '\n';
          default:
            return 'value' in node ? (node.value as string) : '';
        }
      })
      .join('');
  }

  // Generate HTML for rich clipboard (Slack accepts rich paste)
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
        return '<p>' + ('value' in node ? this.escapeHtml(node.value as string) : '') + '</p>';
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
