import { BaseConverter } from './base-converter';
import { readMarkdown } from './document';
import { renderRichHtml } from './rich-html';
import { Platform, type ConversionResult } from './types';

export class HtmlConverter extends BaseConverter {
  readonly platform = Platform.HTML;
  readonly label = 'HTML';

  convert(markdown: string): ConversionResult {
    const html = renderRichHtml(readMarkdown(markdown).tree);
    return {
      payload: { 'text/plain': html, 'text/html': html },
      truncated: false,
      originalLength: markdown.length,
    };
  }
}
