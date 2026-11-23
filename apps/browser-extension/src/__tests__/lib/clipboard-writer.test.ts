import { afterEach, beforeEach, expect, it, vi } from 'vitest';
import { writeExact, writeWithFallback } from '@/lib/clipboard-writer';

const rich = { 'text/plain': 'Hello', 'text/html': '<strong>Hello</strong>' };
const write = vi.fn();
const writeText = vi.fn();
const exec = vi.fn();

beforeEach(() => {
  vi.resetAllMocks();
  vi.stubGlobal(
    'ClipboardItem',
    class {
      constructor(readonly representations: Record<string, Blob>) {}
    },
  );
  Object.defineProperty(navigator, 'clipboard', {
    configurable: true,
    value: { write, writeText },
  });
  Object.defineProperty(document, 'execCommand', { configurable: true, value: exec });
});

afterEach(() => {
  Reflect.deleteProperty(navigator, 'clipboard');
  Reflect.deleteProperty(document, 'execCommand');
  vi.unstubAllGlobals();
  vi.restoreAllMocks();
  document.body.replaceChildren();
});

it('requests only the HTML and plain clipboard flavors the browser can write', async () => {
  expect(await writeWithFallback({ ...rich, 'text/rtf': 'rtf' })).toEqual({
    fallback: false,
    types: ['text/plain', 'text/html'],
  });
  const [{ representations }] = write.mock.calls[0][0];
  expect(Object.keys(representations)).toEqual(['text/plain', 'text/html']);
  expect(await representations['text/plain'].text()).toBe('Hello');
  expect(await representations['text/html'].text()).toBe(rich['text/html']);
  expect(writeText).not.toHaveBeenCalled();
});

it('uses the plain clipboard API without HTML', async () => {
  expect(await writeExact({ 'text/plain': 'Hello' })).toEqual(['text/plain']);
  expect(writeText).toHaveBeenCalledWith('Hello');
  expect(write).not.toHaveBeenCalled();
});

it('writes both flavors through a legacy copy event and restores focus and selection', async () => {
  write.mockRejectedValue(new Error('denied'));
  const input = document.createElement('button');
  const paragraph = document.createElement('p');
  paragraph.textContent = 'Keep this selection';
  document.body.append(input, paragraph);
  input.focus();
  document.getSelection()!.selectAllChildren(paragraph);
  expect(document.getSelection()?.toString()).toBe('Keep this selection');
  const setData = vi.fn();
  exec.mockImplementation(() => {
    const event = new Event('copy', { cancelable: true });
    Object.defineProperty(event, 'clipboardData', { value: { setData } });
    document.dispatchEvent(event);
    expect(event.defaultPrevented).toBe(true);
    return true;
  });

  expect(await writeExact(rich)).toEqual(['text/plain', 'text/html']);
  expect(setData.mock.calls).toEqual(Object.entries(rich));
  expect(document.activeElement).toBe(input);
  expect(document.getSelection()?.toString()).toBe('Keep this selection');
  expect(document.querySelector('textarea')).toBeNull();
});

it('falls back when a copy event lacks clipboard data', async () => {
  write.mockRejectedValue(new Error('denied'));
  exec.mockImplementation(() => {
    document.dispatchEvent(new Event('copy', { cancelable: true }));
    return true;
  });
  vi.spyOn(document, 'getSelection').mockReturnValue(null);
  vi.spyOn(document, 'activeElement', 'get').mockReturnValue(null);
  expect(await writeWithFallback(rich)).toEqual({ types: ['text/plain'], fallback: true });
  expect(writeText).toHaveBeenCalledWith('Hello');
});

it('falls back to a legacy plain copy, and reports failure without retrying a plain write', async () => {
  writeText.mockRejectedValue(new Error('denied'));
  exec.mockReturnValueOnce(true);
  expect(await writeExact({ 'text/plain': 'Hello' })).toEqual(['text/plain']);
  exec.mockImplementation(() => {
    throw new Error('copy disabled');
  });
  await expect(writeWithFallback({ 'text/plain': 'Hello' })).rejects.toThrow(
    'Clipboard access failed.',
  );
  expect(exec).toHaveBeenCalledTimes(2);
  expect(document.querySelector('textarea')).toBeNull();
});
