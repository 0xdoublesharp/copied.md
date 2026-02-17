import { defineStore } from 'pinia';
import { ref, computed } from 'vue';
import { Platform, getConverter } from '@copied-md/converters';
import type { ConversionResult } from '@copied-md/converters';

export const useUiStore = defineStore('ui', () => {
  const inputMarkdown = ref('');
  const selectedPlatform = ref<Platform | null>(null);
  const copied = ref(false);

  const conversionResult = computed<ConversionResult | null>(() => {
    if (!selectedPlatform.value || !inputMarkdown.value.trim()) return null;
    try {
      const converter = getConverter(selectedPlatform.value);
      return converter.convert(inputMarkdown.value);
    } catch {
      return null;
    }
  });

  const previewText = computed(() => conversionResult.value?.payload['text/plain'] ?? '');
  const isTruncated = computed(() => conversionResult.value?.truncated ?? false);
  const hasHtml = computed(() => !!conversionResult.value?.payload['text/html']);

  function selectPlatform(platform: Platform) {
    selectedPlatform.value = platform;
    copied.value = false;
  }

  function markCopied() {
    copied.value = true;
    setTimeout(() => {
      copied.value = false;
    }, 2000);
  }

  function reset() {
    inputMarkdown.value = '';
    selectedPlatform.value = null;
    copied.value = false;
  }

  return {
    inputMarkdown,
    selectedPlatform,
    copied,
    conversionResult,
    previewText,
    isTruncated,
    hasHtml,
    selectPlatform,
    markCopied,
    reset,
  };
});
