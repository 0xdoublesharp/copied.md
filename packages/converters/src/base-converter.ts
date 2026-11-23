import { SKIP, visit } from 'unist-util-visit';
import type { Root, RootContent } from 'mdast';
import { footnoteNumbers, htmlText, readMarkdown } from './document';
import { sanitizeHtmlFragment } from './rich-html';
import type { ConversionResult, Platform, PlatformConverter, ClipboardPayload } from './types';

export abstract class BaseConverter implements PlatformConverter {
  abstract readonly platform: Platform;
  abstract readonly label: string;
  readonly maxLength?: number;

  protected parseMarkdown(md: string): Root {
    const { tree } = readMarkdown(md);
    const numbers = footnoteNumbers(tree);
    visit(tree, 'footnoteReference', (node, index, parent) => {
      if (index !== undefined && parent) {
        parent.children[index] = { type: 'text', value: '[' + numbers.get(node.identifier) + ']' };
        return [SKIP, index + 1];
      }
    });
    tree.children = tree.children.flatMap<RootContent>((node) => {
      if (node.type !== 'footnoteDefinition') return [node];
      const number = numbers.get(node.identifier);
      if (!number) return [];
      return [
        { type: 'paragraph', children: [{ type: 'text', value: '[' + number + ']' }] },
        ...node.children,
      ];
    });
    return tree;
  }

  abstract convert(markdown: string): ConversionResult;

  /**
   * Strip HTML tags from a raw HTML string, keeping only text content.
   * Used to handle `html` type AST nodes (raw HTML embedded in markdown).
   */
  protected stripHtmlTags(html: string): string {
    return htmlText(html).trimEnd();
  }

  private normalizePlainTextOutput(text: string): string {
    return text.replace(/\r\n?/g, '\n');
  }

  private normalizeHtmlOutput(html: string): string {
    return html.replace(/\r\n?/g, '\n');
  }

  protected buildResult(payload: ClipboardPayload, originalLength: number): ConversionResult {
    const normalizedPlainText = this.normalizePlainTextOutput(payload['text/plain']);
    payload['text/plain'] = normalizedPlainText;

    if (payload['text/html'] != null) {
      payload['text/html'] = this.normalizeHtmlOutput(sanitizeHtmlFragment(payload['text/html']));
    }

    const plainText = payload['text/plain'];
    const plainCodePoints = Array.from(plainText);
    const truncated = this.maxLength != null && plainCodePoints.length > this.maxLength;
    if (truncated) {
      payload['text/plain'] = plainCodePoints.slice(0, this.maxLength!).join('');
    }
    return {
      payload,
      truncated,
      originalLength,
    };
  }
}
