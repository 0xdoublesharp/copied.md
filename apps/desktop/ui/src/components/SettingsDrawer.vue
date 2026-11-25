<script setup lang="ts">
/* global Event, HTMLElement, HTMLButtonElement, HTMLSelectElement, KeyboardEvent */
import { computed, nextTick, onMounted, ref } from 'vue';
import {
  type Columns,
  type ConfigV2,
  type CopyTarget,
  type DesktopApi,
  type DesktopState,
  type LoginItemStatus,
  type Panel,
  type ShortcutMode,
  visibleTargets,
} from '../lib/ipc';
import { captureShortcut, formatShortcut, SHORTCUT_HINT } from '../lib/shortcut';

const props = defineProps<{
  state: DesktopState;
  api: DesktopApi;
  panel: Panel;
  backLabel: string;
}>();
const emit = defineEmits<{
  config: [config: ConfigV2];
  login: [status: LoginItemStatus];
  close: [];
}>();

const modes: { id: ShortcutMode; label: string }[] = [
  { id: 'copy-and-pick', label: 'Copy Last Used and Show Picker' },
  { id: 'picker', label: 'Show Picker First' },
  { id: 'copy-only', label: 'Copy Last Used Only' },
];
const columns: Columns[] = [3, 4, 5];
const direct = computed(() => props.state.capabilities.globalShortcuts === 'direct');
const visible = computed(() => visibleTargets(props.state));
const visibleIds = computed(() => new Set(visible.value.map((target) => target.id)));
const activeApp = computed(() =>
  props.state.capabilities.activeAppDetection ? props.state.activeApp : null,
);
const appPin = computed(
  () => activeApp.value && props.state.config.pinnedFormats[activeApp.value.id],
);
const appPinSelect = ref<HTMLSelectElement>();
const capture = ref<HTMLElement>();
const back = ref<HTMLButtonElement>();
const recorder = ref<HTMLButtonElement>();
const busy = ref(false);
const recording = ref(false);
const error = ref('');

async function attempt(action: () => Promise<void>) {
  if (busy.value) return;
  const wasRecording = recording.value;
  busy.value = true;
  error.value = '';
  try {
    await action();
  } catch (cause) {
    error.value = String(cause);
  } finally {
    busy.value = false;
    if (wasRecording && !recording.value) {
      await nextTick();
      (recorder.value ?? back.value)?.focus();
    }
  }
}

function saveConfig(action: () => Promise<ConfigV2>) {
  return attempt(async () => emit('config', await action()));
}

function toggleVisibility(target: CopyTarget) {
  const shown = visibleIds.value.has(target);
  if (shown && visible.value.length === 1) return;
  return saveConfig(() => props.api.updateConfig({ formatVisibility: { [target]: !shown } }));
}

function setPin(event: Event) {
  if (!activeApp.value) return;
  const { id } = activeApp.value;
  const input = event.currentTarget as HTMLSelectElement;
  const target = (input.value || null) as CopyTarget | null;
  input.value = appPin.value ?? '';
  return saveConfig(() => props.api.setAppPin(id, target));
}

function beginRecording() {
  return attempt(async () => {
    await props.api.beginRecording();
    recording.value = true;
    await nextTick();
    capture.value?.focus();
  });
}

function cancelRecording() {
  return attempt(async () => {
    try {
      await props.api.cancelRecording();
    } finally {
      recording.value = false;
    }
  });
}

function handleKey(event: KeyboardEvent) {
  if (!recording.value && event.key !== 'Escape') return;
  // Keep normal tab navigation and keyboard activation of the recorder's Cancel button.
  if (event.key === 'Tab' || (event.target !== capture.value && event.key === 'Enter')) return;
  event.preventDefault();
  event.stopPropagation();
  if (busy.value) return;
  if (event.key === 'Escape') {
    if (recording.value) void cancelRecording();
    else emit('close');
    return;
  }
  if (
    event.repeat ||
    event.isComposing ||
    /^(Control|Shift|Alt|Meta)(Left|Right)$/.test(event.code)
  )
    return;
  const spec = captureShortcut(event);
  if (!spec) {
    error.value = SHORTCUT_HINT;
    return;
  }
  void attempt(async () => {
    try {
      emit('config', await props.api.commitRecording(spec));
    } finally {
      recording.value = false;
    }
  });
}

function close() {
  if (busy.value) return;
  if (!recording.value) {
    emit('close');
    return;
  }
  void cancelRecording().then(() => emit('close'));
}

function changeLogin() {
  return attempt(async () => {
    const enable = props.state.loginItemStatus === 'disabled';
    emit('login', await props.api.toggleLogin(enable));
  });
}

onMounted(() => {
  if (props.panel === 'shortcut' && direct.value) void beginRecording();
  else if (props.panel === 'app-pin' && appPinSelect.value) appPinSelect.value.focus();
  else back.value?.focus();
});
</script>

