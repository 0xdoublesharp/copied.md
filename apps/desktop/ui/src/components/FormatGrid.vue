<script setup lang="ts">
/* global HTMLElement, HTMLButtonElement, HTMLInputElement, KeyboardEvent, Node */
import { computed, nextTick, ref, watch } from 'vue';
import { formatIcons } from '@0xdoublesharp/copied-md-shared-icons';
import PreviewOutput from './PreviewOutput.vue';
import type {
  Columns,
  ConfigV2,
  CopyTarget,
  DesktopApi,
  DesktopState,
  PickerOpenRequest,
  TargetDefinition,
} from '../lib/ipc';
import { useMarkdownPreview } from '../lib/preview';
import { formatDiagnostic } from '../lib/diagnostics';

const props = defineProps<{
  request: PickerOpenRequest;
  api: DesktopApi;
  targets: TargetDefinition[];
  columns: Columns;
  activeApp: DesktopState['activeApp'];
  pinnedFormats: ConfigV2['pinnedFormats'];
  autoDetectAppFormat: boolean;
  busy?: boolean;
}>();

const emit = defineEmits<{
  choose: [target: CopyTarget | null];
  dismiss: [];
  settings: [];
}>();

const grid = ref<HTMLElement>();
const filterInput = ref<HTMLInputElement>();
const query = ref('');
const selected = ref<CopyTarget>();
const hovered = ref<CopyTarget>();
const copied = computed(() => props.request.copied);
const aliases: Partial<Record<CopyTarget, string[]>> = {
  sms: ['msg', 'imessage'],
  whatsapp: ['wa'],
  telegram: ['tg'],
  'plain-text': ['md', 'plain'],
};
const filteredTargets = computed(() => {
  const filter = query.value.trim().toLowerCase();
  if (!filter) return props.targets;
  return props.targets.filter(
    ({ id, label }) =>
      id.includes(filter) || label.toLowerCase().includes(filter) || aliases[id]?.includes(filter),
  );
});
const selectedIndex = computed(() =>
  filteredTargets.value.findIndex((target) => target.id === selected.value),
);
const selectedTarget = computed(() => filteredTargets.value[selectedIndex.value]);
const previewTarget = computed(
  () => filteredTargets.value.find((target) => target.id === hovered.value) ?? selectedTarget.value,
);
const isDone = computed(() => copied.value != null && selected.value === copied.value.target);

// The picker supplies a suggestion for resolved app formats. When it already copied the
// result, the copied target can also be the last-used fallback, so check the app ID.
const detectedApps = [
  ['slack-paste', ['com.tinyspeck.slackmacgap', 'slack.exe', 'slack']],
  [
    'discord',
    ['com.hnc.discord', 'com.hnc.discordptb', 'com.hnc.discordcanary', 'discord.exe', 'discord'],
  ],
  [
    'telegram',
    [
      'ru.keepcoder.telegram',
      'org.telegram.desktop',
      'telegram.exe',
      'telegram-desktop',
      'telegramdesktop',
    ],
  ],
  ['signal', ['org.whispersystems.signal-desktop', 'signal.exe', 'signal']],
  [
    'teams',
    ['com.microsoft.teams2', 'com.microsoft.teams', 'ms-teams.exe', 'teams.exe', 'teams-for-linux'],
  ],
  ['word', ['com.microsoft.word', 'winword.exe']],
  ['whatsapp', ['net.whatsapp.whatsapp', 'whatsapp.exe', 'whatsapp']],
  ['sms', ['com.apple.mobilesms']],
] satisfies [CopyTarget, string[]][];
const appContext = computed(() => {
  if (!props.activeApp) return '';
  const { id, name } = props.activeApp;
  const context = `Active: ${name}`;
  const suggested = props.request.suggested;
  const resolved = suggested ?? copied.value?.target;
  if (!resolved) return context;
  if (resolved === props.pinnedFormats[id]) return `${context} · Pinned`;
  const appId = id.toLowerCase().split(':').at(-1) ?? '';
  const detected = detectedApps.find(([, ids]) => ids.includes(appId))?.[0];
  if (props.autoDetectAppFormat && (suggested || detected === resolved)) {
    return `${context} · Auto-detected`;
  }
  return context;
});

