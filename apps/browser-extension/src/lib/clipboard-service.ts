import type { ClipboardPayload } from '@0xdoublesharp/copied-md-converters';
import { richClipboardType, type ClipboardOutcome, type ClipboardType } from './clipboard-writer';

interface WriteResponse {
  types?: ClipboardType[];
  error?: string;
}

export interface SelectionSource {
  text: string;
  blocked?: boolean;
  sourceUrl?: string;
}

let queued = Promise.resolve<unknown>(undefined);
let creating: Promise<void> | undefined;

export function writeToClipboard(
  payload: ClipboardPayload,
  tabId?: number,
  frameId?: number,
): Promise<ClipboardOutcome> {
  const result = queued.then(() => routeCopy(payload, tabId, frameId));
  queued = result.catch(() => undefined);
  return result;
}

async function routeCopy(
  payload: ClipboardPayload,
  tabId?: number,
  frameId?: number,
): Promise<ClipboardOutcome> {
  const attempts = [payload];
  if (richClipboardType(payload)) attempts.push({ 'text/plain': payload['text/plain'] });
  let lastError: unknown;
  for (const [index, representation] of attempts.entries()) {
    if (tabId !== undefined) {
      try {
        const response = (await browser.tabs.sendMessage(
          tabId,
          {
            type: 'WRITE_CLIPBOARD',
            destination: 'content',
            payload: representation,
          },
          frameId === undefined ? undefined : { frameId },
        )) as WriteResponse | undefined;
        return { types: validate(response, representation), fallback: index > 0 };
      } catch (error) {
        lastError = error;
      }
    }
    if (typeof chrome !== 'undefined' && chrome.offscreen) {
      try {
        await ensureOffscreen();
        const response = (await chrome.runtime.sendMessage({
          type: 'WRITE_CLIPBOARD',
          destination: 'offscreen',
          payload: representation,
        })) as WriteResponse | undefined;
        return { types: validate(response, representation), fallback: index > 0 };
      } catch (error) {
        lastError = error;
      }
    }
  }
  throw lastError ?? new Error('Clipboard write failed: no available clipboard context');
}

function validate(response: WriteResponse | undefined, payload: ClipboardPayload): ClipboardType[] {
  const types = response?.types;
  const richType = richClipboardType(payload);
  if (!types?.includes('text/plain') || (richType && !types.includes(richType))) {
    throw new Error(response?.error ?? 'The clipboard did not accept the requested format.');
  }
  return types;
}

async function ensureOffscreen() {
  creating ??= (async () => {
    const path = 'offscreen.html';
    const contexts = await chrome.runtime.getContexts({
      contextTypes: ['OFFSCREEN_DOCUMENT' as chrome.runtime.ContextType],
      documentUrls: [chrome.runtime.getURL(path)],
    });
    if (contexts.length > 0) return;
    await chrome.offscreen.createDocument({
      url: path,
      reasons: ['CLIPBOARD' as chrome.offscreen.Reason],
      justification: 'Write converted Markdown to the clipboard',
    });
  })().finally(() => {
    creating = undefined;
  });
  await creating;
}
