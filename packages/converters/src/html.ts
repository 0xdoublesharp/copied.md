import { unified } from 'unified';
import remarkParse from 'remark-parse';
import remarkGfm from 'remark-gfm';
import remarkRehype from 'remark-rehype';
import rehypeSanitize from 'rehype-sanitize';
import rehypeStringify from 'rehype-stringify';
import { BaseConverter } from './base-converter';
import { Platform } from './types';
import type { ConversionResult } from './types';

export class HtmlConverter extends BaseConverter {
  readonly platform = Platform.HTML;
  readonly label = 'HTML';

  convert(markdown: string): ConversionResult {
    if (!markdown.trim()) {
      return this.buildResult({ 'text/plain': '', 'text/html': '' }, markdown.length);
    }

    const result = unified()
      .use(remarkParse)
      .use(remarkGfm)
      .use(remarkRehype)
      .use(rehypeSanitize)
      .use(rehypeStringify)
      .processSync(markdown);

    const html = String(result);

    return this.buildResult(
      {
        'text/plain': html,
        'text/html': html,
      },
      markdown.length,
    );
  }
}
