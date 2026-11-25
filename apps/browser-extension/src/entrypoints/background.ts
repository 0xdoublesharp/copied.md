// Keep this first for the service worker's Markdown entity decoder.
import '@/lib/dom-polyfill';

import {
  convertMarkdown,
  resolveTarget,
  targets,
  type CopyTarget,
} from '@0xdoublesharp/copied-md-converters';
import { MENU_PARENT_ID, STORAGE_KEYS } from '@/lib/constants';
import { writeToClipboard, type SelectionSource } from '@/lib/clipboard-service';
import { preferredFormat, visibleFormats } from '@/lib/copy-formats';
import type { SettingsState } from '@/stores/settings.store';
import { registerChatAccess } from '@/lib/chat-access';

let selectionQueue: Promise<unknown> = Promise.resolve();
let menuQueue: Promise<unknown> = Promise.resolve();

async function readSelection(tabId: number, frameId: number): Promise<SelectionSource | undefined> {
  const request = () => browser.tabs.sendMessage(tabId, { type: 'READ_SELECTION' }, { frameId });
  const existing = await request().catch(() => undefined);
  if (existing) return existing;
  try {
    const file = 'content-scripts/content.js';
    if (browser.runtime.getManifest().manifest_version === 3) {
      await browser.scripting.executeScript({
        target: { tabId, frameIds: [frameId] },
        files: [file],
      });
    } else {
      await browser.tabs.executeScript(tabId, { file: '/' + file, frameId });
    }
    return await request();
  } catch {
    return undefined;
  }
}

async function report(
  text: string,
  tabId: number | undefined,
  frameId: number,
  error: boolean,
  toast: boolean,
) {
  const action = browser.action ?? browser.browserAction;
  await Promise.allSettled([
    action?.setBadgeText({ text: error ? '!' : '' }),
    action?.setTitle({ title: 'copied.md · ' + text }),
    toast && tabId !== undefined
      ? browser.tabs.sendMessage(tabId, { type: 'COPY_STATUS', text }, { frameId })
      : undefined,
  ]);
}

function copySelection(target: CopyTarget, tabId?: number, frameId = 0, fallback = '') {
  selectionQueue = selectionQueue.then(async () => {
    try {
      const source = tabId === undefined ? undefined : await readSelection(tabId, frameId);
      const text = source?.blocked ? '' : source?.text || fallback;
      if (!text.trim()) {
        await report('Select Markdown to copy.', tabId, frameId, true, true);
        return;
      }
      const result = convertMarkdown(text, target, { sourceUrl: source?.sourceUrl });
      if (!result.representations['text/plain'].trim()) {
        await report('There is no text to copy.', tabId, frameId, true, true);
        return;
      }
      const outcome = await writeToClipboard(result.representations, tabId, frameId);
      const settings = await browser.storage.local.get(STORAGE_KEYS.SETTINGS).catch(() => ({}));
      const showNotifications =
        (settings as Record<string, { showNotifications?: boolean }>)[STORAGE_KEYS.SETTINGS]
          ?.showNotifications ?? true;
      await browser.storage.local
        .set({ [STORAGE_KEYS.LAST_TARGET]: target })
        .catch(() => undefined);
      const label = targets.find((format) => format.id === target)!.label;
      const warning = result.diagnostics.find((diagnostic) => diagnostic.severity === 'warning');
      const message = outcome.fallback ? 'Copied as plain text.' : 'Copied as ' + label + '.';
      const attention = outcome.fallback || !!warning;
      await report(
        message + (warning ? ' ' + warning.message : ''),
        tabId,
        frameId,
        attention,
        showNotifications || attention,
      );
    } catch {
      await report('Copy failed. Open copied.md to paste and convert.', tabId, frameId, true, true);
    }
  });
}

function refreshMenus() {
  menuQueue = menuQueue
    .then(async () => {
      const saved = await browser.storage.local.get(STORAGE_KEYS.SETTINGS).catch(() => ({}));
      const settings = (saved as Record<string, Partial<SettingsState>>)[STORAGE_KEYS.SETTINGS];
      await browser.contextMenus.removeAll();
      browser.contextMenus.create({
        id: MENU_PARENT_ID,
        title: 'Copy Markdown As…',
        contexts: ['selection'],
      });
      for (const format of visibleFormats(settings?.formatVisibility)) {
        browser.contextMenus.create({
          id: 'copied-md-' + format.id,
          parentId: MENU_PARENT_ID,
          title: format.label,
          contexts: ['selection'],
        });
      }
    })
    .catch(() => undefined);
}

export default defineBackground(() => {
  registerChatAccess();
  browser.runtime.onInstalled.addListener(refreshMenus);
  browser.storage.onChanged.addListener((changes, area) => {
    if (area === 'local' && changes[STORAGE_KEYS.SETTINGS]) refreshMenus();
  });

  browser.contextMenus.onClicked.addListener((info, tab) => {
    const target = resolveTarget(String(info.menuItemId).replace(/^copied-md-/, ''));
    if (target) copySelection(target, tab?.id, info.frameId, info.selectionText);
  });

  browser.commands.onCommand.addListener(async (command) => {
    if (command !== 'copy-last-target') return;
    try {
      const [tab] = await browser.tabs.query({ active: true, currentWindow: true });
      const saved = await browser.storage.local.get([
        STORAGE_KEYS.LAST_TARGET,
        STORAGE_KEYS.SETTINGS,
      ]);
      const settings = saved[STORAGE_KEYS.SETTINGS] as Partial<SettingsState> | undefined;
      const target = preferredFormat(
        settings?.formatVisibility,
        saved[STORAGE_KEYS.LAST_TARGET],
        settings?.defaultPlatform,
      );
      copySelection(target, tab?.id);
    } catch {
      await report('Copy failed. Open copied.md to paste and convert.', undefined, 0, true, false);
    }
  });
});
