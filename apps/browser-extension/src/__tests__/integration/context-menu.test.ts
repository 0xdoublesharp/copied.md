import { describe, it, expect, vi } from 'vitest';
import { targets } from '@0xdoublesharp/copied-md-converters';
import { MENU_PARENT_ID } from '@/lib/constants';

type Click = (
  info: { menuItemId: string; frameId: number; selectionText: string },
  tab: { id: number },
) => void;

describe('Browser selection copy', () => {
  it('registers all targets and converts the live selection from its frame', async () => {
    let install: (() => void) | undefined;
    let click: Click | undefined;
    const create = vi.fn();
    const status = vi.fn();
    let injected = false;
    const executeScript = vi.fn(async () => {
      injected = true;
    });
    const sendMessage = vi.fn(async (_tab, message) => {
      if (message.type === 'READ_SELECTION' && injected)
        return { text: 'Read [more](article).', sourceUrl: 'https://example.com/' };
      if (message.type === 'WRITE_CLIPBOARD') return { types: ['text/plain', 'text/html'] };
      return undefined;
    });
    vi.stubGlobal('defineBackground', (callback: () => void) => callback);
    vi.stubGlobal('browser', {
      scripting: { executeScript },
      runtime: {
        getManifest: () => ({ manifest_version: 3 }),
        onInstalled: {
          addListener: (callback: () => void) => {
            install = callback;
          },
        },
      },
      commands: { onCommand: { addListener: vi.fn() } },
      contextMenus: {
        removeAll: vi.fn(async () => undefined),
        create,
        onClicked: {
          addListener: (callback: Click) => {
            click = callback;
          },
        },
      },
      tabs: { sendMessage },
      storage: { local: { get: vi.fn(async () => ({})), set: vi.fn(async () => undefined) } },
      action: { setBadgeText: vi.fn(), setTitle: status },
    });
    try {
      const background = (await import('@/entrypoints/background')).default;
      (background as unknown as () => void)();
      install?.();
      await vi.waitFor(() => expect(create).toHaveBeenCalledTimes(targets.length + 1));
      expect(create).toHaveBeenCalledWith(expect.objectContaining({ id: MENU_PARENT_ID }));
      click?.(
        { menuItemId: 'copied-md-rich-text', frameId: 7, selectionText: 'flattened fallback' },
        { id: 12 },
      );
      await vi.waitFor(() => expect(status).toHaveBeenCalled());
      expect(sendMessage).toHaveBeenCalledWith(12, { type: 'READ_SELECTION' }, { frameId: 7 });
      expect(executeScript).toHaveBeenCalledWith({
        target: { tabId: 12, frameIds: [7] },
        files: ['content-scripts/content.js'],
      });
      expect(sendMessage).toHaveBeenCalledWith(
        12,
        expect.objectContaining({
          type: 'WRITE_CLIPBOARD',
          payload: {
            'text/plain': 'Read more (https://example.com/article).',
            'text/html': '<p>Read <a href="https://example.com/article">more</a>.</p>',
          },
        }),
        { frameId: 7 },
      );
    } finally {
      vi.unstubAllGlobals();
    }
  });
});
