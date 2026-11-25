<script setup lang="ts">
import { computed, onMounted, onUnmounted, ref, shallowRef } from 'vue';
import FormatGrid from './components/FormatGrid.vue';
import SettingsDrawer from './components/SettingsDrawer.vue';
import {
  desktopApi,
  type CopyTarget,
  type DesktopApi,
  type DesktopState,
  type PickerOpenRequest,
  visibleTargets,
} from './lib/ipc';

const props = withDefaults(defineProps<{ api?: DesktopApi }>(), { api: () => desktopApi });

const state = shallowRef<DesktopState>();
const targets = computed(() => (state.value ? visibleTargets(state.value) : []));
const settings = ref(false);
const pending = ref(false);
const error = ref('');
const opening = ref(0);
const cleanups: (() => void)[] = [];
let unmounted = false;
let refreshGeneration = 0;

async function refresh(request?: PickerOpenRequest) {
  const generation = ++refreshGeneration;
  if (request && state.value && (state.value.currentPicker?.opId ?? 0) < request.opId) {
    state.value = { ...state.value, currentPicker: request, panel: 'picker' };
    settings.value = false;
    pending.value = false;
    opening.value = generation;
  }
  error.value = '';
  try {
    const next = await props.api.getState();
    if (unmounted || generation !== refreshGeneration) return;
    opening.value = generation;
    state.value = next;
    settings.value = next.panel !== 'picker';
    pending.value = false;
  } catch (cause) {
    if (!unmounted && generation === refreshGeneration) error.value = String(cause);
  }
}

onMounted(async () => {
  try {
    const results = await Promise.allSettled([
      props.api.onPickerOpen(refresh),
      props.api.onPanelOpen(() => void refresh()),
    ]);
    for (const result of results) {
      if (result.status === 'fulfilled') {
        if (unmounted) result.value();
        else cleanups.push(result.value);
      }
    }
    if (unmounted) return;
    const failure = results.find((result) => result.status === 'rejected');
    if (failure) throw failure.reason;
    await refresh();
  } catch (cause) {
    if (!unmounted) error.value = String(cause);
  }
});

onUnmounted(() => {
  unmounted = true;
  for (const cleanup of cleanups) cleanup();
});

async function finish(action: () => Promise<void>) {
  if (pending.value) return;
  const generation = opening.value;
  pending.value = true;
  error.value = '';
  try {
    await action();
  } catch (cause) {
    if (generation === opening.value) error.value = String(cause);
  } finally {
    if (generation === opening.value) pending.value = false;
  }
}

function choose(target: CopyTarget | null) {
  const request = state.value?.currentPicker;
  if (request) void finish(() => props.api.choose(request.opId, target));
}

function dismiss() {
  const request = state.value?.currentPicker;
  if (request) void finish(() => props.api.dismiss(request.opId));
}

function closeSettings() {
  if (state.value?.panel === 'picker') {
    settings.value = false;
  } else {
    void finish(props.api.closePanel);
  }
}
</script>

<template>
  <main class="desktop-shell">
    <p v-if="error" class="warning shell-error" role="alert">{{ error }}</p>
    <SettingsDrawer
      v-if="state?.panel && settings"
      :key="opening"
      :state="state"
      :api="api"
      :panel="state.panel"
      :back-label="state.panel === 'picker' ? 'Back to Picker' : 'Done'"
      @config="state = { ...state, config: $event }"
      @login="state = { ...state, loginItemStatus: $event }"
      @close="closeSettings"
    />
    <FormatGrid
      v-else-if="state?.currentPicker"
      :request="state.currentPicker"
      :targets="targets"
      :columns="state.config.pickerColumns"
      :busy="pending"
      @choose="choose"
      @dismiss="dismiss"
      @settings="settings = true"
    />
  </main>
</template>