const previewKey = 'copied.md.picker.preview-collapsed';
const previewExpanded = ref(true);
try {
  previewExpanded.value = window.localStorage.getItem(previewKey) !== 'true';
} catch {
  // The picker still works when the webview does not allow storage.
}
const { result: preview, failed: previewFailed } = useMarkdownPreview(
  props.api,
  () => props.request.sourcePreview,
  () => previewTarget.value?.id,
  () => props.request.opId,
);
const copiedNotice = computed(() => {
  if (!copied.value?.warning) return null;
  const formatted = formatDiagnostic(copied.value.warning);
  const shownInPreview =
    props.request.sourcePreview != null &&
    previewExpanded.value &&
    preview.value?.diagnostics.some(
      (diagnostic) =>
        diagnostic.severity === 'warning' &&
        formatDiagnostic(diagnostic.message, diagnostic.code).label === formatted.label,
    );
  return shownInPreview ? null : formatted;
});

function togglePreview() {
  previewExpanded.value = !previewExpanded.value;
  try {
    window.localStorage.setItem(previewKey, String(!previewExpanded.value));
  } catch {
    // Keep the preference for the current picker even if it cannot be stored.
  }
}

function focusSelection() {
  void nextTick(() => {
    const button = grid.value?.querySelector<HTMLButtonElement>('button[tabindex="0"]');
    (button ?? grid.value)?.focus();
  });
}

function resetFilter() {
  query.value = '';
  hovered.value = undefined;
  selected.value =
    props.targets.find((target) => target.id === props.request.selected)?.id ??
    props.targets[0]?.id;
  focusSelection();
}

watch(() => props.request.opId, resetFilter, { immediate: true });
watch(filteredTargets, (targets) => {
  if (!targets.some((target) => target.id === selected.value)) selected.value = targets[0]?.id;
});

function chooseCard(target: CopyTarget) {
  if (!props.busy) emit('choose', copied.value?.target === target ? null : target);
}

function chooseSelected() {
  if (selectedTarget.value) chooseCard(selectedTarget.value.id);
}

function handleKey(event: KeyboardEvent) {
  if (event.defaultPrevented || event.altKey || event.isComposing) return;
  const modified = event.ctrlKey || event.metaKey;
  const card = /^[1-9]$/.test(event.key) ? filteredTargets.value[Number(event.key) - 1] : undefined;
  if (card && !event.shiftKey && (modified || !query.value)) {
    event.preventDefault();
    if (!event.repeat) chooseCard(card.id);
    return;
  }
  if (modified && !event.shiftKey && event.key.toLowerCase() === 'k') {
    event.preventDefault();
    filterInput.value?.focus();
    return;
  }
  if (modified) return;

  if (event.key === 'Escape') {
    event.preventDefault();
    if (query.value) resetFilter();
    else if (!props.busy && !event.repeat) emit('dismiss');
    return;
  }

  const inFilter = event.target === filterInput.value;
  const inGrid = grid.value?.contains(event.target as Node);
  if (event.key === 'Enter' && (inFilter || inGrid)) {
    event.preventDefault();
    chooseSelected();
    return;
  }
  if (!inFilter && event.key.length === 1 && event.key !== ' ') {
    event.preventDefault();
    query.value += event.key;
    filterInput.value?.focus();
    return;
  }
  if (inFilter && (event.key === 'ArrowUp' || event.key === 'ArrowDown') && selectedTarget.value) {
    event.preventDefault();
    focusSelection();
    return;
  }
  if (!inGrid || event.shiftKey || props.busy) return;

  const current = Math.max(0, selectedIndex.value);
  const last = filteredTargets.value.length - 1;
  const nextIndices: Record<string, number> = {
    ArrowLeft: current - 1,
    ArrowRight: current + 1,
    ArrowUp: current - props.columns,
    ArrowDown: current + props.columns,
    Home: 0,
    End: last,
  };
  if (!Object.hasOwn(nextIndices, event.key) || last < 0) return;
  event.preventDefault();
  const next = Math.max(0, Math.min(last, nextIndices[event.key]));
  selected.value = filteredTargets.value[next].id;
  focusSelection();
}
</script>

