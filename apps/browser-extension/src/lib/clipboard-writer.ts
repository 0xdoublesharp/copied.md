import type { ClipboardPayload } from '@0xdoublesharp/copied-md-converters';

export type ClipboardType = keyof ClipboardPayload;

export interface ClipboardOutcome {
  types: ClipboardType[];
  fallback: boolean;
}

export function richClipboardType(payload: ClipboardPayload) {
  if (payload['text/signal']) return 'text/signal';
  if (payload['text/html']) return 'text/html';
  return undefined;
}

export async function writeExact(payload: ClipboardPayload): Promise<ClipboardType[]> {
  const plain = payload['text/plain'];
  const signal = payload['text/signal'];
  if (signal) {
    // Chromium's copy event writes the DataTransfer format Signal Desktop reads.
    if (!import.meta.env.FIREFOX && copyWithEvent(plain, ['text/signal', signal])) {
      return ['text/plain', 'text/signal'];
    }
    throw new Error('Signal clipboard access failed.');
  }
  const html = payload['text/html'];
  const types: ClipboardType[] = html ? ['text/plain', 'text/html'] : ['text/plain'];
  try {
    if (html) {
      await navigator.clipboard.write([
        new ClipboardItem({
          'text/plain': new Blob([plain], { type: 'text/plain' }),
          'text/html': new Blob([html], { type: 'text/html' }),
        }),
      ]);
    } else {
      await navigator.clipboard.writeText(plain);
    }
    return types;
  } catch {
    if (copyWithEvent(plain, html ? ['text/html', html] : undefined)) return types;
    throw new Error(html ? 'Rich clipboard access failed.' : 'Clipboard access failed.');
  }
}

export async function writeWithFallback(payload: ClipboardPayload): Promise<ClipboardOutcome> {
  try {
    return { types: await writeExact(payload), fallback: false };
  } catch (error) {
    if (!richClipboardType(payload)) throw error;
    return { types: await writeExact({ 'text/plain': payload['text/plain'] }), fallback: true };
  }
}

function copyWithEvent(plain: string, rich?: [ClipboardType, string]): boolean {
  const previous = document.activeElement;
  const selection = document.getSelection();
  const ranges = selection
    ? Array.from({ length: selection.rangeCount }, (_, i) => selection.getRangeAt(i).cloneRange())
    : [];
  const textarea = document.createElement('textarea');
  textarea.value = plain;
  textarea.style.cssText = 'position:fixed;opacity:0;pointer-events:none;left:-9999px';
  let richWritten = false;
  const setFormats = (event: ClipboardEvent) => {
    event.preventDefault();
    if (!event.clipboardData || !rich) return;
    event.clipboardData.setData('text/plain', plain);
    event.clipboardData.setData(...rich);
    richWritten = true;
  };
  document.body.append(textarea);
  if (rich) document.addEventListener('copy', setFormats, { capture: true, once: true });
  try {
    textarea.focus();
    textarea.select();
    return document.execCommand('copy') && (!rich || richWritten);
  } catch {
    return false;
  } finally {
    document.removeEventListener('copy', setFormats, true);
    textarea.remove();
    if (previous instanceof HTMLElement) previous.focus({ preventScroll: true });
    if (selection && ranges.length) {
      selection.removeAllRanges();
      for (const range of ranges) selection.addRange(range);
    }
  }
}
