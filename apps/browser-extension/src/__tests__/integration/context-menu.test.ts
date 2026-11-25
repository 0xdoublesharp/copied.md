import { describe, it, expect, vi } from 'vitest';
import { MENU_PARENT_ID, STORAGE_KEYS } from '@/lib/constants';
import { visibleFormats } from '@/lib/copy-formats';

type Click = (
  info: { menuItemId: string; frameId: number; selectionText: string },
  tab: { id: number },
) => void;

const events = {
  permissions: { onAdded: { addListener: vi.fn() }, onRemoved: { addListener: vi.fn() } },
  startup: { addListener: vi.fn() },
  storageChanged: { addListener: vi.fn() },
};

describe('Browser selection copy', () => {
  it('registers all targets and converts the live selection from its frame', async () => {
    const install: (() => void)[] = [];
    let click: Click | undefined;
    const create = vi.fn();
    const status = vi.fn();
    const get = vi.fn(async (): Promise<Record<string, unknown>> => ({}));
    const removeAll = vi.fn(async () => undefined);
    get.mockRejectedValueOnce(new Error('Storage unavailable'));
    removeAll.mockRejectedValueOnce(new Error('Menus busy'));
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
      permissions: events.permissions,
      runtime: {
        getManifest: () => ({ manifest_version: 3 }),
        onStartup: events.startup,
        onInstalled: {
          addListener: (callback: () => void) => {
            install.push(callback);
          },
        },
      },
      commands: { onCommand: { addListener: vi.fn() } },
      contextMenus: {
        removeAll,
        create,
        onClicked: {
          addListener: (callback: Click) => {
            click = callback;
          },
        },
      },
      tabs: { sendMessage },
      storage: {
        local: { get, set: vi.fn(async () => undefined) },
        onChanged: events.storageChanged,
      },
      action: { setBadgeText: vi.fn(), setTitle: status },
    });
    try {
      const background = (await import('@/entrypoints/background')).default;
      (background as unknown as () => void)();
      install.forEach((callback) => callback());
      await vi.waitFor(() => expect(removeAll).toHaveBeenCalledOnce());
      expect(create).not.toHaveBeenCalled();
      install.forEach((callback) => callback());
      await vi.waitFor(() => expect(create).toHaveBeenCalledTimes(visibleFormats().length + 1));
      expect(create).toHaveBeenCalledWith(expect.objectContaining({ id: MENU_PARENT_ID }));
      expect(create).not.toHaveBeenCalledWith(expect.objectContaining({ id: 'copied-md-teams' }));
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
      create.mockClear();
      const changed = events.storageChanged.addListener.mock.lastCall![0];
      changed({}, 'local');
      changed({ [STORAGE_KEYS.SETTINGS]: {} }, 'sync');
      expect(create).not.toHaveBeenCalled();
      get.mockResolvedValue({
        [STORAGE_KEYS.SETTINGS]: { formatVisibility: { signal: false, teams: true } },
      });
      changed({ [STORAGE_KEYS.SETTINGS]: {} }, 'local');
      await vi.waitFor(() =>
        expect(create).toHaveBeenCalledWith(expect.objectContaining({ id: 'copied-md-teams' })),
      );
      expect(create).not.toHaveBeenCalledWith(expect.objectContaining({ id: 'copied-md-signal' }));
    } finally {
      vi.unstubAllGlobals();
    }
  });

  async function start(manifest = 3) {
    vi.resetModules();
    let click!: Click;
    let shortcut!: (command: string) => Promise<void>;
    const status = vi.fn();
    const send = vi.fn(async (_tab, message): Promise<unknown> => {
      if (message.type === 'READ_SELECTION') return { text: '**Ready**' };
      if (message.type === 'WRITE_CLIPBOARD') return { types: ['text/plain'] };
      return undefined;
    });
    const get = vi.fn(async (): Promise<Record<string, unknown>> => ({}));
    const set = vi.fn(async () => undefined);
    const query = vi.fn(async () => [{ id: 10 }]);
    const inject = vi.fn();
    vi.stubGlobal('defineBackground', (callback: () => void) => callback);
    vi.stubGlobal('browser', {
      permissions: events.permissions,
      runtime: {
        getManifest: () => ({ manifest_version: manifest }),
        onInstalled: { addListener: vi.fn() },
        onStartup: events.startup,
      },
      commands: {
        onCommand: {
          addListener: (callback: typeof shortcut) => {
            shortcut = callback;
          },
        },
      },
      contextMenus: {
        onClicked: {
          addListener: (callback: Click) => {
            click = callback;
          },
        },
      },
      tabs: { sendMessage: send, executeScript: inject, query },
      storage: { local: { get, set }, onChanged: events.storageChanged },
      browserAction: { setBadgeText: vi.fn(), setTitle: status },
    });
    const background = (await import('@/entrypoints/background')).default;
    (background as unknown as () => void)();
    return { click, shortcut, send, status, get, set, query, inject };
  }

  it('does not use stale context-menu text for protected selections', async () => {
    try {
      const { click, send, status } = await start();
      send.mockResolvedValueOnce({ text: '', blocked: true });
      click({ menuItemId: 'unrelated', frameId: 0, selectionText: '' }, { id: 10 });
      click({ menuItemId: 'copied-md-telegram', frameId: 3, selectionText: 'private' }, { id: 10 });
      await vi.waitFor(() =>
        expect(status).toHaveBeenCalledWith({ title: 'copied.md · Select Markdown to copy.' }),
      );
      expect(send.mock.calls.filter(([, msg]) => msg.type === 'READ_SELECTION')).toHaveLength(1);
      expect(send.mock.calls.some(([, msg]) => msg.type === 'WRITE_CLIPBOARD')).toBe(false);
      send.mockResolvedValueOnce({ text: '[x]: https://example.org' });
      click({ menuItemId: 'copied-md-plain-text', frameId: 1, selectionText: '' }, { id: 10 });
      await vi.waitFor(() =>
        expect(status).toHaveBeenLastCalledWith({ title: 'copied.md · There is no text to copy.' }),
      );
    } finally {
      vi.unstubAllGlobals();
    }
  });

  it('uses the Firefox injection API and recovers from failed frame access', async () => {
    try {
      const { click, send, status, inject, set, get } = await start(2);
      send.mockRejectedValueOnce(new Error('no listener'));
      inject.mockRejectedValueOnce(new Error('restricted page'));
      get.mockRejectedValueOnce(new Error('storage unavailable'));
      set.mockRejectedValueOnce(new Error('storage unavailable'));
      click(
        { menuItemId: 'copied-md-plain-text', frameId: 4, selectionText: 'fallback' },
        { id: 10 },
      );
      await vi.waitFor(() =>
        expect(status).toHaveBeenCalledWith({ title: 'copied.md · Copied as Plain Text.' }),
      );
      expect(inject).toHaveBeenCalledWith(10, { file: '/content-scripts/content.js', frameId: 4 });
      expect(send).toHaveBeenCalledWith(
        10,
        expect.objectContaining({ type: 'WRITE_CLIPBOARD', payload: { 'text/plain': 'fallback' } }),
        { frameId: 4 },
      );
    } finally {
      vi.unstubAllGlobals();
    }
  });

  it('uses saved shortcuts, handles failures and serializes subsequent copy requests', async () => {
    try {
      const { shortcut, get, query, send, status, set } = await start();
      await shortcut('other-command');
      expect(query).not.toHaveBeenCalled();
      query.mockRejectedValueOnce(new Error('tabs unavailable'));
      await shortcut('copy-last-target');
      expect(status).toHaveBeenLastCalledWith({ title: expect.stringContaining('Copy failed.') });
      get
        .mockResolvedValueOnce({ [STORAGE_KEYS.LAST_TARGET]: 'plain-text' })
        .mockResolvedValueOnce({ [STORAGE_KEYS.SETTINGS]: { showNotifications: false } });
      await shortcut('copy-last-target');
      await vi.waitFor(() =>
        expect(set).toHaveBeenCalledWith({ [STORAGE_KEYS.LAST_TARGET]: 'plain-text' }),
      );
      expect(send.mock.calls.some(([, msg]) => msg.type === 'COPY_STATUS')).toBe(false);

      get.mockResolvedValueOnce({ [STORAGE_KEYS.SETTINGS]: { defaultPlatform: 'telegram' } });
      await shortcut('copy-last-target');
      await vi.waitFor(() =>
        expect(status).toHaveBeenLastCalledWith({ title: 'copied.md · Copied as plain text.' }),
      );

      get.mockResolvedValueOnce({ [STORAGE_KEYS.LAST_TARGET]: 42 });
      send.mockImplementation(async (_tab, msg) => {
        if (msg.type === 'READ_SELECTION') return { text: '[Note](./notes)' };
        if (msg.type === 'WRITE_CLIPBOARD') throw new Error('clipboard unavailable');
        return undefined;
      });
      await shortcut('copy-last-target');
      await vi.waitFor(() =>
        expect(status).toHaveBeenLastCalledWith({ title: expect.stringContaining('Copy failed.') }),
      );
      send.mockImplementation(async (_tab, msg) => {
        if (msg.type === 'READ_SELECTION') return { text: '[Note](./notes)' };
        if (msg.type === 'WRITE_CLIPBOARD') return { types: ['text/plain', 'text/html'] };
        return undefined;
      });
      await shortcut('copy-last-target');
      await vi.waitFor(() =>
        expect(status).toHaveBeenLastCalledWith({
          title: expect.stringContaining('A relative link needs an original page URL.'),
        }),
      );
    } finally {
      vi.unstubAllGlobals();
    }
  });

  it('reports an empty active window without attempting frame messaging', async () => {
    try {
      const { shortcut, query, send, status } = await start();
      query.mockResolvedValueOnce([]);
      await shortcut('copy-last-target');
      await vi.waitFor(() => {
        expect(status).toHaveBeenLastCalledWith({
          title: 'copied.md · Select Markdown to copy.',
        });
      });
      expect(send).not.toHaveBeenCalled();
    } finally {
      vi.unstubAllGlobals();
    }
  });
});
