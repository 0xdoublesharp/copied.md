import { describe, expect, it } from 'vitest';
import { convertMarkdown, getConverter, Platform } from '../index';

const copy = (platform: Platform, markdown: string) => getConverter(platform).convert(markdown);

describe('Markdown fidelity', () => {
  it('resolves reference links and retains footnotes in text', () => {
    const result = copy(
      Platform.SMS,
      'Read [the report][r].[^n]\n\n[r]: https://example.com\n[^n]: A note.',
    );
    expect(result.payload['text/plain']).toContain('the report (https://example.com)');
    expect(result.payload['text/plain']).toContain('A note.');
  });

  it('writes matching HTML footnote links and anchors', () => {
    const result = copy(Platform.HTML, 'Footnote.[^n]\n\n[^n]: Notes.');
    const html = result.payload['text/html'] ?? '';
    const fragments = [...html.matchAll(/href="#([^"]+)"/g)];
    expect(fragments).toHaveLength(2);
    for (const fragment of fragments) expect(html).toContain(`id="${fragment[1]}"`);
  });

  it('preserves ordered list starts and nesting in Reddit', () => {
    expect(copy(Platform.Reddit, '3. Parent\n   - Child\n4. Next').payload['text/plain']).toBe(
      '3. Parent\n   - Child\n4. Next',
    );
  });

  it('leaves ordinary Telegram prose intact', () => {
    const prose = 'Plan - Review costs 2. Next we ship.';
    expect(copy(Platform.Telegram, prose).payload['text/plain']).toBe(prose);
  });

  it('pairs rich HTML with readable text and handles unsafe or remote content', () => {
    const markdown =
      '**Bold** [local](custom:ignored) ![Illustration](https://example.com/image.png)';
    for (const target of ['rich-text', 'slack-paste', 'telegram'] as const) {
      const result = convertMarkdown(markdown, target);
      expect(result.representations['text/plain']).toContain('Bold local Illustration');
      expect(result.representations['text/html']).toContain('<strong>Bold</strong>');
      expect(result.representations['text/html']).not.toMatch(/<img|custom:/);
      expect(result.diagnostics.map(({ code }) => code)).toContain('unsupported-url');
    }
  });

  it('keeps Slack API markup separate from clipboard paste', () => {
    const markdown = 'Read [the report](https://example.com).';
    expect(convertMarkdown(markdown, 'slack-paste').representations['text/plain']).toBe(
      'Read the report (https://example.com).',
    );
    expect(convertMarkdown(markdown, 'slack-api').representations['text/plain']).toBe(
      'Read <https://example.com|the report>.',
    );
  });

  it('counts X text by platform weights without removing content', () => {
    const plain = '漢'.repeat(140) + ' https://example.com';
    const result = convertMarkdown(plain, 'twitter');
    expect(result.representations['text/plain']).toBe(plain);
    expect(result.metrics).toMatchObject({ outputLength: 304, limit: 280 });
    expect(result.diagnostics.map(({ code }) => code)).toContain('length-exceeded');
  });
});
