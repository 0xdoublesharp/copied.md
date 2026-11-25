import { describe, it, expect, beforeEach, vi } from 'vitest';
import { setActivePinia, createPinia } from 'pinia';
import { useSettingsStore } from '@/stores/settings.store';
import { Platform } from '@0xdoublesharp/copied-md-converters';
import { visibleFormats } from '@/lib/copy-formats';

// Mock the browser.storage.local API
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

describe('useSettingsStore', () => {
  beforeEach(() => {
    setActivePinia(createPinia());
    // Clear mock storage
    for (const key of Object.keys(mockStorage)) {
      delete mockStorage[key];
    }
    vi.clearAllMocks();
  });

  it('has correct defaults', () => {
    const store = useSettingsStore();
    expect(store.defaultPlatform).toBe('rich-text');
    expect(store.showNotifications).toBe(true);
    expect(store.autoClose).toBe(true);
  });

  it('loads saved settings from storage', async () => {
    mockStorage['copied-md-settings'] = {
      defaultPlatform: Platform.HTML,
      showNotifications: false,
      autoClose: false,
    };

    const store = useSettingsStore();
    await store.load();

    expect(store.defaultPlatform).toBe('rich-text');
    expect(store.showNotifications).toBe(false);
    expect(store.autoClose).toBe(false);
  });

  it('saves settings to storage', async () => {
    const store = useSettingsStore();
    store.defaultPlatform = Platform.Reddit;
    store.showNotifications = false;
    await store.save();

    expect(mockBrowser.storage.local.set).toHaveBeenCalledWith({
      'copied-md-settings': {
        defaultPlatform: Platform.Reddit,
        showNotifications: false,
        autoClose: true,
      },
    });
  });

  it('handles missing storage gracefully', async () => {
    const store = useSettingsStore();
    await store.load();
    // Should keep defaults
    expect(store.defaultPlatform).toBe('rich-text');
  });

  it('keeps the current format when persisted settings are unknown', async () => {
    const store = useSettingsStore();
    store.defaultPlatform = 'telegram';
    mockStorage['copied-md-settings'] = { defaultPlatform: 'obsolete' };
    await store.load();
    expect(store.defaultPlatform).toBe('telegram');
  });

  it('keeps one picker format visible and updates a hidden default', async () => {
    const store = useSettingsStore();
    store.defaultPlatform = 'signal';
    store.setFormatVisible('signal', false);
    expect(store.defaultPlatform).toBe('rich-text');
    for (const { id } of visibleFormats()) {
      if (id !== 'rich-text') store.setFormatVisible(id, false);
    }
    store.setFormatVisible('rich-text', false);
    expect(store.formatVisibility['rich-text']).toBeUndefined();
    expect(store.formats.map(({ id }) => id)).toEqual(['rich-text']);
    await store.save();
    expect(mockStorage['copied-md-settings']).toMatchObject({
      formatVisibility: { signal: false },
    });
  });
});
