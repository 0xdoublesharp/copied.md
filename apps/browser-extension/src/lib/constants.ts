export const MENU_PARENT_ID = 'copied-md-parent';

export const STORAGE_KEYS = {
  SETTINGS: 'copied-md-settings',
  HISTORY: 'copied-md-history',
  LAST_TARGET: 'copied-md-last-target',
  CHAT_SITES: 'copied-md-chat-sites',
} as const;

export const CHAT_SITES = [
  { id: 'chatgpt', label: 'ChatGPT', host: 'chatgpt.com' },
  { id: 'claude', label: 'Claude', host: 'claude.ai' },
] as const;

export type ChatSite = (typeof CHAT_SITES)[number];

export function chatOrigin(site: ChatSite) {
  return 'https://' + site.host + '/*';
}
