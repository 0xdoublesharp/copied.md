import type { Element } from 'hast';
import { htmlText } from './document';

export const spoilerOpen = '<span data-spoiler="true">';
const placeholder = '[spoiler omitted]';

export function signalSpanOpen(node: Element): string {
  return String(node.properties.dataSpoiler).toLowerCase() === 'true' ? spoilerOpen : '<span>';
}

export function signalSpoilerFallback(html: string) {
  if (!html.includes(spoilerOpen)) return undefined;

  // Track literal span boundaries before parsing HTML. A parser can implicitly close
  // a malformed span when it encounters crossing Markdown emphasis.
  const openTags: string[] = [];
  let spoilerDepth = 0;
  let malformed = false;
  let cursor = 0;
  let visible = '';
  for (const match of html.matchAll(/<(\/?)(span|strong|em|s)(?: [^>]*)?>/g)) {
    const [tag, closing, name] = match;
    if (closing) {
      const opening = openTags.pop();
      if (spoilerDepth > 0 && opening !== name) malformed = true;
    } else {
      openTags.push(name);
    }

    if (name !== 'span') continue;
    if (spoilerDepth > 0) {
      spoilerDepth += closing ? -1 : 1;
      if (spoilerDepth === 0) cursor = match.index + tag.length;
    } else if (tag === spoilerOpen) {
      visible += html.slice(cursor, match.index) + placeholder;
      spoilerDepth = 1;
    }
  }
  if (spoilerDepth === 0) visible += html.slice(cursor);
  return { plain: htmlText(visible), malformed: malformed || spoilerDepth > 0 };
}