<template>
  <section class="settings-panel" aria-label="Settings" @keydown.capture="handleKey">
    <header class="panel-heading settings-heading">
      <div>
        <h1>
          {{ panel === 'app-pin' && activeApp ? `Format for ${activeApp.name}` : 'Settings' }}
        </h1>
      </div>
      <button ref="back" type="button" class="button" :disabled="busy" @click="close">
        {{ backLabel }}
      </button>
    </header>
    <div class="settings-body">
      <p v-if="error" role="alert" class="warning">{{ error }}</p>

      <fieldset v-if="panel !== 'app-pin'" :disabled="busy || recording">
        <legend>Picker columns</legend>
        <div class="segments">
          <button
            v-for="count in columns"
            :key="count"
            type="button"
            class="button"
            :aria-pressed="state.config.pickerColumns === count"
            @click="saveConfig(() => api.updateConfig({ pickerColumns: count }))"
          >
            {{ count }}
          </button>
        </div>
      </fieldset>

      <fieldset v-if="panel !== 'app-pin'" :disabled="busy || recording">
        <legend>Visible formats</legend>
        <div class="format-toggles">
          <label v-for="target in state.targets" :key="target.id">
            <input
              type="checkbox"
              :checked="visibleIds.has(target.id)"
              :disabled="visibleIds.has(target.id) && visible.length === 1"
              @click.prevent="toggleVisibility(target.id)"
            />
            {{ target.label }}
          </label>
        </div>
      </fieldset>

      <fieldset v-if="panel !== 'app-pin'" :disabled="busy || recording">
        <legend>Shortcut Behavior</legend>
        <label v-for="mode in modes" :key="mode.id">
          <input
            type="radio"
            name="shortcut-mode"
            :checked="state.config.shortcutMode === mode.id"
            @click.prevent="saveConfig(() => api.updateConfig({ shortcutMode: mode.id }))"
          />
          {{ mode.label }}
        </label>
      </fieldset>

      <fieldset :disabled="busy || recording">
        <legend>Per-app format</legend>
        <label v-if="state.capabilities.activeAppDetection">
          <input
            type="checkbox"
            :checked="state.config.autoDetectAppFormat"
            @click.prevent="
              saveConfig(() =>
                api.updateConfig({ autoDetectAppFormat: !state.config.autoDetectAppFormat }),
              )
            "
          />
          Auto-detect destination app (Slack, Signal, Telegram, Discord, Teams, Word, WhatsApp)
        </label>
        <label v-if="activeApp" class="stacked-label">
          Override for {{ activeApp.name }}
          <select ref="appPinSelect" :value="appPin ?? ''" @change="setPin">
            <option value="">Auto / Last-Used Format</option>
            <option
              v-for="target in state.targets"
              :key="target.id"
              :value="target.id"
              :disabled="!visibleIds.has(target.id)"
            >
              {{ target.label }}{{ visibleIds.has(target.id) ? '' : ' (hidden)' }}
            </option>
          </select>
        </label>
        <p v-else-if="!state.capabilities.activeAppDetection">
          Per-app formats are unavailable on Wayland or when the desktop cannot identify the active
          app.
        </p>
        <p v-else>Focus an app and reopen Settings to override its format.</p>
      </fieldset>

      <fieldset v-if="panel !== 'app-pin'" :disabled="busy">
        <legend>Keyboard shortcut</legend>
        <template v-if="direct">
          <label>
            <input
              type="checkbox"
              :checked="state.config.shortcutEnabled"
              :disabled="recording"
              @click.prevent="saveConfig(() => api.toggleShortcut(!state.config.shortcutEnabled))"
            />
            Use Keyboard Shortcut
          </label>
          <div v-if="recording" ref="capture" class="recording" tabindex="0">
            {{ SHORTCUT_HINT }}
            <button type="button" class="button" @click="cancelRecording">Cancel Recording</button>
          </div>
          <template v-else>
            <p>{{ formatShortcut(state.config.shortcut) }}</p>
            <p v-if="state.shortcutFailure" role="status">{{ state.shortcutFailure }}</p>
            <div class="settings-actions">
              <button ref="recorder" type="button" class="button" @click="beginRecording">
                Change Keyboard Shortcut…
              </button>
              <button type="button" class="button" @click="saveConfig(api.resetShortcut)">
                Reset to Default Shortcut
              </button>
            </div>
          </template>
        </template>
        <p v-else-if="state.capabilities.globalShortcuts === 'portal'">
          Wayland manages global shortcuts through its desktop portal. Use your desktop’s keyboard
          shortcut settings or choose Copy Markdown As… from the tray.
        </p>
        <p v-else>Global shortcuts are unavailable in this desktop session.</p>
        <p v-if="!state.capabilities.backgroundClipboard">
          This desktop session restricts clipboard access from the tray.
        </p>
      </fieldset>

      <fieldset
        v-if="panel !== 'app-pin'"
        :disabled="busy || recording || !state.capabilities.launchAtLogin"
      >
        <legend>Startup</legend>
        <label>
          <input
            type="checkbox"
            :checked="
              state.loginItemStatus === 'enabled' || state.loginItemStatus === 'requires-approval'
            "
            @click.prevent="changeLogin"
          />
          Launch at Login
        </label>
        <p v-if="state.loginItemStatus === 'requires-approval'">
          Allow copied.md to launch at login in your system settings.
        </p>
      </fieldset>
    </div>
  </section>
</template>
