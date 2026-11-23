import { defineStore } from 'pinia';
import { ref } from 'vue';
import type { Platform } from '@0xdoublesharp/copied-md-converters';
import { STORAGE_KEYS } from '@/lib/constants';

const MAX_HISTORY = 50;

export interface HistoryEntry {
  id: string;
  platform: Platform;
  inputPreview: string;
  outputPreview: string;
  timestamp: number;
  truncated: boolean;
}

export const useHistoryStore = defineStore('history', () => {
  const entries = ref<HistoryEntry[]>([]);

  async function load() {
    try {
      const result = await browser.storage.local.get(STORAGE_KEYS.HISTORY);
      entries.value = (result[STORAGE_KEYS.HISTORY] as HistoryEntry[]) ?? [];
    } catch {
      entries.value = [];
    }
  }

  async function addEntry(entry: Omit<HistoryEntry, 'id' | 'timestamp'>) {
    const newEntry: HistoryEntry = {
      ...entry,
      id: crypto.randomUUID(),
      timestamp: Date.now(),
    };
    entries.value.unshift(newEntry);
    if (entries.value.length > MAX_HISTORY) {
      entries.value = entries.value.slice(0, MAX_HISTORY);
    }
    await browser.storage.local.set({ [STORAGE_KEYS.HISTORY]: entries.value });
  }

  async function clear() {
    entries.value = [];
    await browser.storage.local.set({ [STORAGE_KEYS.HISTORY]: [] });
  }

  return { entries, load, addEntry, clear };
});
