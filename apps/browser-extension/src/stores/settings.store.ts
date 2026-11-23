import { resolveTarget, type CopyTarget } from '@0xdoublesharp/copied-md-converters';
import { defineStore } from 'pinia';
import { ref } from 'vue';
import { STORAGE_KEYS } from '@/lib/constants';

interface SettingsState {
  defaultPlatform: string;
  showNotifications: boolean;
  autoClose: boolean;
}

export const useSettingsStore = defineStore('settings', () => {
  const defaultPlatform = ref<CopyTarget>('rich-text');
  const showNotifications = ref(true);
  const autoClose = ref(true);

  async function load() {
    try {
      const result = await browser.storage.local.get(STORAGE_KEYS.SETTINGS);
      const saved = result[STORAGE_KEYS.SETTINGS] as Partial<SettingsState> | undefined;
      if (typeof saved?.defaultPlatform === 'string') {
        defaultPlatform.value = resolveTarget(saved.defaultPlatform) ?? defaultPlatform.value;
      }
      if (typeof saved?.showNotifications === 'boolean')
        showNotifications.value = saved.showNotifications;
      if (typeof saved?.autoClose === 'boolean') autoClose.value = saved.autoClose;
    } catch {
      // Keep current defaults when storage is unavailable.
    }
  }

  async function save() {
    const state: SettingsState = {
      defaultPlatform: defaultPlatform.value,
      showNotifications: showNotifications.value,
      autoClose: autoClose.value,
    };
    await browser.storage.local.set({ [STORAGE_KEYS.SETTINGS]: state });
  }

  return { defaultPlatform, showNotifications, autoClose, load, save };
});
