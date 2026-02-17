import { defineStore } from 'pinia';
import { ref } from 'vue';
import type { Platform } from '@copied-md/converters';

const HISTORY_KEY = 'copied-md-history';
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
      const result = await browser.storage.local.get(HISTORY_KEY);
      entries.value = (result[HISTORY_KEY] as HistoryEntry[]) ?? [];
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
    await browser.storage.local.set({ [HISTORY_KEY]: entries.value });
  }

  async function clear() {
    entries.value = [];
    await browser.storage.local.set({ [HISTORY_KEY]: [] });
  }

  return { entries, load, addEntry, clear };
});
