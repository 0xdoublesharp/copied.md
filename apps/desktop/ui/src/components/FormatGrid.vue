<script setup lang="ts">
/* global HTMLElement, HTMLButtonElement, KeyboardEvent, Node */
import { computed, nextTick, ref, watch } from 'vue';
import { formatIcons } from '@0xdoublesharp/copied-md-shared-icons';
import type { Columns, CopyTarget, PickerOpenRequest, TargetDefinition } from '../lib/ipc';

const props = defineProps<{
  request: PickerOpenRequest;
  targets: TargetDefinition[];
  columns: Columns;
  busy?: boolean;
}>();

const emit = defineEmits<{
  choose: [target: CopyTarget | null];
  dismiss: [];
  settings: [];
}>();

const grid = ref<HTMLElement>();
const selected = ref<CopyTarget>();
const copied = computed(() => props.request.copied);
const selectedIndex = computed(() =>
  props.targets.findIndex((target) => target.id === selected.value),
);
const selectedTarget = computed(() => props.targets[selectedIndex.value]);
const isDone = computed(() => copied.value != null && selected.value === copied.value.target);

function focusSelection() {
  void nextTick(() => {
    const button = grid.value?.querySelector<HTMLButtonElement>('button[tabindex="0"]');
    (button ?? grid.value)?.focus();
  });
}

function selectInitial() {
  const initial = props.request.selected;
  selected.value = props.targets.some((target) => target.id === initial)
    ? initial
    : props.targets[0]?.id;
  focusSelection();
}

watch(() => props.request.opId, selectInitial, { immediate: true });
watch(
  () => props.targets,
  (targets) => {
    if (!targets.some((target) => target.id === selected.value)) selectInitial();
  },
);

function chooseSelected() {
  if (props.busy || !selectedTarget.value) return;
  emit('choose', isDone.value ? null : selectedTarget.value.id);
}

function handleKey(event: KeyboardEvent) {
  if (event.ctrlKey || event.altKey || event.metaKey || event.shiftKey || event.isComposing) return;
  if (event.key === 'Escape') {
    event.preventDefault();
    if (!props.busy) emit('dismiss');
    return;
  }
  // Keep native activation for the gear and the footer buttons.
  if (event.key === 'Enter' && grid.value?.contains(event.target as Node)) {
    event.preventDefault();
    chooseSelected();
    return;
  }

  const current = Math.max(0, selectedIndex.value);
  const last = props.targets.length - 1;
  const nextIndices: Record<string, number> = {
    ArrowLeft: current - 1,
    ArrowRight: current + 1,
    ArrowUp: current - props.columns,
    ArrowDown: current + props.columns,
    Home: 0,
    End: last,
  };
  if (!Object.hasOwn(nextIndices, event.key) || props.busy || last < 0) return;
  event.preventDefault();
  const next = Math.max(0, Math.min(last, nextIndices[event.key]));
  selected.value = props.targets[next].id;
  focusSelection();
}
</script>

<template>
  <section class="picker-panel" aria-label="Copy Markdown As" @keydown="handleKey">
    <header class="panel-heading">
      <div>
        <h1>Copy Markdown As</h1>
        <p v-if="copied">Copied as {{ copied.label }}. Choose a different format to replace it.</p>
        <p v-else>Choose a format for the Markdown on your clipboard.</p>
      </div>
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

    <p v-if="copied?.warning" class="warning" role="status" :title="copied.warning">
      {{ copied.warning }}
    </p>

    <div
      ref="grid"
      class="format-grid"
      role="group"
      aria-label="Copy formats"
      :style="{ '--columns': columns }"
      :data-columns="columns"
      tabindex="-1"
    >
      <button
        v-for="target in targets"
        :key="target.id"
        type="button"
        class="format-card"
        :class="{ selected: selected === target.id }"
        :aria-current="selected === target.id ? 'true' : undefined"
        :aria-label="copied?.target === target.id ? `${target.label}, copied` : target.label"
        :tabindex="selected === target.id ? 0 : -1"
        :disabled="busy"
        :data-target="target.id"
        :style="{ '--format-accent': formatIcons[target.id].accent }"
        @focus="selected = target.id"
        @click="emit('choose', copied?.target === target.id ? null : target.id)"
      >
        <span class="format-icon">
          <svg viewBox="0 0 24 24" width="25" height="25" aria-hidden="true">
            <path v-for="path in formatIcons[target.id].paths" :key="path.d" v-bind="path" />
          </svg>
        </span>
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
        <span>{{ target.label }}</span>
      </button>
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
