import { BaseConverter } from './base-converter';
import { convertMarkdown } from './conversion';
import { Platform, type ConversionResult } from './types';

export class DiscordConverter extends BaseConverter {
  readonly platform = Platform.Discord;
  readonly label = 'Discord';

  convert(markdown: string): ConversionResult {
    return this.buildResult(convertMarkdown(markdown, 'discord').representations, markdown.length);
  }
}
