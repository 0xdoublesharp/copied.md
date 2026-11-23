<script setup lang="ts">
import { onMounted, watch } from 'vue';
import { targets } from '@0xdoublesharp/copied-md-converters';
import { useSettingsStore } from '@/stores/settings.store';

const settings = useSettingsStore();

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
      <label class="text-sm font-medium">Default format</label>
      <select
        v-model="settings.defaultPlatform"
        class="w-full rounded-md border border-input bg-background px-3 py-2 text-sm"
      >
        <option v-for="p in targets" :key="p.id" :value="p.id">
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
