import { describe, it, expect, beforeEach, vi } from 'vitest';
import { setActivePinia, createPinia } from 'pinia';
import { useHistoryStore } from '@/stores/history.store';
import { Platform } from '@0xdoublesharp/copied-md-converters';

// Mock browser.storage.local
const mockStorage: Record<string, unknown> = {};
const mockBrowser = {
  storage: {
    local: {
      get: vi.fn(async (key: string) => ({ [key]: mockStorage[key] })),
      set: vi.fn(async (items: Record<string, unknown>) => {
        Object.assign(mockStorage, items);
      }),
    },
  },
};

vi.stubGlobal('browser', mockBrowser);

// Mock crypto.randomUUID
vi.stubGlobal('crypto', {
  randomUUID: vi.fn(() => 'test-uuid-' + Math.random().toString(36).slice(2)),
});

describe('useHistoryStore', () => {
  beforeEach(() => {
    setActivePinia(createPinia());
    for (const key of Object.keys(mockStorage)) {
      delete mockStorage[key];
    }
    vi.clearAllMocks();
  });

  it('starts with empty entries', () => {
    const store = useHistoryStore();
    expect(store.entries).toEqual([]);
  });

  it('loads entries from storage', async () => {
    mockStorage['copied-md-history'] = [
      {
        id: '1',
        platform: Platform.Slack,
        inputPreview: '**test**',
        outputPreview: '*test*',
        timestamp: 1000,
        truncated: false,
      },
    ];

    const store = useHistoryStore();
    await store.load();
    expect(store.entries).toHaveLength(1);
    expect(store.entries[0].platform).toBe(Platform.Slack);
  });

  it('clears stale in-memory history when storage cannot be read', async () => {
    const store = useHistoryStore();
    await store.addEntry({
      platform: Platform.Slack,
      inputPreview: 'before',
      outputPreview: 'after',
      truncated: false,
    });
    mockBrowser.storage.local.get.mockRejectedValueOnce(new Error('storage unavailable'));
    await store.load();
    expect(store.entries).toEqual([]);
  });

  it('loads missing history as an empty list', async () => {
    const store = useHistoryStore();
    await store.load();
    expect(store.entries).toEqual([]);
  });

  it('adds entries to the beginning', async () => {
    const store = useHistoryStore();
    await store.addEntry({
      platform: Platform.Slack,
      inputPreview: '**test**',
      outputPreview: '*test*',
      truncated: false,
    });

    expect(store.entries).toHaveLength(1);
    expect(store.entries[0].platform).toBe(Platform.Slack);
    expect(store.entries[0].id).toBeDefined();
    expect(store.entries[0].timestamp).toBeDefined();
  });

  it('limits to 50 entries', async () => {
    const store = useHistoryStore();

    for (let i = 0; i < 55; i++) {
      await store.addEntry({
        platform: Platform.Slack,
        inputPreview: `entry ${i}`,
        outputPreview: `output ${i}`,
        truncated: false,
      });
    }

    expect(store.entries.length).toBeLessThanOrEqual(50);
  });

  it('clears all entries', async () => {
    const store = useHistoryStore();
    await store.addEntry({
      platform: Platform.Slack,
      inputPreview: 'test',
      outputPreview: 'test',
      truncated: false,
    });

    await store.clear();
    expect(store.entries).toEqual([]);
  });
});
