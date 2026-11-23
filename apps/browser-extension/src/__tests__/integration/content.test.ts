import { afterEach, beforeEach, expect, it, vi } from 'vitest';

type Listener = (
  message: Record<string, unknown>,
  sender: unknown,
  reply: (result: unknown) => void,
) => true | undefined;

const { writeExact } = vi.hoisted(() => ({ writeExact: vi.fn() }));
vi.mock('@/lib/clipboard-writer', () => ({ writeExact }));
let listener: Listener;
const reply = vi.fn();

beforeEach(() => {
  vi.resetAllMocks();
  vi.resetModules();
  vi.useFakeTimers();
  const runtime = {
    onMessage: {
      addListener: (callback: Listener) => {
        listener = callback;
      },
    },
  };
  vi.stubGlobal('browser', { runtime });
  vi.stubGlobal('chrome', { runtime });
  vi.stubGlobal('defineContentScript', (entry: { main: () => void }) => entry);
});

afterEach(() => {
  vi.useRealTimers();
  vi.unstubAllGlobals();
  vi.restoreAllMocks();
  document.body.replaceChildren();
});

it('reads selected form text and document text without exposing passwords', async () => {
  const entry = (await import('@/entrypoints/content')).default;
  await entry.main({} as never);
  const field = document.createElement('input');
  field.value = 'secret sample';
  document.body.append(field);
  field.focus();
  field.setSelectionRange(7, 13);
  listener({ type: 'READ_SELECTION' }, {}, reply);
  expect(reply).toHaveBeenLastCalledWith({
    text: 'sample',
    blocked: false,
    sourceUrl: document.location.href,
  });
  field.type = 'password';
  listener({ type: 'READ_SELECTION' }, {}, reply);
  expect(reply).toHaveBeenLastCalledWith(expect.objectContaining({ text: '', blocked: true }));

  const editor = document.createElement('textarea');
  editor.value = 'first\nsecond';
  document.body.append(editor);
  editor.focus();
  editor.setSelectionRange(6, editor.value.length);
  listener({ type: 'READ_SELECTION' }, {}, reply);
  expect(reply).toHaveBeenLastCalledWith(
    expect.objectContaining({ text: 'second', blocked: false }),
  );

  field.type = 'number';
  field.focus();
  const p = document.createElement('p');
  p.textContent = 'document selection';
  document.body.append(p);
  document.getSelection()!.selectAllChildren(p);
  listener({ type: 'READ_SELECTION' }, {}, reply);
  expect(reply).toHaveBeenLastCalledWith(expect.objectContaining({ text: 'document selection' }));
  field.blur();
  listener({ type: 'READ_SELECTION' }, {}, reply);
  expect(reply).toHaveBeenLastCalledWith(expect.objectContaining({ text: 'document selection' }));
  vi.spyOn(document, 'getSelection').mockReturnValue(null);
  listener({ type: 'READ_SELECTION' }, {}, reply);
  expect(reply).toHaveBeenLastCalledWith(expect.objectContaining({ text: '', blocked: false }));
});

it('renders a single accessible toast and removes it when its timer ends', async () => {
  await (await import('@/entrypoints/content')).default.main({} as never);
  listener({ type: 'COPY_STATUS', text: 'Old status' }, {}, reply);
  listener({ type: 'COPY_STATUS', text: '<strong>Copied</strong>' }, {}, reply);
  const toasts = document.querySelectorAll('[role=status]');
  expect(toasts).toHaveLength(1);
  expect(toasts[0].textContent).toBe('<strong>Copied</strong>');
  expect(toasts[0].children).toHaveLength(0);
  await vi.advanceTimersByTimeAsync(30);
  expect((toasts[0] as HTMLElement).style.opacity).toBe('1');
  await vi.advanceTimersByTimeAsync(2470);
  expect(document.querySelector('[role=status]')).toBeNull();
});

it.each(['content', 'offscreen'])(
  'handles %s clipboard messages independently',
  async (destination) => {
    if (destination === 'content')
      await (await import('@/entrypoints/content')).default.main({} as never);
    else await import('@/lib/offscreen-handler');
    expect(listener({ type: 'OTHER' }, {}, reply)).toBeUndefined();
    expect(listener({ type: 'WRITE_CLIPBOARD', destination: 'other' }, {}, reply)).toBeUndefined();
    expect(writeExact).not.toHaveBeenCalled();

    const payload = { 'text/plain': 'Notes' };
    writeExact.mockResolvedValueOnce(['text/plain']).mockRejectedValueOnce(new Error('denied'));
    const message = { type: 'WRITE_CLIPBOARD', destination, payload };
    expect(listener(message, {}, reply)).toBe(true);
    await Promise.resolve();
    expect(writeExact).toHaveBeenLastCalledWith(payload);
    expect(reply).toHaveBeenLastCalledWith({ types: ['text/plain'] });
    expect(listener(message, {}, reply)).toBe(true);
    await Promise.resolve();
    await Promise.resolve();
    expect(reply).toHaveBeenLastCalledWith({ error: 'Error: denied' });
  },
);
