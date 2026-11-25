import { createApp } from 'vue';
import { createPinia } from 'pinia';
import ReplyPicker from '@/components/ReplyPicker.vue';
import stylesheet from '@/assets/styles/main.css?inline';
import { installReplyControls } from '@/lib/chat-replies';
import { CHAT_SITES, STORAGE_KEYS } from '@/lib/constants';

function openPicker(markdown: string, trigger: HTMLButtonElement) {
  const host = document.createElement('div');
  const shadow = host.attachShadow({ mode: 'closed' });
  const style = document.createElement('style');
  style.textContent = stylesheet.replaceAll(':root', ':host');
  const container = document.createElement('div');
  container.classList.toggle('dark', window.matchMedia('(prefers-color-scheme: dark)').matches);
  shadow.append(style, container);
  trigger.after(host);
  trigger.setAttribute('aria-expanded', 'true');
  const app = createApp(ReplyPicker, {
    markdown,
    sourceUrl: location.href,
    onClose: () => {
      remove();
      trigger.focus();
    },
  });
  let removed = false;
  function remove() {
    if (removed) return;
    removed = true;
    app.unmount();
    host.remove();
    trigger.setAttribute('aria-expanded', 'false');
  }
  app.use(createPinia());
  app.mount(container);
  shadow.querySelector('button')?.focus();
  return remove;
}

export default defineContentScript({
  registration: 'runtime',
  noScriptStartedPostMessage: true,
  main(ctx) {
    const site = CHAT_SITES.find(({ host }) => host === location.hostname);
    if (!site) return;
    const { id } = site;
    const world = window as Window & { copiedMdChatCleanup?: () => void };
    world.copiedMdChatCleanup?.();
    let dispose: (() => void) | undefined;
    let skipInitialState = false;
    function apply(enabled: unknown) {
      const allowed = Array.isArray(enabled) && enabled.includes(id);
      if (allowed)
        dispose ??= installReplyControls(document, id, () => location.pathname, openPicker);
      else {
        dispose?.();
        dispose = undefined;
      }
    }
    const listener: Parameters<typeof browser.storage.onChanged.addListener>[0] = (
      changes,
      area,
    ) => {
      if (area !== 'local' || !changes[STORAGE_KEYS.CHAT_SITES]) return;
      skipInitialState = true;
      apply(changes[STORAGE_KEYS.CHAT_SITES].newValue);
    };
    browser.storage.onChanged.addListener(listener);
    const cleanup = () => {
      skipInitialState = true;
      browser.storage.onChanged.removeListener(listener);
      dispose?.();
    };
    world.copiedMdChatCleanup = cleanup;
    ctx.onInvalidated(cleanup);
    void browser.storage.local
      .get(STORAGE_KEYS.CHAT_SITES)
      .then((saved) => {
        if (!skipInitialState) apply(saved[STORAGE_KEYS.CHAT_SITES]);
      })
      .catch(() => undefined);
  },
});
