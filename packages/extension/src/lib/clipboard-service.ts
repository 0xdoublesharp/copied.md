import type { ClipboardPayload } from '@copied-md/converters';

export async function writeToClipboard(payload: ClipboardPayload): Promise<void> {
  // Try offscreen document approach first (Chrome)
  if (typeof chrome !== 'undefined' && chrome.offscreen) {
    await writeViaOffscreen(payload);
    return;
  }

  // Fallback: send to active tab's content script
  await writeViaContentScript(payload);
}

async function writeViaOffscreen(payload: ClipboardPayload): Promise<void> {
  // Ensure offscreen document exists
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
    // Fallback for older Chrome — try creating, ignore error if already exists
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

async function writeViaContentScript(payload: ClipboardPayload): Promise<void> {
  const [tab] = await browser.tabs.query({ active: true, currentWindow: true });
  if (tab?.id) {
    await browser.tabs.sendMessage(tab.id, {
      type: 'WRITE_CLIPBOARD',
      payload,
    });
  }
}
