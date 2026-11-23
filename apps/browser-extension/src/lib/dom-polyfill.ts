// Minimal DOM polyfill for service worker environments.
// The `decode-named-character-reference` package (used by micromark/remark)
// calls `document.createElement("i")` at module load time to decode HTML
// entities via innerHTML/textContent. This shim provides just enough DOM
// surface for that to work in a service worker where `document` is undefined.

import { characterEntities } from 'character-entities';

if (typeof document === 'undefined') {
  const createElement = () => {
    let html = '';
    return {
      set innerHTML(value: string) {
        html = value;
      },
      get textContent(): string {
        const match = html.match(/^&([a-zA-Z][a-zA-Z0-9]*);$/);
        if (match) {
          const entity = characterEntities[match[1]];
          if (entity) return entity;
        }
        // Numeric character references
        const numMatch = html.match(/^&#([0-9]+);$/);
        if (numMatch) {
          return String.fromCodePoint(Number(numMatch[1]));
        }
        const hexMatch = html.match(/^&#x([0-9a-fA-F]+);$/);
        if (hexMatch) {
          return String.fromCodePoint(parseInt(hexMatch[1], 16));
        }
        return html;
      },
    };
  };

  (globalThis as Record<string, unknown>).document = {
    createElement,
  };
}
