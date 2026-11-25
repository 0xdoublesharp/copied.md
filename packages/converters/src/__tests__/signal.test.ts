import { describe, expect, it } from 'vitest';
import { convertMarkdown, resolveTarget, targets } from '../conversion';

const signal = 'signal';

describe('Signal Desktop', () => {
  it('advertises a rich copy target and renders the styles supported by the composer', () => {
    expect(resolveTarget(signal)).toBe('signal');
    expect(targets).toContainEqual({ id: 'signal', label: 'Signal', rich: true });
    const result = convertMarkdown(
      '# Update\n\n***Ready***, ~~old~~, `a < b` and <span data-spoiler="true">**surprise**</span> 🎉',
      signal,
    );
    expect(result.representations).toEqual({
      'text/plain': 'Update\n\nReady, old, a < b and [spoiler omitted] 🎉',
      'text/signal':
        '<strong>Update</strong><br><br><em><strong>Ready</strong></em>, <s>old</s>, ' +
        '<span data-monospace="true">a &lt; b</span> and ' +
        '<span data-spoiler="true"><strong>surprise</strong></span> 🎉',
    });
  });

  it('keeps visible URLs, code indentation, and readable block boundaries', () => {
    const result = convertMarkdown(
      '1. **Read** [the notes](./notes)\n2. ![Diagram](https://example.org/figure)\n\n' +
        '> A quote\n\n```\na\n  <b>\n```',
      signal,
      { sourceUrl: 'https://example.org/docs/page' },
    );
    expect(result.representations['text/plain']).toBe(
      '1. Read the notes (https://example.org/docs/notes)\n2. Diagram (https://example.org/figure)\n\n' +
        '> A quote\n\na\n  <b>',
    );
    expect(result.representations['text/signal']).toBe(
      '1. <strong>Read</strong> the notes (https://example.org/docs/notes)<br>' +
        '2. Diagram (https://example.org/figure)<br><br>&gt; A quote<br><br>' +
        '<span data-monospace="true">a<br>  &lt;b&gt;</span>',
    );
  });

  it('only emits safe markup and keeps explicitly nested spans inside spoilers', () => {
    const result = convertMarkdown(
      'A <span data-spoiler="true"><span onclick="bad()">hidden</span> again</span> ' +
        '[label](javascript:alert) <img src="bad" onerror="bad()" alt="image">',
      signal,
    );
    expect(result.representations['text/signal']).toBe(
      'A <span data-spoiler="true"><span>hidden</span> again</span> label image',
    );
    expect(result.diagnostics).toEqual(
      expect.arrayContaining([expect.objectContaining({ code: 'unsupported-url' })]),
    );
    expect(result.representations['text/plain']).toBe('A [spoiler omitted] label image');
  });

  it('omits spoilers in HTML blocks and keeps nearby public text', () => {
    const { representations, diagnostics } = convertMarkdown(
      '<div>Before <span data-spoiler="true">hidden\ntext</span> after.</div>',
      signal,
    );
    expect(representations['text/plain']).toBe('Before [spoiler omitted] after.');
    expect(representations['text/signal']).toContain(
      '<span data-spoiler="true">hidden<br>text</span>',
    );
    expect(diagnostics).toContainEqual({
      code: 'spoiler-omitted',
      severity: 'info',
      message: 'Plain-text paste omits Signal spoilers.',
    });
  });

  it('recognizes a spoiler attribute with HTML quoting and casing variations', () => {
    const { representations } = convertMarkdown(
      "Before <SPAN class=x DATA-SPOILER='TRUE'>private</SPAN> after",
      signal,
    );
    expect(representations['text/plain']).toBe('Before [spoiler omitted] after');
    expect(representations['text/signal']).toBe(
      'Before <span data-spoiler="true">private</span> after',
    );
  });

  it.each([
    'Before <span data-spoiler="true">private\n\ntext',
    'Before <span data-spoiler="true" private text',
    'Before <span class=x data-spoiler="true>private text',
    '<span data-spoiler="true">\nprivate\n\ntext',
    'Before **<span data-spoiler="true">private** text</span>',
    'Before <span data-spoiler="true">**private</span> text**',
  ])('fails closed for a malformed spoiler (%s)', (markdown) => {
    const { representations, diagnostics } = convertMarkdown(markdown, signal);
    expect(representations['text/plain']).toContain('[spoiler omitted]');
    for (const value of Object.values(representations)) expect(value).not.toContain('private');
    expect(diagnostics).toContainEqual(
      expect.objectContaining({ code: 'spoiler-omitted', severity: 'warning' }),
    );
  });

  it('leaves a spoiler tag shown as literal code intact', () => {
    const literal = '<span data-spoiler="true">example</span>';
    const result = convertMarkdown('`' + literal + '`', signal);
    expect(result.representations['text/plain']).toBe(literal);
    expect(result.diagnostics).toEqual([]);
  });

  it('keeps an explicit closing tag on a separate line', () => {
    const { representations } = convertMarkdown(
      'Before <span data-spoiler="true">private\n\ntext\n\n</span>\n\nAfter',
      signal,
    );
    expect(representations['text/plain']).toBe('Before [spoiler omitted]\n\nAfter');
    expect(representations['text/signal']).toContain(
      '<span data-spoiler="true">private<br><br>text<br><br></span>',
    );
  });
});
