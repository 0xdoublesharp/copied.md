import type { CopyTarget } from '@0xdoublesharp/copied-md-converters';
import { defineStore } from 'pinia';
import { computed, ref } from 'vue';
import { STORAGE_KEYS } from '@/lib/constants';
import { preferredFormat, visibleFormats, type FormatVisibility } from '@/lib/copy-formats';

export interface SettingsState {
  defaultPlatform: string;
  showNotifications: boolean;
  autoClose: boolean;
  formatVisibility?: FormatVisibility;
}

export const useSettingsStore = defineStore('settings', () => {
  const defaultPlatform = ref<CopyTarget>('rich-text');
  const showNotifications = ref(true);
  const autoClose = ref(true);
  const formatVisibility = ref<FormatVisibility>({});
  const formats = computed(() => visibleFormats(formatVisibility.value));

  async function load() {
    try {
      const result = await browser.storage.local.get(STORAGE_KEYS.SETTINGS);
      const saved = result[STORAGE_KEYS.SETTINGS] as Partial<SettingsState> | undefined;
      if (saved?.formatVisibility) formatVisibility.value = saved.formatVisibility;
      defaultPlatform.value = preferredFormat(
        formatVisibility.value,
        saved?.defaultPlatform,
        defaultPlatform.value,
      );
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
      ...(Object.keys(formatVisibility.value).length
        ? { formatVisibility: { ...formatVisibility.value } }
        : {}),
    };
    await browser.storage.local.set({ [STORAGE_KEYS.SETTINGS]: state });
  }

  function setFormatVisible(target: CopyTarget, visible: boolean) {
    if (!visible && formats.value.length === 1 && formats.value[0].id === target) return;
    formatVisibility.value = { ...formatVisibility.value, [target]: visible };
    defaultPlatform.value = preferredFormat(formatVisibility.value, defaultPlatform.value);
  }

  return {
    defaultPlatform,
    showNotifications,
    autoClose,
    formatVisibility,
    formats,
    setFormatVisible,
    load,
    save,
  };
});
