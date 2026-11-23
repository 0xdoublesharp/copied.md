<script setup lang="ts">
import { ref, watch } from 'vue';
import { browser } from 'wxt/browser';
import { useUiStore } from '@/stores/ui.store';
import { useSettingsStore } from '@/stores/settings.store';
import { Button } from '@/components/ui/button';
import { writeWithFallback } from '@/lib/clipboard-writer';
import { STORAGE_KEYS } from '@/lib/constants';

const ui = useUiStore();
const settings = useSettingsStore();
const status = ref('');
const copyFailed = ref(false);
const pending = ref(false);

watch(
  () => ui.conversionResult,
  () => {
    status.value = '';
    copyFailed.value = false;
  },
);

async function copyToClipboard() {
  const result = ui.conversionResult;
  if (!result || pending.value) return;
  pending.value = true;
  copyFailed.value = false;
  try {
    const outcome = await writeWithFallback(result.representations);
    await browser.storage.local
      .set({ [STORAGE_KEYS.LAST_TARGET]: result.target })
      .catch(() => undefined);
    if (result !== ui.conversionResult) return;
    status.value = outcome.fallback ? 'Copied as plain text.' : 'Copied.';
    ui.markCopied();
    if (
      settings.autoClose &&
      !outcome.fallback &&
      !result.diagnostics.some((item) => item.severity === 'warning')
    )
      window.close();
  } catch {
    status.value = 'Clipboard access failed. Try copying again.';
    copyFailed.value = true;
  } finally {
    pending.value = false;
  }
}
</script>

<template>
  <div v-if="ui.conversionResult" class="mt-3 space-y-2">
    <div class="flex items-center justify-between">
      <h3 class="text-sm font-medium">{{ ui.hasHtml ? 'Plain text preview' : 'Preview' }}</h3>
      <span v-if="ui.hasHtml" class="text-xs text-muted-foreground">Rich text</span>
      <span v-else-if="ui.conversionResult.metrics.limit" class="text-xs text-muted-foreground">
        {{ ui.conversionResult.metrics.outputLength }} / {{ ui.conversionResult.metrics.limit }}
      </span>
    </div>
    <pre
      class="max-h-32 overflow-auto whitespace-pre-wrap break-words rounded-md border bg-muted p-3 text-xs"
      >{{ ui.previewText }}</pre>
    <ul
      v-if="ui.conversionResult.diagnostics.length"
      class="space-y-1 text-xs text-muted-foreground"
    >
      <li v-for="item in ui.conversionResult.diagnostics" :key="item.code">{{ item.message }}</li>
    </ul>
    <p v-if="status" :role="copyFailed ? 'alert' : 'status'" class="text-xs">{{ status }}</p>
    <Button class="w-full" :disabled="pending || !ui.previewText.trim()" @click="copyToClipboard">
      {{ pending ? 'Copying…' : ui.copied ? 'Copied' : 'Copy to Clipboard' }}
    </Button>
  </div>
</template>
