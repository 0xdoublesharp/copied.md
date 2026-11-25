import { createApp, type App as VueApp } from 'vue';
import { afterEach, describe, expect, it, vi } from 'vitest';
import { formatIcons } from '@0xdoublesharp/copied-md-shared-icons';
import App from '../App.vue';
import {
  createDesktopApi,
  type ConfigPatch,
  type CopyTarget,
  type DesktopBridge,
  type DesktopState,
  type MarkdownConversion,
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
  window.localStorage.clear();
});

async function settle() {
  await new Promise<void>((resolve) => setTimeout(resolve, 0));
}

async function mount(
  state = fixture(),
  preview = async (args: Record<string, unknown> | undefined): Promise<MarkdownConversion> =>
    conversion(args?.target as CopyTarget, 'Converted text'),
) {
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
        case 'preview_markdown':
          response = await preview(args);
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

function conversion(
  target: CopyTarget,
  text: string,
  metrics: Partial<MarkdownConversion['metrics']> = {},
  diagnostics: MarkdownConversion['diagnostics'] = [],
): MarkdownConversion {
  return {
    schemaVersion: 1,
    target,
    representations: { 'text/plain': text },
    metrics: { sourceCodeUnits: 12, outputLength: text.length, unit: 'code-points', ...metrics },
    diagnostics,
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

async function filter(root: HTMLElement, value: string) {
  const input = root.querySelector<HTMLInputElement>('input[aria-label="Filter formats"]')!;
  input.focus();
  input.value = value;
  input.dispatchEvent(new Event('input', { bubbles: true }));
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

  it('numbers the first nine displayed formats and immediately copies the corresponding card', async () => {
    const state = fixture();
    state.targets = [
      ...targets,
      { id: 'sms', label: 'Messages', rich: false },
      { id: 'whatsapp', label: 'WhatsApp', rich: false },
      { id: 'signal', label: 'Signal', rich: true },
      { id: 'reddit', label: 'Reddit', rich: false },
    ];
    const { root, calls } = await mount(state);
    const cards = [...root.querySelectorAll<HTMLElement>('[data-target]')];
    expect(cards.map((card) => card.querySelector('kbd')?.textContent)).toEqual([
      '1',
      '2',
      '3',
      '4',
      '5',
      '6',
      '7',
      '8',
      '9',
      undefined,
    ]);
    for (const [index, card] of cards.slice(0, 9).entries()) {
      await key(String(index + 1));
      expect(calls.at(-1)).toEqual(['picker_choose', { opId: 1, target: card.dataset.target }]);
    }
  });

  it('focuses and seeds the filter from a card, then copies the filtered selection with Enter', async () => {
    const { root, calls } = await mount();
    await key('S', { shiftKey: true });
    const input = root.querySelector<HTMLInputElement>('input[aria-label="Filter formats"]')!;
    expect(document.activeElement).toBe(input);
    expect(input.value).toBe('S');
    await key('ArrowDown');
    expect(document.activeElement).toBe(button(root, 'HTML Source'));
    await key('L', { shiftKey: true });
    expect(document.activeElement).toBe(input);
    expect(input.value).toBe('SL');
    expect(root.querySelectorAll('[data-target]')).toHaveLength(1);
    expect(button(root, 'Slack').querySelector('kbd')?.textContent).toBe('1');
    await key('Enter');
    expect(calls.at(-1)).toEqual(['picker_choose', { opId: 1, target: 'slack-paste' }]);
  });

  it('focuses the filter with Command+K, then clears an unmatched query before dismissing', async () => {
    const { root, calls } = await mount();
    await key('k', { metaKey: true });
    expect(document.activeElement).toBe(root.querySelector('input'));
    await filter(root, 'nothing matches');
    expect(root.textContent).toContain('No matching formats — press Esc to clear');
    await key('Enter');
    await key('Escape');
    expect(root.querySelector('input')?.value).toBe('');
    expect(root.querySelectorAll('[data-target]')).toHaveLength(6);
    expect(calls.some(([command]) => command.startsWith('picker_'))).toBe(false);
    await key('Escape');
    expect(calls.at(-1)).toEqual(['picker_dismiss', { opId: 1 }]);
  });

  it.each([
    ['msg', 'sms'],
    ['iMessage', 'sms'],
    ['wa', 'whatsapp'],
    ['tg', 'telegram'],
    ['md', 'plain-text'],
    ['plain', 'plain-text'],
    ['SLACK-PASTE', 'slack-paste'],
  ] as const)('matches %s to %s and supports modified number shortcuts', async (query, target) => {
    const state = fixture();
    state.targets = [
      ...targets,
      { id: 'sms', label: 'Messages', rich: false },
      { id: 'whatsapp', label: 'WhatsApp', rich: false },
    ];
    const { root, calls } = await mount(state);
    await filter(root, query);
    expect(
      [...root.querySelectorAll<HTMLElement>('[data-target]')].map((card) => card.dataset.target),
    ).toEqual([target]);
    await key('1');
    expect(calls.at(-1)?.[0]).toBe('get_desktop_state');
    await key('1', { ctrlKey: true });
    expect(calls.at(-1)).toEqual(['picker_choose', { opId: 1, target }]);
    await key('1', { metaKey: true });
    expect(calls.at(-1)).toEqual(['picker_choose', { opId: 1, target }]);
  });

  it('shows the foreground app and attributes a resolved destination to its pin or auto-detection', async () => {
    const state = fixture();
    const slack = { id: 'mac:bundle:com.tinyspeck.slackmacgap', name: 'Slack' };
    state.activeApp = slack;
    state.config.pinnedFormats[slack.id] = 'plain-text';
    state.currentPicker!.suggested = 'plain-text';
    const { root, reopen } = await mount(state);
    expect(root.textContent).toContain('Active: Slack · Pinned');
    delete state.config.pinnedFormats[slack.id];
    await reopen({
      ...state.currentPicker!,
      opId: 2,
      suggested: undefined,
      copied: { target: 'slack-paste', label: 'Slack' },
    });
    expect(root.textContent).toContain('Active: Slack · Auto-detected');
    state.activeApp = editor;
    await reopen({ ...state.currentPicker!, opId: 3 });
    expect(root.querySelector('.app-context')?.textContent?.trim()).toBe('Active: Editor');
  });

  it('renders preview metrics and warnings and ignores a late reply from the previous format', async () => {
    const state = fixture();
    const markdown = '# Hello **team**';
    state.currentPicker!.sourcePreview = markdown;
    let resolveFirst!: (result: MarkdownConversion) => void;
    const first = new Promise<MarkdownConversion>((resolve) => {
      resolveFirst = resolve;
    });
    const { root, calls, reopen } = await mount(state, async (args) => {
      if (args?.target === 'rich-text') return first;
      return conversion('plain-text', 'Hello team', { outputLength: 42, limit: 40 }, [
        { code: 'table-flattened', severity: 'warning', message: 'Table flattened' },
        { code: 'info', severity: 'info', message: 'Informational diagnostic' },
      ]);
    });
    await vi.waitFor(() =>
      expect(calls).toContainEqual([
        'preview_markdown',
        { markdown, target: 'rich-text', sourceUrl: null, decorativeUnicode: false },
      ]),
    );
    await key('ArrowRight');
    await vi.waitFor(() =>
      expect(root.querySelector('.preview-text')?.textContent).toBe('Hello team'),
    );
    expect(calls).toContainEqual([
      'preview_markdown',
      { markdown, target: 'plain-text', sourceUrl: null, decorativeUnicode: false },
    ]);
    expect(root.querySelector('.metric.over-limit')?.textContent).toContain('42 / 40');
    expect(root.textContent).toContain('Table flattened');
    expect(root.textContent).not.toContain('Informational diagnostic');
    resolveFirst(conversion('rich-text', 'Stale result'));
    await settle();
    expect(root.querySelector('.preview-text')?.textContent).toBe('Hello team');
    await reopen({ ...state.currentPicker!, opId: 2, selected: 'plain-text' });
    await vi.waitFor(() => {
      expect(calls.filter(([command]) => command === 'preview_markdown')).toHaveLength(3);
    });
  });

  it('remembers the collapsed preview when the picker is mounted again', async () => {
    const state = fixture();
    state.currentPicker!.sourcePreview = 'Hello';
    const { root } = await mount(state);
    const toggle = button(root, 'Preview');
    expect(toggle.getAttribute('aria-expanded')).toBe('true');
    toggle.click();
    await settle();
    expect(toggle.getAttribute('aria-expanded')).toBe('false');
    expect(root.querySelector<HTMLElement>('#picker-preview-output')?.style.display).toBe('none');
    const next = await mount(state);
    expect(button(next.root, 'Preview').getAttribute('aria-expanded')).toBe('false');
  });
});

describe('first-run welcome', () => {
  it('shows the configured shortcut, previews and copies the selected sample, and finishes onboarding', async () => {
    const state = fixture();
    state.panel = 'welcome';
    state.currentPicker = null;
    state.config.hasLaunched = false;
    const { root, calls } = await mount(state);
    expect(root.querySelector('h1')?.textContent).toBe('Welcome to copied.md');
    expect(root.querySelectorAll('.shortcut-keys kbd')).toHaveLength(3);
    expect(root.querySelector('.shortcut-keys kbd:last-child')?.textContent).toBe('C');
    button(root, 'Discord').click();
    await vi.waitFor(() =>
      expect(root.querySelector('.preview-text')?.textContent).toBe('Converted text'),
    );
    const preview = calls.find(
      ([command, args]) => command === 'preview_markdown' && args?.target === 'discord',
    );
    expect(preview?.[1]).toEqual({
      markdown: expect.stringContaining('**'),
      target: 'discord',
      sourceUrl: null,
      decorativeUnicode: false,
    });
    expect(root.textContent).toContain('14 chars');
    button(root, 'Copy Sample').click();
    await settle();
    expect(calls.at(-1)).toEqual([
      'copy_sample_markdown',
      { markdown: preview?.[1]?.markdown, target: 'discord' },
    ]);
    expect(root.textContent).toContain('Copied! Try pasting it anywhere.');
    checkbox(root, 'Launch at Login').click();
    await settle();
    expect(calls.at(-1)).toEqual(['toggle_login_item', { enabled: true }]);
    button(root, 'Get Started').click();
    await settle();
    expect(calls.slice(-2)).toEqual([
      ['update_config', { patch: { hasLaunched: true } }],
      ['close_panel', undefined],
    ]);
  });

  it('omits the launch-at-login option on platforms without that capability', async () => {
    const state = fixture();
    state.panel = 'welcome';
    state.capabilities.launchAtLogin = false;
    const { root } = await mount(state);
    expect(root.textContent).not.toContain('Launch at Login');
  });
});

describe('desktop settings', () => {
  it('lists saved overrides, including unfocused apps, and removes the selected override', async () => {
    const state = fixture();
    const slackId = 'mac:bundle:com.tinyspeck.slackmacgap';
    state.panel = 'settings';
    state.config.pinnedFormats = { [slackId]: 'slack-paste', [editor.id]: 'plain-text' };
    const { root, calls } = await mount(state);
    expect(root.textContent).toContain('Saved app overrides');
    const slack = root.querySelector(`li:has([title="${slackId}"])`) as HTMLElement;
    expect(slack.textContent).toContain('com.tinyspeck.slackmacgap');
    expect(slack.textContent).toContain('Slack');
    button(slack, 'Remove').click();
    await settle();
    expect(calls.at(-1)).toEqual(['set_app_pin', { appId: slackId, target: null }]);
    expect(root.querySelector(`[title="${slackId}"]`)).toBeNull();
    expect(root.querySelector(`[title="${editor.id}"]`)?.textContent).toBe('Editor');
  });
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
