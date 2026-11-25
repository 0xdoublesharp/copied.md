<script setup lang="ts">
import { computed } from 'vue';
import type { CopyTarget, MarkdownConversion } from '../lib/ipc';
import { formatDiagnostic } from '../lib/diagnostics';

const props = defineProps<{
  label: string;
  result?: MarkdownConversion;
  failed: boolean;
}>();

const markupTargets = new Set<CopyTarget>([
  'discord',
  'telegram',
  'reddit',
  'slack-api',
  'html-source',
  'whatsapp',
  'twitter',
]);
const warnings = computed(() =>
  props.result?.diagnostics
    .filter((diagnostic) => diagnostic.severity === 'warning')
    .map((diagnostic) => formatDiagnostic(diagnostic.message, diagnostic.code)),
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
          class="preview-note"
          :title="warning.tooltip"
        >
          <svg viewBox="0 0 16 16" width="11" height="11" fill="none" aria-hidden="true">
            <circle cx="8" cy="8" r="6.25" stroke="currentColor" stroke-width="1.4" />
            <path
              d="M8 7.2v3.6M8 5.2h.01"
              stroke="currentColor"
              stroke-width="1.5"
              stroke-linecap="round"
            />
          </svg>
          <span class="preview-note-text">{{ warning.label }}</span>
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
