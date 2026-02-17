<script setup lang="ts">
import { onMounted, watch } from 'vue';
import { Platform } from '@copied-md/converters';
import { useSettingsStore } from '@/stores/settings.store';

const settings = useSettingsStore();

const platforms = [
  { id: Platform.Facebook, label: 'Facebook' },
  { id: Platform.Twitter, label: 'Twitter/X' },
  { id: Platform.Slack, label: 'Slack' },
  { id: Platform.Discord, label: 'Discord' },
  { id: Platform.Telegram, label: 'Telegram' },
  { id: Platform.SMS, label: 'SMS' },
  { id: Platform.Reddit, label: 'Reddit' },
  { id: Platform.HTML, label: 'HTML' },
];

onMounted(() => {
  settings.load();
});

// Auto-save on changes
watch(
  () => ({
    defaultPlatform: settings.defaultPlatform,
    showNotifications: settings.showNotifications,
    autoClose: settings.autoClose,
  }),
  () => {
    settings.save();
  },
  { deep: true },
);
</script>

<template>
  <div class="space-y-4 py-2">
    <div class="space-y-2">
      <label class="text-sm font-medium">Default Platform</label>
      <select
        v-model="settings.defaultPlatform"
        class="w-full rounded-md border border-input bg-background px-3 py-2 text-sm"
      >
        <option v-for="p in platforms" :key="p.id" :value="p.id">
          {{ p.label }}
        </option>
      </select>
    </div>

    <div class="flex items-center justify-between">
      <label class="text-sm">Show notifications</label>
      <input v-model="settings.showNotifications" type="checkbox" class="rounded" />
    </div>

    <div class="flex items-center justify-between">
      <label class="text-sm">Auto-close after copy</label>
      <input v-model="settings.autoClose" type="checkbox" class="rounded" />
    </div>
  </div>
</template>
