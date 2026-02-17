export default defineContentScript({
  matches: ['<all_urls>'],
  main() {
    browser.runtime.onMessage.addListener((message, _sender, sendResponse) => {
      if (message.type === 'WRITE_CLIPBOARD') {
        handleClipboardWrite(message.payload)
          .then(() => sendResponse({ success: true }))
          .catch((err) => sendResponse({ success: false, error: String(err) }));
        return true;
      }

      if (message.type === 'COPY_SUCCESS') {
        showToast(message.platform, message.truncated);
      }
    });
  },
});

async function handleClipboardWrite(payload: {
  'text/plain': string;
  'text/html'?: string;
}): Promise<void> {
  // Use ClipboardItem API for multi-format clipboard write
  if (payload['text/html'] && typeof ClipboardItem !== 'undefined') {
    try {
      const item = new ClipboardItem({
        'text/plain': new Blob([payload['text/plain']], { type: 'text/plain' }),
        'text/html': new Blob([payload['text/html']], { type: 'text/html' }),
      });
      await navigator.clipboard.write([item]);
      return;
    } catch {
      // Fall through to text-only
    }
  }

  // Fallback: plain text via Clipboard API
  try {
    await navigator.clipboard.writeText(payload['text/plain']);
  } catch {
    // Last resort: temporary textarea + execCommand
    const textarea = document.createElement('textarea');
    textarea.value = payload['text/plain'];
    textarea.style.position = 'fixed';
    textarea.style.opacity = '0';
    textarea.style.left = '-9999px';
    document.body.appendChild(textarea);
    textarea.select();
    document.execCommand('copy');
    document.body.removeChild(textarea);
  }
}

function showToast(platform: string, truncated: boolean) {
  // Remove any existing toast
  const existing = document.getElementById('copied-md-toast');
  if (existing) existing.remove();

  const toast = document.createElement('div');
  toast.id = 'copied-md-toast';
  toast.textContent = truncated
    ? `Copied for ${platform} (truncated)`
    : `Copied for ${platform}`;

  Object.assign(toast.style, {
    position: 'fixed',
    bottom: '20px',
    right: '20px',
    padding: '12px 20px',
    background: '#1a1a1a',
    color: '#fff',
    borderRadius: '8px',
    zIndex: '2147483647',
    fontSize: '14px',
    fontFamily: 'system-ui, -apple-system, sans-serif',
    boxShadow: '0 4px 12px rgba(0, 0, 0, 0.3)',
    transition: 'opacity 0.3s ease',
    opacity: '0',
  });

  document.body.appendChild(toast);

  // Fade in
  requestAnimationFrame(() => {
    toast.style.opacity = '1';
  });

  // Fade out and remove
  setTimeout(() => {
    toast.style.opacity = '0';
    setTimeout(() => toast.remove(), 300);
  }, 2200);
}
