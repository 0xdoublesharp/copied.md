import { createApp, type App as VueApp } from 'vue';
import { afterEach, describe, expect, it } from 'vitest';
import { formatIcons } from '@0xdoublesharp/copied-md-shared-icons';
import App from '../App.vue';
import {
  createDesktopApi,
  type ConfigPatch,
  type DesktopBridge,
  type DesktopState,
  type PickerOpenRequest,
  type ShortcutSpec,
} from '../lib/ipc';

const shortcut: ShortcutSpec = { code: 'KeyC', ctrl: true, meta: true, alt: false, shift: false };
const targets: DesktopState['targets'] = [
  { id: 'rich-text', label: 'Rich Text', rich: true },
  { id: 'plain-text', label: 'Plain Text', rich: false },
  { id: 'html-source', label: 'HTML Source', rich: false },
  { id: 'slack-paste', label: 'Slack', rich: true },
  { id: 'discord', label: 'Discord', rich: false },
  { id: 'telegram', label: 'Telegram', rich: true },
  { id: 'teams', label: 'Teams', rich: true, enabledByDefault: false },
];
const editor = { id: 'mac:bundle:com.example.Editor', name: 'Editor' };

function fixture(): DesktopState {
  const currentPicker: PickerOpenRequest = {
    opId: 1,
    visibleTargets: targets.filter((target) => target.enabledByDefault !== false),
    columns: 4,
    selected: 'rich-text',
  };
  return {
    config: {
      schemaVersion: 2,
      shortcut,
      shortcutEnabled: true,
      shortcutMode: 'copy-and-pick',
      formatVisibility: {},
      pickerColumns: 4,
      pinnedFormats: {},
      autoDetectAppFormat: true,
      hasLaunched: true,
    },
    targets,
    activeApp: editor,
    capabilities: {
      backgroundClipboard: true,
      activeAppDetection: true,
      globalShortcuts: 'direct',
      launchAtLogin: true,
    },
    loginItemStatus: 'disabled',
    shortcutRegistered: true,
    shortcutFailure: null,
    currentPicker,
    panel: 'picker',
  };
}

const apps: VueApp[] = [];

afterEach(() => {
  for (const app of apps.splice(0)) app.unmount();
  document.body.replaceChildren();
});

async function settle() {
  await new Promise<void>((resolve) => setTimeout(resolve, 0));
}

async function mount(state = fixture()) {
  const calls: [string, Record<string, unknown> | undefined][] = [];
  const events = new Map<string, (payload: never) => void>();
  let rejectCommit = false;
  const bridge: DesktopBridge = {
    async invoke<T>(command: string, args?: Record<string, unknown>): Promise<T> {
      calls.push([command, args]);
      let response: unknown;
      switch (command) {
        case 'get_desktop_state':
          response = structuredClone(state);
          break;
        case 'update_config': {
          const { formatVisibility, ...patch } = args?.patch as ConfigPatch;
          state.config = {
            ...state.config,
            ...patch,
            lastTarget: patch.lastTarget ?? state.config.lastTarget,
            pinnedFormats: state.config.pinnedFormats,
            formatVisibility: { ...state.config.formatVisibility, ...formatVisibility },
          };
          response = state.config;
          break;
        }
        case 'set_app_pin': {
          const { appId, target } = args as {
            appId: string;
            target: keyof typeof formatIcons | null;
          };
          const pinnedFormats = { ...state.config.pinnedFormats };
          if (target) pinnedFormats[appId] = target;
          else delete pinnedFormats[appId];
          state.config = { ...state.config, pinnedFormats };
          response = state.config;
          break;
        }
        case 'toggle_login_item':
          state.loginItemStatus = args?.enabled ? 'requires-approval' : 'disabled';
          response = state.loginItemStatus;
          break;
        case 'commit_shortcut_recording':
          if (rejectCommit) throw 'Shortcut already in use. The previous shortcut was restored.';
          state.config = { ...state.config, shortcut: args?.spec as ShortcutSpec };
          response = state.config;
          break;
      }
      return response as T;
    },
    async listen<T>(event: string, callback: (payload: T) => void) {
      events.set(event, callback);
      return () => void events.delete(event);
    },
  };
  const root = document.createElement('div');
  document.body.append(root);
  const app = createApp(App, { api: createDesktopApi(bridge) });
  app.mount(root);
  apps.push(app);
  await settle();

  return {
    root,
    calls,
    rejectCommit: () => (rejectCommit = true),
    async reopen(request: PickerOpenRequest) {
      state.currentPicker = request;
      events.get('picker:open')?.(request as never);
      await settle();
    },
  };
}

