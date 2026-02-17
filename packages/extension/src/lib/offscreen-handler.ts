// This script runs inside the offscreen document.
// It listens for clipboard write messages from the background service worker.

chrome.runtime.onMessage.addListener((message, _sender, sendResponse) => {
  if (message.type === 'WRITE_CLIPBOARD') {
    handleClipboardWrite(message.payload)
      .then(() => sendResponse({ success: true }))
      .catch((err: unknown) => sendResponse({ success: false, error: String(err) }));
    return true; // Keep the message channel open for async response
  }
});

async function handleClipboardWrite(payload: {
  'text/plain': string;
  'text/html'?: string;
}): Promise<void> {
  // Use ClipboardItem API for multi-format clipboard write
  if (payload['text/html'] && typeof ClipboardItem !== 'undefined') {
    const item = new ClipboardItem({
      'text/plain': new Blob([payload['text/plain']], { type: 'text/plain' }),
      'text/html': new Blob([payload['text/html']], { type: 'text/html' }),
    });
    await navigator.clipboard.write([item]);
    return;
  }

  // Fallback: plain text only
  try {
    await navigator.clipboard.writeText(payload['text/plain']);
  } catch {
    // Last resort: textarea + execCommand
    const textarea = document.getElementById('clipboard-area') as HTMLTextAreaElement;
    textarea.value = payload['text/plain'];
    textarea.select();
    document.execCommand('copy');
  }
}
