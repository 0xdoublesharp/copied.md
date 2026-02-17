import { defineStore } from 'pinia';
import { ref } from 'vue';
import { Platform } from '@copied-md/converters';

const SETTINGS_KEY = 'copied-md-settings';

interface SettingsState {
  defaultPlatform: Platform;
  showNotifications: boolean;
  autoClose: boolean;
}

const DEFAULTS: SettingsState = {
  defaultPlatform: Platform.Slack,
  showNotifications: true,
  autoClose: true,
};

export const useSettingsStore = defineStore('settings', () => {
  const defaultPlatform = ref<Platform>(DEFAULTS.defaultPlatform);
  const showNotifications = ref(DEFAULTS.showNotifications);
  const autoClose = ref(DEFAULTS.autoClose);

  async function load() {
    try {
      const result = await browser.storage.local.get(SETTINGS_KEY);
      const saved = result[SETTINGS_KEY] as SettingsState | undefined;
      if (saved) {
        defaultPlatform.value = saved.defaultPlatform;
        showNotifications.value = saved.showNotifications;
        autoClose.value = saved.autoClose;
      }
    } catch {
      // Use defaults
    }
  }

  async function save() {
    const state: SettingsState = {
      defaultPlatform: defaultPlatform.value,
      showNotifications: showNotifications.value,
      autoClose: autoClose.value,
    };
    await browser.storage.local.set({ [SETTINGS_KEY]: state });
  }

  return {
    defaultPlatform,
    showNotifications,
    autoClose,
    load,
    save,
  };
});
