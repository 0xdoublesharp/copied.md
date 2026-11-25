<script setup lang="ts">
import { computed } from 'vue';
import type { CopyTarget, MarkdownConversion } from '../lib/ipc';

const props = defineProps<{
  label: string;
  result?: MarkdownConversion;
  failed: boolean;
}>();

const markupTargets = new Set<CopyTarget>([
  'discord',
  'reddit',
  'slack-api',
  'html-source',
  'whatsapp',
  'twitter',
]);
const warnings = computed(() =>
  props.result?.diagnostics.filter((diagnostic) => diagnostic.severity === 'warning'),
);
const characterCount = computed(() => {
  const metrics = props.result?.metrics;
  if (!metrics) return '';
  return metrics.limit != null
    ? `${metrics.outputLength} / ${metrics.limit}`
    : `${metrics.outputLength} chars`;
});
const overLimit = computed(() => {
  const metrics = props.result?.metrics;
  return metrics?.limit != null && metrics.outputLength > metrics.limit;
});
</script>

<template>
  <div class="preview-output">
    <div class="preview-meta">
      <span class="preview-label">{{ label }}</span>
      <template v-if="result">
        <span class="metric" :class="{ 'over-limit': overLimit }">
          {{ characterCount }}
        </span>
        <span
          v-for="(warning, index) in warnings"
          :key="index"
          class="preview-warning"
          :title="warning.message"
        >
          {{ warning.message }}
        </span>
      </template>
    </div>
    <pre
      v-if="result"
      class="preview-text"
      :class="{ 'preview-markup': markupTargets.has(result.target) }"
      tabindex="0"
      :aria-label="`${label} preview`"
      >{{ result.representations['text/plain'] }}</pre>
    <p v-else class="preview-placeholder" role="status">
      {{ failed ? 'Preview unavailable.' : 'Preparing preview…' }}
    </p>
  </div>
</template>