function button(root: HTMLElement, label: string): HTMLButtonElement {
  const result = [...root.querySelectorAll('button')].find(
    (element) =>
      element.textContent?.trim() === label || element.getAttribute('aria-label') === label,
  );
  if (!result) throw new Error(`Missing button "${label}"`);
  return result;
}

function checkbox(root: HTMLElement, label: string): HTMLInputElement {
  const result = [...root.querySelectorAll('label')]
    .find((element) => element.textContent?.trim() === label)
    ?.querySelector<HTMLInputElement>('input');
  if (!result) throw new Error(`Missing checkbox "${label}"`);
  return result;
}

async function key(key: string, options: KeyboardEventInit = {}) {
  document.activeElement?.dispatchEvent(
    new KeyboardEvent('keydown', { bubbles: true, cancelable: true, key, ...options }),
  );
  await settle();
}

describe('format picker', () => {
  it.each([3, 4, 5] as const)('renders %i columns and destination marks', async (columns) => {
    const state = fixture();
    state.config.pickerColumns = columns;
    const { root } = await mount(state);
    const grid = root.querySelector<HTMLElement>('.format-grid')!;
    expect(grid.dataset.columns).toBe(String(columns));
    expect(grid.style.getPropertyValue('--columns')).toBe(String(columns));
    expect(grid.querySelectorAll('button svg[viewBox="0 0 24 24"]')).toHaveLength(6);
    expect(Object.keys(formatIcons)).toHaveLength(15);
  });

  it('moves focus with arrows, Home and End, then copies the highlighted format', async () => {
    const { root, calls } = await mount();
    expect(document.activeElement).toBe(button(root, 'Rich Text'));
    await key('ArrowDown');
    expect(document.activeElement).toBe(button(root, 'Discord'));
    await key('ArrowRight');
    expect(document.activeElement).toBe(button(root, 'Telegram'));
    await key('ArrowLeft');
    expect(document.activeElement).toBe(button(root, 'Discord'));
    await key('ArrowUp');
    expect(document.activeElement).toBe(button(root, 'Rich Text'));
    await key('End');
    expect(document.activeElement).toBe(button(root, 'Telegram'));
    await key('Home');
    await key('ArrowRight');
    await key('Enter');
    expect(calls.at(-1)).toEqual(['picker_choose', { opId: 1, target: 'plain-text' }]);
  });

  it('dismisses with Escape and uses the fresh opId when reopened', async () => {
    const state = fixture();
    const { root, calls, reopen } = await mount(state);
    await key('Escape');
    expect(calls.at(-1)).toEqual(['picker_dismiss', { opId: 1 }]);
    await reopen({ ...state.currentPicker!, opId: 2, selected: 'slack-paste' });
    expect(document.activeElement).toBe(button(root, 'Slack'));
    button(root, 'Discord').click();
    await settle();
    expect(calls.at(-1)).toEqual(['picker_choose', { opId: 2, target: 'discord' }]);
  });

  it('shows the full warning in a tooltip and Enter confirms an automatic copy', async () => {
    const state = fixture();
    state.currentPicker!.copied = {
      target: 'rich-text',
      label: 'Rich Text',
      warning: 'Some formatting was omitted.',
    };
    const { root, calls } = await mount(state);
    expect(root.querySelector('.warning')?.getAttribute('title')).toBe(
      'Some formatting was omitted.',
    );
    expect(button(root, 'Rich Text, copied').querySelector('.card-check')).not.toBeNull();
    expect(button(root, 'Done')).toBeDefined();
    await key('Enter');
    expect(calls.at(-1)).toEqual(['picker_choose', { opId: 1, target: null }]);
  });
});

