<script setup lang="ts">
import { computed, ref } from 'vue';
import type { ConfigV2, CopyTarget, DesktopApi, DesktopState, LoginItemStatus } from '../lib/ipc';
import { useMarkdownPreview } from '../lib/preview';
import { formatShortcut } from '../lib/shortcut';
import PreviewOutput from './PreviewOutput.vue';

const props = defineProps<{ state: DesktopState; api: DesktopApi }>();
const emit = defineEmits<{
  config: [config: ConfigV2];
  login: [status: LoginItemStatus];
}>();

const sample = [
  '# Friday handoff',
  '',
  'The **release** is ready for *review*. Run `pnpm test` before merging.',
  '',
  '- Read the [release notes](https://example.com/release-notes)',
  '- Share feedback in the team chat',
].join('\n');
const sampleTargets = [
  { id: 'rich-text', label: 'Rich Text' },
  { id: 'slack-paste', label: 'Slack' },
  { id: 'discord', label: 'Discord' },
  { id: 'signal', label: 'Signal' },
  { id: 'telegram', label: 'Telegram' },
] satisfies { id: CopyTarget; label: string }[];
const selected = ref(sampleTargets[0]);
const shortcut = computed(() => formatShortcut(props.state.config.shortcut));
const { result: preview, failed: previewFailed } = useMarkdownPreview(
  props.api,
  () => sample,
  () => selected.value.id,
);
const busy = ref(false);
const copied = ref(false);
const error = ref('');

async function attempt(action: () => Promise<void>) {
  if (busy.value) return;
  busy.value = true;
  error.value = '';
  try {
    await action();
  } catch (cause) {
    error.value = String(cause);
  } finally {
    busy.value = false;
  }
}

function selectTarget(target: (typeof sampleTargets)[number]) {
  selected.value = target;
  copied.value = false;
}

function copySample() {
  return attempt(async () => {
    copied.value = false;
    await props.api.copySample(sample, selected.value.id);
    copied.value = true;
  });
}

function changeLogin() {
  return attempt(async () => {
    emit('login', await props.api.toggleLogin(props.state.loginItemStatus === 'disabled'));
  });
}

function getStarted() {
  return attempt(async () => {
    emit('config', await props.api.updateConfig({ hasLaunched: true }));
    await props.api.closePanel();
  });
}
</script>

<template>
  <section class="welcome-panel" aria-labelledby="welcome-title">
    <div class="welcome-body">
      <header class="panel-heading welcome-heading">
        <h1 id="welcome-title">Welcome to copied.md</h1>
        <p>
          Copy Markdown anywhere, paste it formatted for Slack, Signal, Telegram, Discord, Word, and
          more.
        </p>
      </header>

      <ol class="welcome-steps">
        <li>1. Copy Markdown</li>
        <li>
          2. Press shortcut
          <span class="shortcut-keys" role="group" :aria-label="shortcut">
            <kbd v-for="part in shortcut.split(' + ')" :key="part" aria-hidden="true">{{
              part
            }}</kbd>
          </span>
        </li>
        <li>3. Paste formatted</li>
      </ol>

      <section class="sample-card" aria-labelledby="sample-title">
        <h2 id="sample-title">Try it now</h2>
        <div class="sample-targets" role="group" aria-label="Sample format">
          <button
            v-for="target in sampleTargets"
            :key="target.id"
            type="button"
            class="button sample-target"
            :aria-pressed="selected.id === target.id"
            :disabled="busy"
            @click="selectTarget(target)"
          >
            {{ target.label }}
          </button>
        </div>
        <div class="sample-compare">
          <div class="preview-output">
            <div class="preview-meta"><span class="preview-label">Markdown</span></div>
            <pre class="sample-source" tabindex="0" aria-label="Sample Markdown">{{ sample }}</pre>
          </div>
          <PreviewOutput :label="selected.label" :result="preview" :failed="previewFailed" />
        </div>
        <div class="sample-actions">
          <button class="button" type="button" :disabled="busy" @click="copySample">
            Copy Sample
          </button>
          <p v-if="copied" class="sample-copied" role="status">Copied! Try pasting it anywhere.</p>
        </div>
      </section>

      <p v-if="error" class="warning welcome-error" role="alert">{{ error }}</p>
    </div>
    <footer class="panel-footer welcome-footer">
      <label v-if="state.capabilities.launchAtLogin" class="welcome-login">
        <input
          type="checkbox"
          :checked="state.loginItemStatus !== 'disabled'"
          :disabled="busy || state.loginItemStatus === 'unsupported'"
          @click.prevent="changeLogin"
        />
        Launch at Login
      </label>
      <button class="button primary" type="button" :disabled="busy" @click="getStarted">
        Get Started
      </button>
    </footer>
  </section>
</template>
