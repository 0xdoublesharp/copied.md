import { afterEach, expect, it, vi } from 'vitest';
import { installReplyControls, replyMarkdown } from '@/lib/chat-replies';

let dispose: (() => void) | undefined;
const buttons = () => [
  ...document.querySelectorAll<HTMLButtonElement>('button[data-copied-md-reply]'),
];

afterEach(() => {
  dispose?.();
  dispose = undefined;
  document.body.replaceChildren();
});

it('copies just the selected completed ChatGPT reply and waits for streaming to end', async () => {
  document.body.innerHTML = `
    <main>
      <div data-message-author-role="user"><div class="markdown">User prompt</div></div>
      <article data-turn="assistant">
        <div data-message-author-role="assistant">
          <div class="markdown"><p>First <b>answer</b></p><button>Copy code</button></div>
        </div>
        <button data-testid="copy-turn-action-button">Native copy</button>
      </article>
      <div data-chatgpt-search-unit-key="test:assistant" data-is-streaming="true">
        <div data-response-body><p>Second answer</p></div>
        <button aria-label="Copy response">Native copy</button>
      </div>
      <div data-message-author-role="assistant"><div class="markdown">No copy marker</div></div>
      <div data-message-author-role="assistant">
        <div class="markdown">Only code copy <button aria-label="Copy">Copy code</button></div>
        <button aria-label="Copy" aria-hidden="true">Hidden copy</button>
        <div data-message-author-role="assistant"><div class="markdown">Nested assistant</div>
          <button data-testid="copy-turn-action-button" hidden>Nested copy</button>
        </div>
      </div>
      <div data-message-author-role="assistant" hidden><div class="markdown">Hidden</div>
        <button aria-label="Copy response">Hidden copy</button>
      </div>
    </main>`;
  const close = vi.fn();
  const open = vi.fn((_markdown: string, button: HTMLButtonElement) => {
    button.setAttribute('aria-expanded', 'true');
    return close;
  });
  dispose = installReplyControls(document, 'chatgpt', () => '/c/test', open);
  expect(buttons()).toHaveLength(1);
  buttons()[0].click();
  expect(open).toHaveBeenLastCalledWith('First **answer**', buttons()[0]);
  buttons()[0].click();
  expect(close).toHaveBeenCalledOnce();
  expect(open).toHaveBeenCalledOnce();

  const streaming = document.querySelector('[data-is-streaming]')!;
  streaming.setAttribute('data-is-streaming', 'false');
  await vi.waitFor(() => expect(buttons()).toHaveLength(2));
  buttons()[1].click();
  expect(open).toHaveBeenLastCalledWith('Second answer', buttons()[1]);
  streaming.setAttribute('data-is-streaming', 'true');
  buttons()[1].click();
  expect(open).toHaveBeenCalledTimes(2);
  await vi.waitFor(() => expect(buttons()).toHaveLength(1));
  expect(close).toHaveBeenCalledTimes(2);
});

it('tracks recycled Claude DOM without exposing controls outside a chat route', async () => {
  document.body.innerHTML = `
    <main>
      <div data-perf-row-streaming="false">
        <div data-testid="assistant-message"><div class="prose">Original reply</div></div>
      </div>
      <div data-testid="assistant-message">
        <div class="prose" aria-busy="true">Streaming</div>
        <button aria-label="Copy" disabled>Disabled copy</button>
      </div>
      <div data-testid="assistant-message" data-is-streaming="false"><div class="prose"></div></div>
      <div data-testid="assistant-message"><div class="prose">Fallback reply</div>
        <button aria-label="Copy">Native copy</button></div>
    </main>`;
  const close = vi.fn();
  const open = vi.fn(() => close);
  let path = '/projects';
  dispose = installReplyControls(document, 'claude', () => path, open);
  expect(buttons()).toHaveLength(0);
  path = '/chat/example';
  document.querySelector('main')!.append(document.createElement('div'));
  await vi.waitFor(() => expect(buttons()).toHaveLength(3));
  buttons()[1].click();
  expect(open).not.toHaveBeenCalled();
  buttons()[0].click();
  expect(open).toHaveBeenLastCalledWith('Original reply', buttons()[0]);
  const body = document.querySelector('.prose')!;
  const replacement = body.cloneNode(false);
  replacement.textContent = 'Replacement';
  body.replaceWith(replacement);
  await vi.waitFor(() => expect(close).toHaveBeenCalledOnce());
  buttons()[0].click();
  expect(open).toHaveBeenLastCalledWith('Replacement', buttons()[0]);
  const removed = buttons()[0];
  removed.remove();
  await vi.waitFor(() => expect(buttons()[0]).not.toBe(removed));
  buttons()[2].click();
  expect(open).toHaveBeenLastCalledWith('Fallback reply', buttons()[2]);
  dispose?.();
  dispose = undefined;
  expect(buttons()).toHaveLength(0);
  expect(close).toHaveBeenCalledTimes(3);
});

it('converts semantic reply markup including tables, lists, links, and code', () => {
  const reply = document.createElement('div');
  reply.innerHTML = `
    <h2>Rollout</h2><p><strong>Ready</strong> <a href="./notes">Notes</a></p>
    <ul><li>Parent<ul><li>Child</li></ul></li></ul>
    <pre><code class="language-js">line();
  indented();</code></pre>
    <table><thead><tr><th>Name</th><th>Status</th></tr></thead>
      <tbody><tr><td>A</td><td>Done</td></tr></tbody></table>
    <p hidden>Private aside</p><span class="sr-only">Screen reader heading</span>
    <div role="toolbar">Surrounding controls</div><form>Editor</form><script>Ignored</script>`;
  const markdown = replyMarkdown(reply);
  expect(markdown).toContain('## Rollout');
  expect(markdown).toContain('**Ready** [Notes](./notes)');
  expect(markdown).toMatch(/- +Parent\n +[-] +Child/);
  expect(markdown).toContain('```js\nline();\n  indented();\n```');
  expect(markdown).toMatch(/\| Name \| Status \|\n\| --- \| --- \|\n\| A \| Done \|/);
  expect(markdown).not.toMatch(/Private aside|Screen reader|Surrounding|Editor|Ignored/);
  expect(reply.querySelector('script')).not.toBeNull();
});
