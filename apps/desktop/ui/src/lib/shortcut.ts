import type { ShortcutSpec } from './ipc';

export const SHORTCUT_HINT =
  'Press a letter or number with at least two modifiers, including Control or Command.';

export function captureShortcut(event: KeyboardEvent): ShortcutSpec | null {
  const { code, ctrlKey: ctrl, altKey: alt, shiftKey: shift, metaKey: meta } = event;
  const modifierCount = [ctrl, alt, shift, meta].filter(Boolean).length;
  if (!/^(Key[A-Z]|Digit[0-9])$/.test(code) || modifierCount < 2 || (!ctrl && !meta)) {
    return null;
  }
  return { code, ctrl, alt, shift, meta };
}

export function formatShortcut({ code, ctrl, alt, shift, meta }: ShortcutSpec) {
  return [
    ctrl && 'Ctrl',
    alt && 'Alt',
    shift && 'Shift',
    meta && (/Mac/i.test(navigator.platform) ? 'Cmd' : 'Meta'),
    code.replace(/^(Key|Digit)/, ''),
  ]
    .filter(Boolean)
    .join(' + ');
}
