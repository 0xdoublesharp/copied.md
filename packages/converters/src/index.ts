export { Platform } from './types';
export type { PlatformConverter, ConversionResult, ClipboardPayload } from './types';

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

const converters: Map<Platform, PlatformConverter> = new Map([
  [Platform.Facebook, new FacebookConverter()],
  [Platform.Twitter, new TwitterConverter()],
  [Platform.Slack, new SlackConverter()],
  [Platform.Discord, new DiscordConverter()],
  [Platform.Telegram, new TelegramConverter()],
  [Platform.SMS, new SmsConverter()],
  [Platform.Reddit, new RedditConverter()],
  [Platform.HTML, new HtmlConverter()],
]);

export function getConverter(platform: Platform): PlatformConverter {
  const converter = converters.get(platform);
  if (!converter) throw new Error(`No converter for platform: ${platform}`);
  return converter;
}

export function getAllPlatforms(): Platform[] {
  return Object.values(Platform);
}
