import type { Node, PhrasingContent, BlockContent } from 'mdast';
import { BaseConverter } from './base-converter';
import { Platform } from './types';
import type { ConversionResult } from './types';
import { toBold, toItalic, toBoldItalic } from './unicode';

export class TwitterConverter extends BaseConverter {
  readonly platform = Platform.Twitter;
  readonly label = 'Twitter/X';
  readonly maxLength = 280;

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
        return '';
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
            return this.processInline(
              node.children,
              style === 'italic' ? 'bolditalic' : 'bold',
            );
          case 'emphasis':
            return this.processInline(
              node.children,
              style === 'bold' ? 'bolditalic' : 'italic',
            );
          case 'delete':
            return this.processInline(node.children, style);
          case 'inlineCode':
            return node.value;
          case 'link':
            return node.url;
          case 'image':
            return node.url;
          case 'break':
            return '\n';
          default:
            return 'value' in node ? (node.value as string) : '';
        }
      })
      .join('');
  }
}
