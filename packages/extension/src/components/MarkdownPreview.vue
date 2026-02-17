<script setup lang="ts">
import { useUiStore } from '@/stores/ui.store';
import { Button } from '@/components/ui/button';

const ui = useUiStore();

async function copyToClipboard() {
  const result = ui.conversionResult;
  if (!result) return;

  try {
    if (result.payload['text/html'] && typeof ClipboardItem !== 'undefined') {
      const item = new ClipboardItem({
        'text/plain': new Blob([result.payload['text/plain']], { type: 'text/plain' }),
        'text/html': new Blob([result.payload['text/html']], { type: 'text/html' }),
      });
      await navigator.clipboard.write([item]);
    } else {
      await navigator.clipboard.writeText(result.payload['text/plain']);
    }
    ui.markCopied();
  } catch {
    // Fallback
    const textarea = document.createElement('textarea');
    textarea.value = result.payload['text/plain'];
    textarea.style.position = 'fixed';
    textarea.style.opacity = '0';
    document.body.appendChild(textarea);
    textarea.select();
    document.execCommand('copy');
    document.body.removeChild(textarea);
    ui.markCopied();
  }
}
</script>

<template>
  <div v-if="ui.selectedPlatform && ui.inputMarkdown.trim()" class="mt-3 space-y-2">
    <div class="flex items-center justify-between">
      <h3 class="text-sm font-medium">Preview</h3>
      <div class="flex items-center gap-2">
        <span v-if="ui.isTruncated" class="text-xs text-destructive"> Truncated </span>
        <span v-if="ui.hasHtml" class="text-xs text-muted-foreground"> Rich text </span>
      </div>
    </div>
    <pre
      class="rounded-md border bg-muted p-3 text-xs max-h-32 overflow-auto whitespace-pre-wrap break-words"
      >{{ ui.previewText }}</pre
    >
    <Button class="w-full" :variant="ui.copied ? 'secondary' : 'default'" @click="copyToClipboard">
      {{ ui.copied ? 'Copied!' : 'Copy to Clipboard' }}
    </Button>
  </div>
</template>
