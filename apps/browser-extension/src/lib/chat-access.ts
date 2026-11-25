import { CHAT_SITES, STORAGE_KEYS, chatOrigin } from './constants';

const contentFile = 'content-scripts/chat-replies.js';
let registrationQueue = Promise.resolve();

export function registerChatAccess() {
  function schedule() {
    registrationQueue = registrationQueue.then(syncChatAccess).catch(() => undefined);
  }

  browser.permissions.onAdded.addListener(schedule);
  browser.permissions.onRemoved.addListener(schedule);
  browser.runtime.onInstalled.addListener(schedule);
  browser.runtime.onStartup.addListener(schedule);
}

async function syncChatAccess() {
  const registered = await browser.scripting.getRegisteredContentScripts();
  const enabled = [];
  const refresh = [];
  for (const site of CHAT_SITES) {
    const origin = chatOrigin(site);
    const id = 'copied-md-' + site.id;
    const active = registered.some((script) => script.id === id);
    const granted = await browser.permissions.contains({ origins: [origin] });
    if (granted) {
      if (!active) {
        await browser.scripting.registerContentScripts([
          { id, matches: [origin], js: [contentFile], runAt: 'document_idle' },
        ]);
        refresh.push(origin);
      }
      enabled.push(site.id);
    } else if (active) {
      await browser.scripting.unregisterContentScripts({ ids: [id] });
    }
  }
  await browser.storage.local.set({ [STORAGE_KEYS.CHAT_SITES]: enabled });
  if (!refresh.length) return;
  const tabs = await browser.tabs.query({ url: refresh });
  await Promise.allSettled(
    tabs.flatMap(({ id }) =>
      id === undefined
        ? []
        : browser.scripting.executeScript({ target: { tabId: id }, files: [contentFile] }),
    ),
  );
}
