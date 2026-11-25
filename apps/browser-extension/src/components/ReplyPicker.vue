<script setup lang="ts">
import { onMounted } from 'vue';
import PlatformSelector from '@/components/PlatformSelector.vue';
import MarkdownPreview from '@/components/MarkdownPreview.vue';
import { useSettingsStore } from '@/stores/settings.store';
import { useUiStore } from '@/stores/ui.store';

const props = defineProps<{ markdown: string; sourceUrl: string }>();
const emit = defineEmits<{ close: [] }>();
const settings = useSettingsStore();
const ui = useUiStore();

onMounted(async () => {
  await settings.load();
  ui.inputMarkdown = props.markdown;
  ui.sourceUrl = props.sourceUrl;
  ui.selectPlatform(settings.defaultPlatform);
});
</script>

<template>
  <section
    class="my-2 w-full max-w-[420px] rounded-md border bg-background p-3 text-foreground"
    aria-label="Copy reply as"
    @keydown.esc.stop="emit('close')"
  >
    <div class="mb-2 flex items-center justify-between gap-2 text-sm">
      <h2 class="font-medium">Copy reply as</h2>
      <button class="rounded px-2 py-1 hover:bg-accent" type="button" @click="emit('close')">
        Close
      </button>
    </div>
    <PlatformSelector />
    <MarkdownPreview embedded />
  </section>
</template>
