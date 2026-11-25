<script setup lang="ts">
import { ref, onMounted, watch } from 'vue';
import PlatformSelector from '@/components/PlatformSelector.vue';
import MarkdownPreview from '@/components/MarkdownPreview.vue';
import SettingsPanel from '@/components/SettingsPanel.vue';
import { useUiStore } from '@/stores/ui.store';
import { useSettingsStore } from '@/stores/settings.store';

const ui = useUiStore();
const settings = useSettingsStore();
const activeTab = ref<'convert' | 'settings'>('convert');

onMounted(async () => {
  await settings.load();
  ui.selectPlatform(settings.defaultPlatform);
});

watch(
  () => settings.formats,
  (formats) => {
    if (!formats.some(({ id }) => id === ui.selectedPlatform))
      ui.selectPlatform(settings.defaultPlatform);
  },
);
</script>

<template>
  <div class="w-[400px] min-h-[300px] p-4">
    <div class="flex items-center justify-between mb-3">
      <h1 class="text-lg font-bold">copied.md</h1>
      <div class="flex gap-1 text-xs">
        <button
          :class="[
            'px-3 py-1 rounded-md transition-colors',
            activeTab === 'convert'
              ? 'bg-primary text-primary-foreground'
              : 'text-muted-foreground hover:text-foreground',
          ]"
          @click="activeTab = 'convert'"
        >
          Convert
        </button>
        <button
          :class="[
            'px-3 py-1 rounded-md transition-colors',
            activeTab === 'settings'
              ? 'bg-primary text-primary-foreground'
              : 'text-muted-foreground hover:text-foreground',
          ]"
          @click="activeTab = 'settings'"
        >
          Settings
        </button>
      </div>
    </div>

    <div v-if="activeTab === 'convert'">
      <textarea
        v-model="ui.inputMarkdown"
        class="w-full h-24 rounded-md border border-input bg-background px-3 py-2 text-sm placeholder:text-muted-foreground focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring resize-none mb-3"
        placeholder="Paste markdown here..."
      />
      <PlatformSelector />
      <label
        v-if="
          ui.selectedPlatform && ['facebook', 'twitter', 'telegram'].includes(ui.selectedPlatform)
        "
        class="mt-3 flex items-center gap-2 text-xs"
      >
        <input v-model="ui.decorativeUnicode" type="checkbox" />
        Use decorative Unicode in plain text
      </label>
      <MarkdownPreview />
    </div>

    <div v-else>
      <SettingsPanel />
    </div>
  </div>
</template>
