import { BaseConverter } from './base-converter';
import { convertMarkdown } from './conversion';
import { Platform, type ConversionResult } from './types';

export class RedditConverter extends BaseConverter {
  readonly platform = Platform.Reddit;
  readonly label = 'Reddit';

  convert(markdown: string): ConversionResult {
    return this.buildResult(convertMarkdown(markdown, 'reddit').representations, markdown.length);
  }
}
