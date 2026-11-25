import { afterEach, expect, it, vi } from 'vitest';
import { registerChatAccess } from '@/lib/chat-access';
import { STORAGE_KEYS } from '@/lib/constants';

afterEach(() => vi.unstubAllGlobals());

it('registers only granted origins and updates already-open tabs, then unregisters on revoke', async () => {
  let sync!: () => void;
  const registerContentScripts = vi.fn(async () => undefined);
  const unregisterContentScripts = vi.fn(async () => undefined);
  const getRegisteredContentScripts = vi.fn(async (): Promise<{ id: string }[]> => []);
  const executeScript = vi.fn(async () => undefined);
  const contains = vi.fn(async ({ origins }: { origins: string[] }) =>
    origins.includes('https://chatgpt.com/*'),
  );
  const query = vi.fn(async () => [{ id: 5 }, {}]);
  const set = vi.fn(async () => undefined);
  const hook = {
    addListener: (listener: () => void) => {
      sync = listener;
    },
  };
  vi.stubGlobal('browser', {
    permissions: { contains, onAdded: hook, onRemoved: hook },
    runtime: { onInstalled: hook, onStartup: hook },
    scripting: {
      registerContentScripts,
      unregisterContentScripts,
      getRegisteredContentScripts,
      executeScript,
    },
    tabs: { query },
    storage: { local: { set } },
  });
  registerChatAccess();

  sync();
  await vi.waitFor(() =>
    expect(executeScript).toHaveBeenCalledWith({
      target: { tabId: 5 },
      files: ['content-scripts/chat-replies.js'],
    }),
  );
  expect(registerContentScripts).toHaveBeenCalledWith([
    {
      id: 'copied-md-chatgpt',
      matches: ['https://chatgpt.com/*'],
      js: ['content-scripts/chat-replies.js'],
      runAt: 'document_idle',
    },
  ]);
  expect(set).toHaveBeenLastCalledWith({ [STORAGE_KEYS.CHAT_SITES]: ['chatgpt'] });
  expect(query).toHaveBeenLastCalledWith({ url: ['https://chatgpt.com/*'] });

  getRegisteredContentScripts.mockResolvedValue([{ id: 'copied-md-chatgpt' }]);
  sync();
  await vi.waitFor(() => expect(set).toHaveBeenCalledTimes(2));
  expect(registerContentScripts).toHaveBeenCalledOnce();
  expect(executeScript).toHaveBeenCalledOnce();

  contains.mockResolvedValue(false);
  getRegisteredContentScripts.mockRejectedValueOnce(new Error('startup unavailable'));
  sync();
  sync();
  await vi.waitFor(() => expect(set).toHaveBeenLastCalledWith({ [STORAGE_KEYS.CHAT_SITES]: [] }));
  expect(unregisterContentScripts).toHaveBeenCalledWith({ ids: ['copied-md-chatgpt'] });
});
