import type { FormatIconId } from '@0xdoublesharp/copied-md-shared-icons';
import { invoke } from '@tauri-apps/api/core';
import { listen, type UnlistenFn } from '@tauri-apps/api/event';

export type CopyTarget = FormatIconId;
export type Columns = 3 | 4 | 5;
export type ShortcutMode = 'copy-and-pick' | 'picker' | 'copy-only';
export type Panel = 'picker' | 'settings' | 'app-pin' | 'shortcut';
export type LoginItemStatus = 'disabled' | 'enabled' | 'requires-approval' | 'unsupported';

export interface ShortcutSpec {
  code: string;
  ctrl: boolean;
  alt: boolean;
  shift: boolean;
  meta: boolean;
}

export interface TargetDefinition {
  id: CopyTarget;
  label: string;
  rich: boolean;
  enabledByDefault?: boolean;
}

export interface ConfigV2 {
  schemaVersion: 2;
  shortcut: ShortcutSpec;
  shortcutEnabled: boolean;
  shortcutMode: ShortcutMode;
  lastTarget?: CopyTarget;
  formatVisibility: Partial<Record<CopyTarget, boolean>>;
  pickerColumns: Columns;
  pinnedFormats: Record<string, CopyTarget>;
  hasLaunched: boolean;
}

export type ConfigPatch = Partial<
  Omit<ConfigV2, 'schemaVersion' | 'pinnedFormats' | 'lastTarget'>
> & {
  lastTarget?: CopyTarget | null;
  pinnedFormats?: Record<string, CopyTarget | null>;
};

export interface PickerOpenRequest {
  opId: number;
  visibleTargets: TargetDefinition[];
  columns: Columns;
  selected: CopyTarget;
  copied?: { target: CopyTarget; label: string; warning?: string };
  suggested?: CopyTarget;
}

export interface DesktopState {
  config: ConfigV2;
  targets: TargetDefinition[];
  activeApp: { id: string; name: string } | null;
  capabilities: {
    backgroundClipboard: boolean;
    globalShortcuts: 'direct' | 'portal' | 'unsupported';
    activeAppDetection: boolean;
    launchAtLogin: boolean;
  };
  loginItemStatus: LoginItemStatus;
  currentPicker: PickerOpenRequest | null;
  panel: Panel | null;
  shortcutRegistered: boolean;
  shortcutFailure: string | null;
}

export interface MarkdownConversion {
  schemaVersion: 1;
  target: CopyTarget;
  representations: { 'text/plain': string } & Partial<
    Record<'text/html' | 'text/rtf' | 'text/signal', string>
  >;
  metrics: {
    sourceCodeUnits: number;
    outputLength: number;
    unit: 'code-points' | 'x-weighted-characters';
    limit?: number;
  };
  diagnostics: { code: string; severity: 'warning' | 'info'; message: string }[];
}

export interface DesktopBridge {
  invoke<T>(command: string, args?: Record<string, unknown>): Promise<T>;
  listen<T>(event: string, callback: (payload: T) => void): Promise<UnlistenFn>;
}

const tauriBridge: DesktopBridge = {
  invoke,
  listen<T>(event: string, callback: (payload: T) => void) {
    return listen<T>(event, ({ payload }) => callback(payload), { target: 'picker' });
  },
};

export function createDesktopApi(bridge: DesktopBridge = tauriBridge) {
  return {
    getState: () => bridge.invoke<DesktopState>('get_desktop_state'),
    choose: (opId: number, target: CopyTarget | null) =>
      bridge.invoke<void>('picker_choose', { opId, target }),
    dismiss: (opId: number) => bridge.invoke<void>('picker_dismiss', { opId }),
    closePanel: () => bridge.invoke<void>('close_panel'),
    updateConfig: (patch: ConfigPatch) => bridge.invoke<ConfigV2>('update_config', { patch }),
    setAppPin: (appId: string, target: CopyTarget | null) =>
      bridge.invoke<ConfigV2>('set_app_pin', { appId, target }),
    beginRecording: () => bridge.invoke<void>('begin_shortcut_recording'),
    commitRecording: (spec: ShortcutSpec) =>
      bridge.invoke<ConfigV2>('commit_shortcut_recording', { spec }),
    cancelRecording: () => bridge.invoke<void>('cancel_shortcut_recording'),
    resetShortcut: () => bridge.invoke<ConfigV2>('reset_shortcut'),
    toggleShortcut: (enabled: boolean) => bridge.invoke<ConfigV2>('toggle_shortcut', { enabled }),
    toggleLogin: (enabled: boolean) =>
      bridge.invoke<LoginItemStatus>('toggle_login_item', { enabled }),
    preview: (
      markdown: string,
      target: CopyTarget,
      sourceUrl: string | null,
      decorativeUnicode: boolean,
    ) =>
      bridge.invoke<MarkdownConversion>('preview_markdown', {
        markdown,
        target,
        sourceUrl,
        decorativeUnicode,
      }),
    onPickerOpen: (callback: (request: PickerOpenRequest) => void) =>
      bridge.listen('picker:open', callback),
    onPanelOpen: (callback: (panel: Panel) => void) => bridge.listen('desktop:panel', callback),
  };
}

export type DesktopApi = ReturnType<typeof createDesktopApi>;
export const desktopApi = createDesktopApi();

export function visibleTargets(state: Pick<DesktopState, 'config' | 'targets'>) {
  return state.targets.filter(
    (target) => state.config.formatVisibility[target.id] ?? target.enabledByDefault !== false,
  );
}
