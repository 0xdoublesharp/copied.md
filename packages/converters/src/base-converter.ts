import { unified } from 'unified';
import remarkParse from 'remark-parse';
import remarkGfm from 'remark-gfm';
import type { Root } from 'mdast';
import type { ConversionResult, Platform, PlatformConverter, ClipboardPayload } from './types';

export abstract class BaseConverter implements PlatformConverter {
  abstract readonly platform: Platform;
  abstract readonly label: string;
  readonly maxLength?: number;

  protected parseMarkdown(md: string): Root {
    return unified().use(remarkParse).use(remarkGfm).parse(md);
  }

  abstract convert(markdown: string): ConversionResult;

  protected buildResult(payload: ClipboardPayload, originalLength: number): ConversionResult {
    const plainText = payload['text/plain'];
    const truncated = this.maxLength != null && plainText.length > this.maxLength;
    if (truncated) {
      payload['text/plain'] = plainText.slice(0, this.maxLength!);
    }
    return {
      payload,
      truncated,
      originalLength,
    };
  }
}
