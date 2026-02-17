import { Platform } from '@copied-md/converters';

export const MENU_PARENT_ID = 'copied-md-parent';

export const PLATFORM_MENU_IDS: Record<Platform, string> = {
  [Platform.Facebook]: 'copied-md-facebook',
  [Platform.Twitter]: 'copied-md-twitter',
  [Platform.Slack]: 'copied-md-slack',
  [Platform.Discord]: 'copied-md-discord',
  [Platform.Telegram]: 'copied-md-telegram',
  [Platform.SMS]: 'copied-md-sms',
  [Platform.Reddit]: 'copied-md-reddit',
  [Platform.HTML]: 'copied-md-html',
};

export const PLATFORM_LABELS: Record<Platform, string> = {
  [Platform.Facebook]: 'Facebook',
  [Platform.Twitter]: 'Twitter/X',
  [Platform.Slack]: 'Slack',
  [Platform.Discord]: 'Discord',
  [Platform.Telegram]: 'Telegram',
  [Platform.SMS]: 'SMS',
  [Platform.Reddit]: 'Reddit',
  [Platform.HTML]: 'HTML',
};

const menuIdToPlatformMap = new Map<string, Platform>(
  Object.entries(PLATFORM_MENU_IDS).map(([platform, menuId]) => [menuId, platform as Platform]),
);

export function menuIdToPlatform(menuId: string): Platform | undefined {
  return menuIdToPlatformMap.get(menuId);
}

export const STORAGE_KEYS = {
  SETTINGS: 'local:settings',
  HISTORY: 'local:history',
} as const;