describe('desktop settings', () => {
  it('updates columns, format visibility, and shortcut behavior in the active picker', async () => {
    const { root, calls } = await mount();
    button(root, 'Settings').click();
    await settle();
    button(root, '5').click();
    await settle();
    expect(button(root, '5').getAttribute('aria-pressed')).toBe('true');
    checkbox(root, 'Teams').click();
    await settle();
    expect(checkbox(root, 'Teams').checked).toBe(true);
    checkbox(root, 'Copy Last Used Only').click();
    await settle();
    expect(calls.at(-1)).toEqual(['update_config', { patch: { shortcutMode: 'copy-only' } }]);
    expect(checkbox(root, 'Copy Last Used Only').checked).toBe(true);
    button(root, 'Back to Picker').click();
    await settle();
    expect(root.querySelector('.format-grid')?.getAttribute('data-columns')).toBe('5');
    expect(button(root, 'Teams')).toBeDefined();
  });

  it('prevents hiding the final visible format', async () => {
    const state = fixture();
    for (const { id } of targets.slice(1)) state.config.formatVisibility[id] = false;
    const { root, calls } = await mount(state);
    button(root, 'Settings').click();
    await settle();
    const last = checkbox(root, 'Rich Text');
    expect(last.checked && last.disabled).toBe(true);
    last.click();
    expect(calls.some(([command]) => command === 'update_config')).toBe(false);
  });

  it('sets and clears a format pin and toggles auto-detection for the captured foreground application', async () => {
    const state = fixture();
    state.panel = 'app-pin';
    state.currentPicker = null;
    const { root, calls } = await mount(state);
    const autoDetect = checkbox(
      root,
      'Auto-detect destination app (Slack, Signal, Telegram, Discord, Teams, Word, WhatsApp)',
    );
    expect(autoDetect.checked).toBe(true);
    autoDetect.click();
    await settle();
    expect(calls.at(-1)).toEqual(['update_config', { patch: { autoDetectAppFormat: false } }]);
    expect(autoDetect.checked).toBe(false);
    const input = root.querySelector('select')!;
    expect(root.textContent).toContain('Format for Editor');
    expect([...input.options].find((option) => option.value === 'teams')?.disabled).toBe(true);
    input.value = 'plain-text';
    input.dispatchEvent(new Event('change'));
    await settle();
    expect(calls.at(-1)).toEqual(['set_app_pin', { appId: editor.id, target: 'plain-text' }]);
    expect(input.value).toBe('plain-text');
    input.value = '';
    input.dispatchEvent(new Event('change'));
    await settle();
    expect(calls.at(-1)).toEqual(['set_app_pin', { appId: editor.id, target: null }]);
  });

  it('requires a valid alphanumeric chord and records physical code and modifiers', async () => {
    const { root, calls } = await mount();
    button(root, 'Settings').click();
    await settle();
    button(root, 'Change Keyboard Shortcut…').click();
    await settle();
    expect(calls.at(-1)).toEqual(['begin_shortcut_recording', undefined]);
    await key('m', { code: 'KeyM', ctrlKey: true });
    expect(root.querySelector('[role="alert"]')?.textContent).toContain('at least two modifiers');
    await key('?', { code: 'Slash', ctrlKey: true, metaKey: true });
    await key('m', { code: 'KeyM', shiftKey: true, altKey: true });
    expect(calls.at(-1)?.[0]).toBe('begin_shortcut_recording');
    const spec = { code: 'KeyM', ctrl: true, alt: true, shift: false, meta: false };
    await key('µ', { code: spec.code, ctrlKey: spec.ctrl, altKey: spec.alt });
    expect(calls.at(-1)).toEqual(['commit_shortcut_recording', { spec }]);
    expect(root.textContent).toContain('Ctrl + Alt + M');
  });

  it('keeps the previous binding on a conflict and Escape cancels a new recording', async () => {
    const state = fixture();
    state.panel = 'shortcut';
    state.currentPicker = null;
    const { root, calls, rejectCommit } = await mount(state);
    rejectCommit();
    await key('x', { code: 'KeyX', ctrlKey: true, metaKey: true });
    expect(root.querySelector('[role="alert"]')?.textContent).toContain(
      'previous shortcut was restored',
    );
    expect(root.textContent).toContain(' + C');
    button(root, 'Change Keyboard Shortcut…').click();
    await settle();
    await key('Escape');
    expect(calls.at(-1)).toEqual(['cancel_shortcut_recording', undefined]);
    expect(button(root, 'Change Keyboard Shortcut…')).toBe(document.activeElement);
  });

  it('explains Wayland portal and active-app limitations without offering unsupported controls', async () => {
    const state = fixture();
    state.currentPicker = null;
    state.panel = 'settings';
    state.capabilities.globalShortcuts = 'portal';
    state.capabilities.activeAppDetection = false;
    state.capabilities.backgroundClipboard = false;
    const { root } = await mount(state);
    expect(root.textContent).toContain('desktop portal');
    expect(root.textContent).toContain('Per-app formats are unavailable on Wayland');
    expect(root.textContent).toContain('restricts clipboard access');
    expect(root.querySelector('select')).toBeNull();
    expect(root.textContent).not.toContain('Change Keyboard Shortcut');
  });

  it('shows when the operating system requires Launch at Login approval', async () => {
    const state = fixture();
    state.currentPicker = null;
    state.panel = 'settings';
    const { root, calls } = await mount(state);
    checkbox(root, 'Launch at Login').click();
    await settle();
    expect(calls.at(-1)).toEqual(['toggle_login_item', { enabled: true }]);
    expect(checkbox(root, 'Launch at Login').checked).toBe(true);
    expect(root.textContent).toContain('Allow copied.md to launch at login');
  });
});
