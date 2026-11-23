import { afterEach, beforeEach, expect, it, vi } from 'vitest';
import { nextTick } from 'vue';
import { STORAGE_KEYS } from '@/lib/constants';

const { set, get, writeWithFallback } = vi.hoisted(() => ({
  set: vi.fn(),
  get: vi.fn(),
  writeWithFallback: vi.fn(),
}));
vi.mock('wxt/browser', () => ({ browser: { storage: { local: { set, get } } } }));
vi.mock('@/lib/clipboard-writer', () => ({ writeWithFallback }));

function button(label: string) {
  const found = [...document.querySelectorAll('button')].find(
    (element) => element.textContent?.trim() === label,
  );
  if (!found) throw new Error('Missing popup button ' + label);
  return found;
}

async function paste(value: string) {
  const editor = document.querySelector('textarea')!;
  editor.value = value;
  editor.dispatchEvent(new Event('input'));
  await nextTick();
}

async function open() {
  document.body.innerHTML = '<div id="app"></div>';
  await import('@/entrypoints/popup/main');
  await vi.waitFor(() => expect(button('Rich Text').getAttribute('aria-pressed')).toBe('true'));
}

beforeEach(() => {
  vi.resetAllMocks();
  vi.resetModules();
  vi.stubGlobal('browser', { storage: { local: { set, get } } });
  set.mockResolvedValue(undefined);
  get.mockResolvedValue({});
  writeWithFallback.mockResolvedValue({ fallback: false, types: ['text/plain', 'text/html'] });
  vi.spyOn(window, 'close').mockImplementation(() => undefined);
});

afterEach(() => {
  vi.restoreAllMocks();
  vi.unstubAllGlobals();
  document.body.replaceChildren();
});

it('previews and copies rich text from the popup, then closes when configured', async () => {
  await open();
  expect(document.querySelector('pre')).toBeNull();
  await paste('**Ready**');
  expect(document.querySelector('pre')?.textContent).toBe('Ready');
  expect(document.body.textContent).toContain('Plain text preview');
  button('Copy to Clipboard').click();
  await vi.waitFor(() =>
    expect(document.querySelector('[role=status]')?.textContent).toBe('Copied.'),
  );
  expect(writeWithFallback).toHaveBeenCalledWith({
    'text/plain': 'Ready',
    'text/html': '<p><strong>Ready</strong></p>',
  });
  expect(set).toHaveBeenCalledWith({ [STORAGE_KEYS.LAST_TARGET]: 'rich-text' });
  expect(window.close).toHaveBeenCalledOnce();
});

it('reports clipboard errors and fallback, then clears the stale status on edit', async () => {
  await open();
  await paste('Notes');
  writeWithFallback.mockRejectedValueOnce(new Error('denied'));
  button('Copy to Clipboard').click();
  await vi.waitFor(() =>
    expect(document.querySelector('[role=alert]')?.textContent).toContain('Try copying'),
  );
  writeWithFallback.mockResolvedValueOnce({ fallback: true, types: ['text/plain'] });
  button('Copy to Clipboard').click();
  await vi.waitFor(() =>
    expect(document.querySelector('[role=status]')?.textContent).toBe('Copied as plain text.'),
  );
  expect(window.close).not.toHaveBeenCalled();
  await paste('New notes');
  expect(document.querySelector('[role=status]')).toBeNull();
});

it('keeps warnings visible and offers a Unicode toggle for supported targets', async () => {
  await open();
  button('X').click();
  await paste('**abc** ' + '漢'.repeat(150));
  expect(document.querySelector('li')?.textContent).toContain('280 weighted characters');
  expect(document.body.textContent).toContain('/ 280');
  const toggle = document.querySelector<HTMLInputElement>('input[type=checkbox]')!;
  toggle.click();
  await nextTick();
  expect(document.querySelector('pre')?.textContent).toContain('𝗮𝗯𝗰');
  button('Copy to Clipboard').click();
  await vi.waitFor(() => expect(document.querySelector('[role=status]')).not.toBeNull());
  expect(window.close).not.toHaveBeenCalled();
});

it('ignores completion for older content and disables the pending copy button', async () => {
  await open();
  await paste('Old');
  let finish!: (value: { fallback: boolean; types: string[] }) => void;
  const pending = new Promise<{ fallback: boolean; types: string[] }>((resolve) => {
    finish = resolve;
  });
  writeWithFallback.mockReturnValueOnce(pending);
  button('Copy to Clipboard').click();
  await nextTick();
  expect(button('Copying…').disabled).toBe(true);
  button('Copying…').dispatchEvent(new Event('click'));
  expect(writeWithFallback).toHaveBeenCalledOnce();
  await paste('New');
  set.mockRejectedValueOnce(new Error('storage unavailable'));
  finish({ fallback: false, types: ['text/plain'] });
  await vi.waitFor(() => expect(button('Copy to Clipboard').disabled).toBe(false));
  expect(document.querySelector('[role=status]')).toBeNull();
  expect(window.close).not.toHaveBeenCalled();
  await paste('[x]: https://example.org');
  expect(button('Copy to Clipboard').disabled).toBe(true);
});

it('does not show a clipboard error from an older edit', async () => {
  await open();
  await paste('Old notes');
  let reject!: (error: Error) => void;
  writeWithFallback.mockReturnValueOnce(
    new Promise((_resolve, fail) => {
      reject = fail;
    }),
  );
  button('Copy to Clipboard').click();
  await nextTick();
  expect(button('Copying…').disabled).toBe(true);
  await paste('New notes');
  reject(new Error('permission denied'));
  await vi.waitFor(() => expect(button('Copy to Clipboard').disabled).toBe(false));
  expect(document.querySelector('[role=alert]')).toBeNull();
  expect(document.querySelector('pre')?.textContent).toBe('New notes');
});

it('saves settings changes and returns to the converter', async () => {
  await open();
  button('Settings').click();
  await nextTick();
  const select = document.querySelector('select')!;
  select.value = 'telegram';
  select.dispatchEvent(new Event('change'));
  const [notifications, autoClose] =
    document.querySelectorAll<HTMLInputElement>('input[type=checkbox]');
  notifications.click();
  autoClose.click();
  await vi.waitFor(() =>
    expect(set).toHaveBeenCalledWith({
      [STORAGE_KEYS.SETTINGS]: {
        defaultPlatform: 'telegram',
        showNotifications: false,
        autoClose: false,
      },
    }),
  );
  button('Convert').click();
  await nextTick();
  expect(document.querySelector('textarea')).not.toBeNull();
  await paste('Ready');
  button('Copy to Clipboard').click();
  await vi.waitFor(() => expect(document.querySelector('[role=status]')).not.toBeNull());
  expect(window.close).not.toHaveBeenCalled();
});
