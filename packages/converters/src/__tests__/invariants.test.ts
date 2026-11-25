import fc from 'fast-check';
import process from 'node:process';
import { fromHtml } from 'hast-util-from-html';
import { visit } from 'unist-util-visit';
import { describe, expect, it } from 'vitest';
import { convertMarkdown, resolveTarget, targets } from '../index';

const parameters = {
  seed: Number(process.env.COPIED_MD_FUZZ_SEED ?? 17041),
  numRuns: Number(process.env.COPIED_MD_FUZZ_RUNS ?? 200),
  ...(process.env.COPIED_MD_FUZZ_PATH === undefined
    ? {}
    : { path: process.env.COPIED_MD_FUZZ_PATH }),
};
const timeout = Math.max(30_000, parameters.numRuns * 150);

const fragments = fc.constantFrom(
  '# Release check',
  '> Quote\n>\n> Second paragraph',
  '- [x] Task\n- **Bold** and *italic*\n  3. Nested',
  '| A | B |\n|---|---|\n| a | b |',
  '~~~ts\nconst a = "<x>";\n~~~',
  '[Link][ref]\n\n[ref]: ./notes "Title"',
  '[^x] and again [^x]\n\n[^x]: footnote',
  '![Icon](https://example.org/icon.png)',
  '[![Image][asset]][manual]\n\n[asset]: ./asset.png\n[manual]: ./manual',
  '<a href="https://example.org/manual"><img src="./asset.png" alt="Image"></a>',
  '[Relative](./notes) [Fragment](#intro) [Mail](mailto:hi@example.org)',
  '[Unsafe](javascript:alert%281%29)',
  '<script>ignored()</script><b>HTML</b>',
  'one  \ntwo ~~three~~',
  'Before <span data-spoiler="true">covered</span> after.',
  'Unclosed <span data-spoiler="true">covered',
  '👩🏽‍💻 🇺🇦 e\u0301 𝄞 漢字 & < > \\ { }',
  '---',
);
const markdown = fc
  .array(
    fc.oneof(
      fragments,
      fc.string({ unit: 'binary', maxLength: 60 }),
      fc.string({ unit: 'grapheme', maxLength: 40 }),
    ),
    {
      maxLength: 8,
    },
  )
  .map((parts) => parts.join('\n\n'));

function assertSafeHtml(html: string) {
  let linkDepth = 0;
  for (const [tag] of html.matchAll(/<\/?a(?=[\s>])[^>]*>/gi)) {
    linkDepth += tag.startsWith('</') ? -1 : 1;
    expect(linkDepth).toBeGreaterThanOrEqual(0);
    expect(linkDepth).toBeLessThanOrEqual(1);
  }
  expect(linkDepth).toBe(0);
  visit(fromHtml(html, { fragment: true }), 'element', (element) => {
    expect(['script', 'iframe', 'object', 'embed', 'style', 'form']).not.toContain(element.tagName);
    for (const [name, value] of Object.entries(element.properties)) {
      expect(name).not.toMatch(/^on/i);
      if (name === 'href' || name === 'src') {
        const url = new URL(String(value), 'https://example.org/');
        expect(['http:', 'https:', 'mailto:']).toContain(url.protocol);
      }
    }
  });
}

function assertValidRtfGroups(rtf: string) {
  expect(rtf).toMatch(/^\{\\rtf1/);
  let depth = 0;
  for (let index = 0; index < rtf.length; index++) {
    const char = rtf[index];
    if (char === '\\') index++;
    if (char === '{') depth++;
    if (char === '}') {
      depth--;
      expect(depth).toBeGreaterThanOrEqual(0);
    }
  }
  expect(depth).toBe(0);
}

describe('Markdown conversion properties', () => {
  it('resolves exactly the published targets and legacy aliases', () => {
    expect(new Set(targets.map(({ id }) => id)).size).toBe(targets.length);
    for (const { id } of targets) expect(resolveTarget(id)).toBe(id);
    expect(resolveTarget('html')).toBe('rich-text');
    expect(resolveTarget('slack')).toBe('slack-paste');
    expect(resolveTarget('missing')).toBeUndefined();
    expect(() => convertMarkdown('', 'missing' as never)).toThrow(RangeError);
  });

  it(
    'preserves payload, metric and determinism invariants for every target',
    () => {
      fc.assert(
        fc.property(markdown, fc.boolean(), fc.boolean(), (source, hasUrl, decorativeUnicode) => {
          const options = {
            decorativeUnicode,
            ...(hasUrl ? { sourceUrl: 'https://example.org/docs/page' } : {}),
          };
          for (const { id, rich } of targets) {
            const result = convertMarkdown(source, id, options);
            const plain = result.representations['text/plain'];
            const malformedSpoiler = result.diagnostics.some(
              ({ code, severity }) => code === 'spoiler-omitted' && severity === 'warning',
            );
            expect(convertMarkdown(source, id, options)).toEqual(result);
            expect(result.schemaVersion).toBe(1);
            expect(result.target).toBe(id);
            expect(result.metrics.sourceCodeUnits).toBe(source.length);
            expect(Object.keys(result.representations).sort()).toEqual(
              [
                'text/plain',
                ...(rich && !malformedSpoiler
                  ? [id === 'signal' ? 'text/signal' : 'text/html']
                  : []),
                ...(id === 'telegram' && plain ? ['text/rtf'] : []),
              ].sort(),
            );
            if (id === 'twitter') {
              expect(result.metrics).toMatchObject({ unit: 'x-weighted-characters', limit: 280 });
              expect(result.diagnostics.some(({ code }) => code === 'length-exceeded')).toBe(
                result.metrics.outputLength > 280,
              );
            } else {
              expect(result.metrics).toMatchObject({
                unit: 'code-points',
                outputLength: [...plain].length,
              });
              expect(result.metrics.limit).toBeUndefined();
            }
            const html =
              id === 'html-source'
                ? plain
                : (result.representations['text/html'] ?? result.representations['text/signal']);
            if (html !== undefined) assertSafeHtml(html);
            const rtf = result.representations['text/rtf'];
            if (rtf !== undefined) assertValidRtfGroups(rtf);
          }
        }),
        parameters,
      );
    },
    timeout,
  );

  it(
    'discards unsafe link schemes even when casing and labels vary',
    () => {
      fc.assert(
        fc.property(
          fc.stringMatching(/^[a-zA-Z0-9]{1,24}$/),
          fc.constantFrom('javascript', 'JaVaScRiPt', 'data', 'vbscript', 'file'),
          (label, scheme) => {
            const source = '[' + label + '](' + scheme + ':example)';
            for (const { id } of targets) {
              const result = convertMarkdown(source, id, {
                sourceUrl: 'https://example.org/docs/',
              });
              expect(result.diagnostics.map(({ code }) => code)).toContain('unsupported-url');
              for (const value of Object.values(result.representations)) {
                expect(value).toContain(label);
                expect(value).not.toContain(scheme + ':example');
              }
            }
          },
        ),
        parameters,
      );
    },
    timeout,
  );
});
