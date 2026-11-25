/**
 * @vitest-environment-options {"url":"https://chatgpt.com/c/example"}
 */
import { afterEach, beforeEach, expect, it, vi } from 'vitest';
import { STORAGE_KEYS } from '@/lib/constants';

type Change = (changes: Record<string, { newValue?: unknown }>, area: string) => void;
const { writeWithFallback, get, set } = vi.hoisted(() => ({
  writeWithFallback: vi.fn(),
  get: vi.fn(),
  set: vi.fn(),
}));
vi.mock('@/lib/clipboard-writer', () => ({ writeWithFallback }));
vi.mock('@/assets/styles/main.css?inline', () => ({ default: ':root { color-scheme: light; }' }));
vi.mock('wxt/browser', () => ({ browser: { storage: { local: { get, set } } } }));
let changed: Change;
let invalidate: () => void;
const removeListener = vi.fn();
const shadows = new WeakMap<Element, ShadowRoot>();

beforeEach(() => {
  vi.resetAllMocks();
  vi.resetModules();
  vi.stubGlobal('defineContentScript', (entry: unknown) => entry);
  vi.stubGlobal('browser', {
    storage: {
      local: { get, set },
      onChanged: {
        addListener: (listener: Change) => {
          changed = listener;
        },
        removeListener,
      },
    },
  });
  vi.stubGlobal(
    'matchMedia',
    vi.fn(() => ({ matches: true })),
  );
  vi.spyOn(window, 'close').mockImplementation(() => undefined);
  const attachShadow = Element.prototype.attachShadow;
  vi.spyOn(Element.prototype, 'attachShadow').mockImplementation(function (this: Element, options) {
    const shadow = attachShadow.call(this, options);
    shadows.set(this, shadow);
    return shadow;
  });
  get.mockResolvedValue({});
  set.mockResolvedValue(undefined);
  writeWithFallback.mockResolvedValue({ fallback: false, types: ['text/html', 'text/plain'] });
  document.body.innerHTML = `
    <main><article data-turn="assistant">
      <div data-message-author-role="assistant">
        <div class="markdown"><p>Read <a href="./notes">these notes</a>.</p></div>
      </div><button data-testid="copy-turn-action-button">Native copy</button>
    </article></main>`;
});

afterEach(() => {
  invalidate?.();
  delete (window as Window & { copiedMdChatCleanup?: () => void }).copiedMdChatCleanup;
  vi.restoreAllMocks();
  vi.unstubAllGlobals();
  document.body.replaceChildren();
});

async function start() {
  const { default: script } = await import('@/entrypoints/chat-replies.content');
  await script.main({
    onInvalidated: (cleanup: () => void) => {
      invalidate = cleanup;
    },
  } as never);
}

function trigger() {
  return document.querySelector<HTMLButtonElement>('button[data-copied-md-reply]');
}

function picker() {
  const host = trigger()?.nextElementSibling;
  return host ? shadows.get(host) : undefined;
}

function operation(label: string) {
  return [...picker()!.querySelectorAll('button')].find(
    (button) => button.textContent?.trim() === label,
  )!;
}

