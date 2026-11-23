// @vitest-environment node

import { afterEach, expect, it, vi } from 'vitest';

afterEach(() => vi.unstubAllGlobals());

it('decodes named and numeric entities in an extension service worker', async () => {
  vi.stubGlobal('document', undefined);
  await import('@/lib/dom-polyfill');
  const element = document.createElement('i');
  for (const [html, expected] of [
    ['&amp;', '&'],
    ['&#65;', 'A'],
    ['&#x1F680;', '🚀'],
    ['&copiedMdUnknown;', '&copiedMdUnknown;'],
    ['<i>plain</i>', '<i>plain</i>'],
  ]) {
    element.innerHTML = html;
    expect(element.textContent).toBe(expected);
  }
});