<template>
  <section class="picker-panel" aria-label="Copy Markdown As" @keydown="handleKey">
    <header class="panel-heading picker-heading">
      <div class="picker-title">
        <h1>Copy Markdown As</h1>
        <p v-if="copied || copiedNotice" class="picker-subtitle">
          <span v-if="copied">Copied as {{ copied.label }}</span>
          <span v-if="copiedNotice" class="picker-note" role="status" :title="copiedNotice.tooltip">
            {{ copied ? '· ' : '' }}{{ copiedNotice.label }}
          </span>
        </p>
      </div>
      <span v-if="appContext" class="app-context" :title="appContext">{{ appContext }}</span>
      <button
        type="button"
        class="icon-button"
        aria-label="Settings"
        title="Settings"
        @click="emit('settings')"
      >
        <svg
          viewBox="0 0 24 24"
          width="19"
          height="19"
          fill="none"
          stroke="currentColor"
          stroke-width="1.7"
          aria-hidden="true"
        >
          <path
            d="M9.7 3h4.6l.6 2.7 2 .9 2.4-1.4 2.3 4-2 1.8v2l2 1.8-2.3 4-2.4-1.4-2 .9-.6 2.7H9.7l-.6-2.7-2-.9-2.4 1.4-2.3-4 2-1.8v-2l-2-1.8 2.3-4 2.4 1.4 2-.9z"
          />
          <circle cx="12" cy="12" r="3" />
        </svg>
      </button>
    </header>

    <div class="format-filter">
      <svg viewBox="0 0 20 20" width="16" height="16" fill="none" aria-hidden="true">
        <circle cx="8.5" cy="8.5" r="4.5" stroke="currentColor" stroke-width="1.6" />
        <path d="m12 12 3.5 3.5" stroke="currentColor" stroke-width="1.6" stroke-linecap="round" />
      </svg>
      <input
        ref="filterInput"
        v-model="query"
        type="text"
        aria-label="Filter formats"
        aria-keyshortcuts="Meta+K Control+K"
        placeholder="Filter formats…"
        autocomplete="off"
        spellcheck="false"
      />
      <span class="filter-hint" aria-hidden="true">Type anywhere <kbd>⌘K</kbd></span>
    </div>

    <div
      v-if="filteredTargets.length"
      ref="grid"
      class="format-grid"
      role="group"
      aria-label="Copy formats"
      :style="{ '--columns': columns }"
      :data-columns="columns"
      tabindex="-1"
      @mouseleave="hovered = undefined"
    >
      <button
        v-for="(target, index) in filteredTargets"
        :key="target.id"
        type="button"
        class="format-card"
        :class="{ selected: selected === target.id }"
        :aria-current="selected === target.id ? 'true' : undefined"
        :aria-label="copied?.target === target.id ? `${target.label}, copied` : target.label"
        :aria-keyshortcuts="index < 9 ? `Meta+${index + 1} Control+${index + 1}` : undefined"
        :tabindex="selected === target.id ? 0 : -1"
        :disabled="busy"
        :data-target="target.id"
        :style="{ '--format-accent': formatIcons[target.id].accent }"
        @mouseenter="hovered = target.id"
        @focus="
          selected = target.id;
          hovered = undefined;
        "
        @click="chooseCard(target.id)"
      >
        <span class="format-icon">
          <svg viewBox="0 0 24 24" width="19" height="19" aria-hidden="true">
            <path v-for="path in formatIcons[target.id].paths" :key="path.d" v-bind="path" />
          </svg>
        </span>
        <span class="format-label">{{ target.label }}</span>
        <span
          v-if="copied?.target === target.id || selected === target.id"
          class="card-check"
          aria-hidden="true"
        >
          <svg
            viewBox="0 0 16 16"
            width="13"
            height="13"
            fill="none"
            stroke="currentColor"
            stroke-width="2"
          >
            <path d="m4 8 2.5 2.5L12 5" />
          </svg>
        </span>
        <kbd v-if="index < 9" class="format-key" aria-hidden="true">{{ index + 1 }}</kbd>
      </button>
    </div>
    <p v-else class="empty-formats" role="status">No matching formats — press Esc to clear</p>

    <div v-if="request.sourcePreview != null && previewTarget" class="picker-preview">
      <div class="preview-header">
        <button
          type="button"
          class="preview-toggle"
          :aria-expanded="previewExpanded"
          aria-controls="picker-preview-output"
          @click="togglePreview"
        >
          <svg viewBox="0 0 16 16" width="13" height="13" fill="none" aria-hidden="true">
            <path
              d="m6 3.5 4 4.5-4 4.5"
              stroke="currentColor"
              stroke-width="1.6"
              stroke-linecap="round"
              stroke-linejoin="round"
            />
          </svg>
          Preview
        </button>
        <span class="preview-hint" aria-hidden="true">Hover to preview · Click to copy</span>
      </div>
      <PreviewOutput
        v-show="previewExpanded"
        id="picker-preview-output"
        :label="previewTarget.label"
        :result="preview"
        :failed="previewFailed"
      />
    </div>

    <footer class="panel-footer">
      <button type="button" class="button" :disabled="busy" @click="emit('dismiss')">Cancel</button>
      <button
        type="button"
        class="button primary"
        :disabled="busy || !selectedTarget"
        @click="chooseSelected"
      >
        {{ isDone ? 'Done' : `Copy ${selectedTarget?.label ?? 'format'}` }}
      </button>
    </footer>
  </section>
</template>