it('reuses the inline picker and clipboard writer without closing the chat tab', async () => {
  get.mockResolvedValueOnce({ [STORAGE_KEYS.CHAT_SITES]: ['chatgpt'] });
  await start();
  await vi.waitFor(() => expect(trigger()).not.toBeNull());
  trigger()!.click();
  await vi.waitFor(() => expect(operation('Copy to Clipboard')).toBeDefined());
  expect(picker()?.querySelector('div.dark')).not.toBeNull();
  expect(trigger()?.nextElementSibling?.shadowRoot).toBeNull();
  expect(operation('Rich Text').getAttribute('aria-pressed')).toBe('true');
  operation('Copy to Clipboard').click();
  await vi.waitFor(() =>
    expect(picker()?.querySelector('[role=status]')?.textContent).toBe('Copied.'),
  );
  expect(writeWithFallback).toHaveBeenLastCalledWith({
    'text/plain': 'Read these notes (https://chatgpt.com/c/notes).',
    'text/html': '<p>Read <a href="https://chatgpt.com/c/notes">these notes</a>.</p>',
  });
  expect(window.close).not.toHaveBeenCalled();
  operation('Close').click();
  expect(picker()).toBeUndefined();
  expect(trigger()?.getAttribute('aria-expanded')).toBe('false');
  expect(document.activeElement).toBe(trigger());
  trigger()!.click();
  expect(picker()).toBeDefined();
  operation('Close').dispatchEvent(new KeyboardEvent('keydown', { key: 'Escape', bubbles: true }));
  expect(picker()).toBeUndefined();
  trigger()!.click();
  changed({ [STORAGE_KEYS.CHAT_SITES]: { newValue: [] } }, 'local');
  expect(trigger()).toBeNull();
});

it('honors an opt-in change while initial storage reads are still pending', async () => {
  let finish!: (value: Record<string, unknown>) => void;
  get.mockReturnValueOnce(
    new Promise((resolve) => {
      finish = resolve;
    }),
  );
  await start();
  expect(trigger()).toBeNull();
  changed({ [STORAGE_KEYS.CHAT_SITES]: { newValue: ['chatgpt'] } }, 'sync');
  changed({}, 'local');
  expect(trigger()).toBeNull();
  changed({ [STORAGE_KEYS.CHAT_SITES]: { newValue: ['claude', 'chatgpt'] } }, 'local');
  expect(trigger()).not.toBeNull();
  changed({ [STORAGE_KEYS.CHAT_SITES]: { newValue: ['chatgpt'] } }, 'local');
  expect(document.querySelectorAll('[data-copied-md-reply]')).toHaveLength(1);
  finish({});
  await Promise.resolve();
  expect(trigger()).not.toBeNull();
  invalidate();
  expect(trigger()).toBeNull();
  expect(removeListener).toHaveBeenCalledWith(changed);
});

it('does not install controls when permission was not granted, and recovers on a later grant', async () => {
  get.mockRejectedValueOnce(new Error('storage unavailable'));
  await start();
  expect(trigger()).toBeNull();
  changed({ [STORAGE_KEYS.CHAT_SITES]: { newValue: ['chatgpt'] } }, 'local');
  expect(trigger()).not.toBeNull();
  get.mockResolvedValueOnce({ [STORAGE_KEYS.CHAT_SITES]: ['claude'] });
  await start();
  expect(trigger()).toBeNull();
});

it('ignores a document outside the supported origins', async () => {
  vi.stubGlobal('location', new URL('https://example.org/'));
  await start();
  expect(get).not.toHaveBeenCalled();
  expect(trigger()).toBeNull();
});

it('warns before copying a Signal reply with a spoiler and redacts the fallback', async () => {
  document.querySelector('.markdown')!.innerHTML =
    'Public <span data-spoiler="true">Hidden</span> text';
  get.mockResolvedValueOnce({ [STORAGE_KEYS.CHAT_SITES]: ['chatgpt'] });
  await start();
  await vi.waitFor(() => expect(trigger()).not.toBeNull());
  trigger()!.click();
  await vi.waitFor(() => expect(operation('Copy to Clipboard')).toBeDefined());
  operation('Signal').click();
  await vi.waitFor(() =>
    expect(picker()?.querySelector('li')?.textContent).toContain(
      'Plain-text paste omits Signal spoilers.',
    ),
  );
  expect(writeWithFallback).not.toHaveBeenCalled();
  expect(picker()?.querySelector('pre')?.textContent).toBe('Public [spoiler omitted] text');
  operation('Copy to Clipboard').click();
  await vi.waitFor(() =>
    expect(writeWithFallback).toHaveBeenCalledWith({
      'text/plain': 'Public [spoiler omitted] text',
      'text/signal': 'Public <span data-spoiler="true">Hidden</span> text',
    }),
  );
});
