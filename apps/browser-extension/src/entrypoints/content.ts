import { writeExact } from '@/lib/clipboard-writer';

export default defineContentScript({
  registration: 'runtime',
  noScriptStartedPostMessage: true,
  main() {
    browser.runtime.onMessage.addListener((message, _sender, sendResponse) => {
      if (message.type === 'READ_SELECTION') {
        const text = readSelection();
        sendResponse({ text: text ?? '', blocked: text === null, sourceUrl: location.href });
        return;
      }
      if (message.type === 'COPY_STATUS') {
        showToast(message.text);
        return;
      }
      if (message.type !== 'WRITE_CLIPBOARD' || message.destination !== 'content') return;
      writeExact(message.payload)
        .then((types) => sendResponse({ types }))
        .catch((error: unknown) => sendResponse({ error: String(error) }));
      return true;
    });
  },
});

function readSelection(): string | null {
  const element = document.activeElement;
  if (element instanceof HTMLInputElement || element instanceof HTMLTextAreaElement) {
    if (element instanceof HTMLInputElement && element.type === 'password') return null;
    if (element.selectionStart !== null && element.selectionEnd !== null) {
      return element.value.slice(element.selectionStart, element.selectionEnd);
    }
  }
  return document.getSelection()?.toString() ?? '';
}

function showToast(message: string) {
  // Remove any existing toast
  const existing = document.getElementById('copied-md-toast');
  if (existing) existing.remove();

  const toast = document.createElement('div');
  toast.id = 'copied-md-toast';
  toast.setAttribute('role', 'status');
  toast.textContent = message;

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
