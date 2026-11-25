import TurndownService from 'turndown';
import { gfm } from 'turndown-plugin-gfm';
import type { ChatSite } from './constants';

interface Reply {
  turn: Element;
  body: Element;
}

const replySelector = {
  chatgpt: '[data-message-author-role="assistant"], [data-chatgpt-search-unit-key$=":assistant"]',
  claude: '[data-testid="assistant-message"]',
};
const bodySelector = {
  chatgpt: '[data-response-body], .markdown',
  claude: '.prose',
};
const streamingSelector =
  '[data-is-streaming="true"], [data-perf-row-streaming="true"], [aria-busy="true"]';
const hiddenSelector =
  '[hidden], [aria-hidden="true"], [inert], [role="presentation"][data-app-action-timeline-scroll]';
const copySelector =
  'button:not([disabled]):is([data-testid="copy-turn-action-button"], ' +
  '[data-testid="copy-message-button"], [aria-label="Copy response"], [aria-label="Copy"])';

const markdown = new TurndownService({
  headingStyle: 'atx',
  bulletListMarker: '-',
  codeBlockStyle: 'fenced',
});
markdown.use(gfm);
markdown.addRule('signal-spoiler', {
  filter: (node) => node.nodeName === 'SPAN' && node.getAttribute('data-spoiler') === 'true',
  replacement: (content) => '<span data-spoiler="true">' + content + '</span>',
});

const omitted = [
  hiddenSelector,
  '.sr-only',
  'button, [role="button"], [role="toolbar"]',
  '[data-testid="message-actions"], [data-testid="user-message-actions"]',
  'input:not([type="checkbox"]), select, textarea, form, nav',
  'script, style, svg, iframe, object, audio, video, template',
].join(', ');

function completed(turn: Element, body: Element, site: ChatSite['id']) {
  if (turn.closest(streamingSelector) || body.querySelector(streamingSelector)) return false;
  if (
    site === 'claude' &&
    (turn.getAttribute('data-is-streaming') === 'false' ||
      turn.closest('[data-perf-row-streaming="false"]'))
  )
    return true;
  const scope = turn.closest('article[data-turn="assistant"]') ?? turn;
  return [...scope.querySelectorAll(copySelector)].some(
    (button) => !body.contains(button) && !button.closest(hiddenSelector),
  );
}

function findReplies(page: Document, site: ChatSite['id'], path: string): Reply[] {
  const chatPath = site === 'claude' ? /^\/chat\/[^/]+/ : /^\/(?:g\/[^/]+\/)?(?:c|share)\/[^/]+/;
  if (!chatPath.test(path)) return [];
  const selector = replySelector[site];
  return [...page.querySelectorAll('main ' + selector.split(', ').join(', main '))]
    .filter((turn) => !turn.parentElement?.closest(selector) && !turn.closest(hiddenSelector))
    .flatMap((turn) => {
      const body = turn.querySelector(bodySelector[site]);
      return body && !body.closest(hiddenSelector) && completed(turn, body, site)
        ? [{ turn, body }]
        : [];
    });
}

export function replyMarkdown(body: Element) {
  const content = body.cloneNode(true) as HTMLElement;
  content.querySelectorAll(omitted).forEach((item) => item.remove());
  return markdown.turndown(content);
}

type OpenPicker = (markdown: string, button: HTMLButtonElement) => () => void;

export function installReplyControls(
  page: Document,
  site: ChatSite['id'],
  path: () => string,
  openPicker: OpenPicker,
) {
  const actions = new Map<Element, { button: HTMLButtonElement; body: Element }>();
  let active: { button: HTMLButtonElement; close: () => void } | undefined;

  function close() {
    active?.close();
    active = undefined;
  }

  function sync() {
    const replies = new Map(findReplies(page, site, path()).map(({ turn, body }) => [turn, body]));
    for (const [turn, { button, body }] of actions) {
      if (replies.get(turn) === body && button.parentElement === turn) continue;
      if (active?.button === button) close();
      button.remove();
      actions.delete(turn);
    }
    for (const [turn, body] of replies) {
      if (actions.has(turn)) continue;
      const button = page.createElement('button');
      button.type = 'button';
      button.textContent = 'Copy as…';
      button.dataset.copiedMdReply = '';
      button.style.cssText =
        'margin-block:6px;padding:4px 8px;border:1px solid currentColor;border-radius:6px;' +
        'background:transparent;color:inherit;font:inherit;font-size:0.875rem;cursor:pointer';
      button.setAttribute('aria-expanded', 'false');
      button.addEventListener('click', () => {
        const toggle = active?.button === button && button.getAttribute('aria-expanded') === 'true';
        close();
        if (toggle || !completed(turn, body, site)) return;
        const source = replyMarkdown(body);
        if (!source.trim()) return;
        active = { button, close: openPicker(source, button) };
      });
      turn.append(button);
      actions.set(turn, { button, body });
    }
  }

  const observer = new MutationObserver(sync);
  observer.observe(page.body, {
    childList: true,
    subtree: true,
    attributes: true,
    attributeFilter: [
      'aria-busy',
      'aria-hidden',
      'aria-label',
      'data-is-streaming',
      'data-perf-row-streaming',
      'disabled',
      'hidden',
      'inert',
    ],
  });
  sync();
  return () => {
    observer.disconnect();
    close();
    for (const { button } of actions.values()) button.remove();
  };
}
