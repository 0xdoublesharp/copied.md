import {
  convertMarkdown,
  resolveTarget,
  type CopyTarget,
} from '@0xdoublesharp/copied-md-converters';
import { defineStore } from 'pinia';
import { computed, ref, watch } from 'vue';

export const useUiStore = defineStore('ui', () => {
  const inputMarkdown = ref('');
  const decorativeUnicode = ref(false);
  const selectedPlatform = ref<CopyTarget | null>(null);
  const copied = ref(false);
  let timer: ReturnType<typeof setTimeout> | undefined;

  const conversionResult = computed(() => {
    if (!selectedPlatform.value || !inputMarkdown.value.trim()) return null;
    return convertMarkdown(inputMarkdown.value, selectedPlatform.value, {
      decorativeUnicode: decorativeUnicode.value,
    });
  });
  const previewText = computed(() => conversionResult.value?.representations['text/plain'] ?? '');
  const hasHtml = computed(() => !!conversionResult.value?.representations['text/html']);

  function selectPlatform(platform: string) {
    selectedPlatform.value = resolveTarget(platform) ?? null;
  }

  function markCopied() {
    clearTimeout(timer);
    copied.value = true;
    timer = setTimeout(() => {
      copied.value = false;
    }, 2000);
  }

  function reset() {
    inputMarkdown.value = '';
    decorativeUnicode.value = false;
    selectedPlatform.value = null;
    copied.value = false;
  }

  watch(
    [inputMarkdown, selectedPlatform, decorativeUnicode],
    () => {
      clearTimeout(timer);
      copied.value = false;
    },
    { flush: 'sync' },
  );

  return {
    inputMarkdown,
    decorativeUnicode,
    selectedPlatform,
    copied,
    conversionResult,
    previewText,
    hasHtml,
    selectPlatform,
    markCopied,
    reset,
  };
});
