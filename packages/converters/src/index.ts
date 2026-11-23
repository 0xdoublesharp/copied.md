export { Platform } from './types';
export type { PlatformConverter, ConversionResult, ClipboardPayload } from './types';
export { convertMarkdown, resolveTarget, targets } from './conversion';
export type { ConversionOptions, CopyTarget, MarkdownConversion } from './conversion';

import { Platform } from './types';
import type { PlatformConverter } from './types';
import { FacebookConverter } from './facebook';
import { TwitterConverter } from './twitter';
import { SlackConverter } from './slack';
import { DiscordConverter } from './discord';
import { TelegramConverter } from './telegram';
import { SmsConverter } from './sms';
import { RedditConverter } from './reddit';
import { HtmlConverter } from './html';

const converters = new Map<Platform, PlatformConverter>();
converters.set(Platform.Facebook, new FacebookConverter());
converters.set(Platform.Twitter, new TwitterConverter());
converters.set(Platform.Slack, new SlackConverter());
converters.set(Platform.Discord, new DiscordConverter());
converters.set(Platform.Telegram, new TelegramConverter());
converters.set(Platform.SMS, new SmsConverter());
converters.set(Platform.Reddit, new RedditConverter());
converters.set(Platform.HTML, new HtmlConverter());

export function getConverter(platform: Platform): PlatformConverter {
  const converter = converters.get(platform);
  if (!converter) throw new Error(`No converter for platform: ${platform}`);
  return converter;
}

export function getAllPlatforms(): Platform[] {
  return Object.values(Platform);
}
