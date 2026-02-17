import type { ClipboardPayload } from '@copied-md/converters';

export interface ClipboardWriteMessage {
  type: 'WRITE_CLIPBOARD';
  payload: ClipboardPayload;
  platform: string;
  truncated: boolean;
}

/**
 * Write to clipboard via the content script running in the active tab.
 * The content script has page focus and can use the Clipboard API reliably.
 * Falls back to the offscreen document if content script is unavailable.
 */
export async function writeToClipboard(
  payload: ClipboardPayload,
  tabId: number | undefined,
  platform: string,
  truncated: boolean,
): Promise<void> {
  // Primary: send to the content script in the active tab
  if (tabId) {
    try {
      await browser.tabs.sendMessage(tabId, {
        type: 'WRITE_CLIPBOARD',
        payload,
        platform,
        truncated,
      } satisfies ClipboardWriteMessage);
      return;
    } catch {
      // Content script not available on this page — fall through to offscreen
    }
  }

  // Fallback: offscreen document (Chrome only, for pages without content scripts)
  if (typeof chrome !== 'undefined' && chrome.offscreen) {
    await writeViaOffscreen(payload);
  }
}

async function writeViaOffscreen(payload: ClipboardPayload): Promise<void> {
  if (chrome.runtime.getContexts) {
    const contexts = await chrome.runtime.getContexts({
      contextTypes: ['OFFSCREEN_DOCUMENT' as chrome.runtime.ContextType],
    });
    if (contexts.length === 0) {
      await chrome.offscreen.createDocument({
        url: 'offscreen.html',
        reasons: ['CLIPBOARD' as chrome.offscreen.Reason],
        justification: 'Write converted markdown to clipboard',
      });
    }
  } else {
    try {
      await chrome.offscreen.createDocument({
        url: 'offscreen.html',
        reasons: ['CLIPBOARD' as chrome.offscreen.Reason],
        justification: 'Write converted markdown to clipboard',
      });
    } catch {
      // Document already exists
    }
  }

  await chrome.runtime.sendMessage({
    type: 'WRITE_CLIPBOARD',
    payload,
  });
}
