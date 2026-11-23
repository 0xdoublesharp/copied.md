import { writeExact } from './clipboard-writer';

chrome.runtime.onMessage.addListener((message, _sender, sendResponse) => {
  if (message.type !== 'WRITE_CLIPBOARD' || message.destination !== 'offscreen') return;
  writeExact(message.payload)
    .then((types) => sendResponse({ types }))
    .catch((error: unknown) => sendResponse({ error: String(error) }));
  return true;
});
