import { readFileSync } from 'node:fs';
import { fileURLToPath } from 'node:url';
import { fromHtml } from 'hast-util-from-html';
import { visit } from 'unist-util-visit';
import { describe, expect, it } from 'vitest';
import { convertMarkdown, targets } from '../conversion';

const fixture = readFileSync(
  fileURLToPath(new URL('./fixtures/clipboard-fidelity.md', import.meta.url)),
  'utf8',
);

describe('destination profiles', () => {
  it.each([
    ['teams', 'Teams', 'monospace'],
    ['word', 'Word', 'Consolas'],
  ] as const)('prepares %s tables and indented code for rich paste', (target, label, font) => {
    expect(targets).toContainEqual({ id: target, label, rich: true, enabledByDefault: false });
    const rich = convertMarkdown(fixture, target).representations;
    const generic = convertMarkdown(fixture, 'rich-text').representations;
    expect(rich['text/plain']).toBe(generic['text/plain']);
    expect(rich['text/html']).not.toBe(generic['text/html']);
    expect(rich['text/html']).toContain('border-collapse: collapse');
    expect(rich['text/html']).toContain(`font-family: ${font}`);
    expect(rich['text/html']).toContain('    publish("staging")');
    expect(rich['text/html']).toContain('href="https://example.org/rollout"');
    const structure: string[] = [];
    visit(fromHtml(rich['text/html']!, { fragment: true }), 'element', ({ tagName }) => {
      if (['ol', 'ul', 'table', 'th', 'td'].includes(tagName)) structure.push(tagName);
    });
    expect(structure).toEqual(['ol', 'ul', 'ul', 'table', 'th', 'th', 'td', 'td', 'td', 'td']);
  });

  it('generates WhatsApp message markup with visible destination URLs', () => {
    expect(targets).toContainEqual({
      id: 'whatsapp',
      label: 'WhatsApp',
      rich: false,
      enabledByDefault: false,
    });
    const { representations } = convertMarkdown(fixture, 'whatsapp');
    const plain = representations['text/plain'];
    expect(Object.keys(representations)).toEqual(['text/plain']);
    expect(plain).toContain('*Release readiness*');
    expect(plain).toContain('*Ready* for review, with _one_ follow-up and ~no migration~.');
    expect(plain).toContain('1. Review the rollout plan (https://example.org/rollout).');
    expect(plain).toContain('- Watch the queue.');
    expect(plain).toContain('> Keep the previous build until checks finish.');
    expect(plain).toContain('`check --dry-run`');
    expect(plain).toContain('```if ready:\n    publish("staging")```');
  });

  it('uses readable code rows for Slack composer tables and keeps other rich content', () => {
    const { representations } = convertMarkdown(fixture, 'slack-paste');
    expect(representations['text/html']).toContain('<strong>Ready</strong>');
    expect(representations['text/html']).toContain(
      '<pre><code>Service | Status\nWeb | Ready\nWorker | Logs (https://example.org/worker)</code></pre>',
    );
    expect(representations['text/html']).not.toContain('<table');
    expect(representations['text/plain']).toContain('Service | Status\nWeb | Ready');
    expect(convertMarkdown(fixture, 'rich-text').representations['text/html']).toContain('<table>');
  });

  it('keeps raw HTML Slack table cells distinct without copying unsafe links', () => {
    const markdown =
      '<table><tr><th>First</th><th>Second</th></tr>' +
      '<tr><td>A<br>B</td><td><a href="javascript:alert(1)">Label</a></td></tr></table>';
    const { representations } = convertMarkdown(markdown, 'slack-paste');
    expect(representations['text/plain']).toBe('First | Second\nA B | Label');
    expect(representations['text/html']).toBe(
      '<pre><code>First | Second\nA B | Label</code></pre>',
    );
  });

  it('keeps Slack table captions, bare URLs, and pipes inside a cell', () => {
    const markdown =
      '<table><caption>Details</caption><tbody><tr>' +
      '<td><a href="https://example.org/bare"></a></td>' +
      '<td><a href="https://example.org/named">https://example.org/named</a></td>' +
      '<td><a href="#internal">A|B</a></td>' +
      '</tr></tbody></table>';
    const { representations } = convertMarkdown(markdown, 'slack-paste');
    expect(representations['text/plain']).toBe(
      'Details\nhttps://example.org/bare | https://example.org/named | A\\|B',
    );
    expect(representations['text/html']).toContain(
      'https://example.org/bare | https://example.org/named | A\\|B',
    );
  });

  it('does not add mismatched WhatsApp code spans around backticks', () => {
    const markdown = 'Inline: ``a`b``\n\n~~~\nFence: ```\n~~~';
    expect(convertMarkdown(markdown, 'whatsapp').representations['text/plain']).toBe(
      'Inline: a`b\n\nFence: ```',
    );
  });
});
