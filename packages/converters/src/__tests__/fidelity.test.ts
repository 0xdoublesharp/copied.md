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

  it('provides Telegram RTF with labeled links and a spaced plain-text fallback', () => {
    const report = [
      '# Project update',
      '',
      '- **Shipped.** See [the rollout][r].',
      '- **Next.** Check `main` and the *draft*.',
      '',
      '[r]: https://example.com/releases/1',
    ].join('\n');
    const { representations } = convertMarkdown(report, 'telegram');
    expect(representations['text/rtf']).toMatch(/\{\\rtf1/);
    expect(representations['text/rtf']).toContain('{\\b Shipped.}');
    expect(representations['text/rtf']).toContain('HYPERLINK "https://example.com/releases/1"');
    expect(representations['text/rtf']).toContain('{\\fldrslt the rollout}');
    expect(representations['text/plain']).toBe(
      'Project update\n\n• Shipped. See the rollout (https://example.com/releases/1).\n\n• Next. Check main and the draft.',
    );
  });

  it('keeps Telegram RTF literal text and URL fields separate', () => {
    const { representations } = convertMarkdown(
      '**Café ☕** `{x} \\b` [local](javascript:bad) [safe](<https://example.com/café?q="x">)',
      'telegram',
    );
    const rtf = representations['text/rtf'];
    expect(rtf).toContain('Caf\\u233? \\u9749?');
    expect(rtf).toContain('\\{x\\} \\\\b');
    expect(rtf).toContain('HYPERLINK "https://example.com/caf%C3%A9?q=%22x%22"');
    expect(rtf).not.toContain('javascript:');
  });

  it('emits visible Telegram HTML bullets, spacing, and bold headings', () => {
    const markdown = [
      '# Project update',
      '',
      '3. **Shipped.** See [the rollout](https://example.com).',
      '   - Check `main`.',
      '4. **Next.** Review the *draft*.',
    ].join('\n');
    const html = convertMarkdown(markdown, 'telegram').representations['text/html'];
    expect(html).toBe(
      '<strong>Project update</strong><br><br>3. <strong>Shipped.</strong> See <a href="https://example.com">the rollout</a>.' +
        '<br>&nbsp;&nbsp;&nbsp;• Check <code>main</code>.<br><br>4. <strong>Next.</strong> Review the <em>draft</em>.',
    );
  });

  it('keeps code block indentation without allowing HTML injection in Telegram HTML', () => {
    const markdown = '```\n  <script>\n\t0\n```\n\n[the draft](<https://example.com/?q="x"&y=z>)';
    const representations = convertMarkdown(markdown, 'telegram').representations;
    expect(representations['text/html']).toContain('<pre>  &lt;script&gt;&#10;\t0</pre>');
    expect(representations['text/html']).toContain(
      'href="https://example.com/?q=&quot;x&quot;&amp;y=z"',
    );
    expect(representations['text/rtf']).not.toContain('\\tab 0');
  });

  it('keeps quotes and code intact next to Telegram HTML lists and paragraphs', () => {
    const markdown = [
      'Intro',
      '',
      '- Item',
      '  > First',
      '  >',
      '  > Second',
      '',
      '  ```',
      '    code',
      '  ```',
      '',
      'After',
    ].join('\n');
    expect(convertMarkdown(markdown, 'telegram').representations['text/html']).toBe(
      'Intro<br><br>• Item<br><blockquote>First<br><br>Second</blockquote><br><pre>  code</pre><br>After',
    );
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
