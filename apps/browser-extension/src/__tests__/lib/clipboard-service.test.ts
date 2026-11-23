import { afterEach, describe, expect, it, vi } from 'vitest';
import { writeToClipboard } from '@/lib/clipboard-service';

const payload = { 'text/plain': 'hello', 'text/html': '<b>hello</b>' };

function offscreen(sendMessage: () => Promise<unknown>, contexts: unknown[] = []) {
  const createDocument = vi.fn(async () => undefined);
  vi.stubGlobal('chrome', {
    offscreen: { createDocument },
    runtime: {
      getURL: (path: string) => path,
      getContexts: vi.fn(async () => contexts),
      sendMessage,
    },
  });
  return createDocument;
}

describe('Clipboard routing', () => {
  afterEach(() => vi.unstubAllGlobals());

  it('writes the requested types in the invoking frame', async () => {
    const sendMessage = vi.fn(async () => ({ types: ['text/plain', 'text/html'] }));
    vi.stubGlobal('browser', { tabs: { sendMessage } });
    await expect(writeToClipboard(payload, 123, 7)).resolves.toEqual({
      types: ['text/plain', 'text/html'],
      fallback: false,
    });
    expect(sendMessage).toHaveBeenCalledWith(
      123,
      {
        type: 'WRITE_CLIPBOARD',
        destination: 'content',
        payload,
      },
      { frameId: 7 },
    );
  });

  it('tries offscreen rich before accepting plain clipboard output', async () => {
    vi.stubGlobal('browser', {
      tabs: { sendMessage: vi.fn(async () => ({ types: ['text/plain'] })) },
    });
    const sendMessage = vi.fn(async () => ({ types: ['text/plain', 'text/html'] }));
    const createDocument = offscreen(sendMessage);
    await expect(writeToClipboard(payload, 123)).resolves.toMatchObject({ fallback: false });
    expect(createDocument).toHaveBeenCalledOnce();
    expect(sendMessage).toHaveBeenCalledWith({
      type: 'WRITE_CLIPBOARD',
      destination: 'offscreen',
      payload,
    });
  });

  it('reports plain fallback after exact rich writes fail', async () => {
    const sendMessage = vi
      .fn()
      .mockResolvedValueOnce({ error: 'denied' })
      .mockResolvedValueOnce({ types: ['text/plain'] });
    vi.stubGlobal('browser', { tabs: { sendMessage } });
    vi.stubGlobal('chrome', undefined);
    await expect(writeToClipboard(payload, 123)).resolves.toEqual({
      types: ['text/plain'],
      fallback: true,
    });
    expect(sendMessage.mock.calls[1]?.[1].payload).toEqual({ 'text/plain': 'hello' });
  });

  it('serializes pending writes and surfaces rejection', async () => {
    let finish: ((value: { error: string }) => void) | undefined;
    const first = new Promise<{ error: string }>((resolve) => {
      finish = resolve;
    });
    const sendMessage = vi
      .fn()
      .mockReturnValueOnce(first)
      .mockResolvedValue({ types: ['text/plain'] });
    vi.stubGlobal('browser', { tabs: { sendMessage } });
    vi.stubGlobal('chrome', undefined);

    const pending = writeToClipboard({ 'text/plain': 'first' }, 123);
    const second = writeToClipboard({ 'text/plain': 'second' }, 123);
    const rejected = expect(pending).rejects.toThrow('denied');
    await vi.waitFor(() => expect(sendMessage).toHaveBeenCalledTimes(1));
    finish?.({ error: 'denied' });
    await rejected;
    await expect(second).resolves.toMatchObject({ fallback: false });
  });

  it('reuses an offscreen document and tries plain text after a rich write error', async () => {
    const sendMessage = vi
      .fn()
      .mockRejectedValueOnce(new Error('rich access denied'))
      .mockResolvedValueOnce({ types: ['text/plain'] });
    const createDocument = offscreen(sendMessage, [{}]);
    await expect(writeToClipboard(payload)).resolves.toEqual({
      types: ['text/plain'],
      fallback: true,
    });
    expect(createDocument).not.toHaveBeenCalled();
  });

  it('reports a missing browser clipboard context', async () => {
    vi.stubGlobal('chrome', undefined);
    await expect(writeToClipboard({ 'text/plain': 'hello' })).rejects.toThrow(
      'no available clipboard context',
    );
    vi.stubGlobal('chrome', {});
    await expect(writeToClipboard({ 'text/plain': 'hello' })).rejects.toThrow(
      'no available clipboard context',
    );
  });
});
