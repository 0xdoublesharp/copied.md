import { describe, expect, it } from 'vitest';
import type { Root } from 'mdast';
import { htmlText } from '../document';
import { FacebookConverter } from '../facebook';
import { convertMarkdown, getConverter, Platform, type PlatformConverter } from '../index';
import { renderText } from '../render-text';
import { SlackConverter } from '../slack';
import { SmsConverter } from '../sms';
import { TelegramConverter } from '../telegram';
import { TwitterConverter } from '../twitter';

describe('format compatibility', () => {
  const inline =
    'Before <img src="bad" alt="inline-alt"> ![Alt](https://example.org/a.png) ![](https://example.org/unlabeled.png) after.';
  const block = '<div>Raw block <img src="bad" alt="block-alt"></div>';
  const table = '| Title | Value |\n| --- | --- |\n| ***BoldItalic*** | ~~Old~~ |';

  it.each([Platform.Facebook, Platform.Slack, Platform.Telegram, Platform.SMS, Platform.Twitter])(
    'retains readable HTML text, images and table content in the %s compatibility API',
    (platform) => {
      const result = getConverter(platform).convert(
        [inline, block, '---', table, '<https://example.org/>', '[^unused]: Drop me'].join('\n\n'),
      );
      expect(result.payload['text/plain']).toContain('Raw block block-alt');
      expect(result.payload['text/plain']).toContain('inline-alt');
      expect(result.payload['text/plain']).toContain('https://example.org/a.png');
      expect(result.payload['text/plain']).toContain('Title');
      expect(result.payload['text/plain']).not.toContain('Drop me');
      if (result.payload['text/html']) {
        expect(result.payload['text/html']).toContain('Raw block block-alt');
        expect(result.payload['text/html']).toContain('<hr>');
        expect(result.payload['text/html']).not.toContain('<img');
      }
    },
  );

  it('materializes referenced notes in first-reference order and discards unused notes', () => {
    const md =
      'Claim[^b] then detail[^a] and again[^b].\n\n[^a]: First\n[^b]: Second\n[^z]: Unused';
    const plain = convertMarkdown(md, 'plain-text').representations['text/plain'];
    expect(plain).toBe('Claim[1] then detail[2] and again[1].\n\n[1] Second\n\n[2] First');
    for (const target of ['reddit', 'discord'] as const) {
      const source = convertMarkdown(md, target).representations['text/plain'];
      expect(source).toContain('Second');
      expect(source).toContain('First');
      expect(source).not.toContain('Unused');
    }
  });

  it('makes deep headings portable without changing Reddit heading depth', () => {
    expect(convertMarkdown('##### Heading', 'discord').representations['text/plain']).toBe(
      '### Heading',
    );
    expect(convertMarkdown('##### Heading', 'reddit').representations['text/plain']).toBe(
      '##### Heading',
    );
  });

  it.each<[string, new () => PlatformConverter]>([
    ['Facebook', FacebookConverter],
    ['Slack', SlackConverter],
    ['SMS', SmsConverter],
    ['Telegram', TelegramConverter],
    ['X', TwitterConverter],
  ])(
    'retains only the readable value of unknown nodes supplied by a %s parser extension',
    (_platform, Converter) => {
      const extensionTree = {
        type: 'root',
        children: [
          { type: 'plugin-block', value: 'Custom <block>' },
          { type: 'plugin-block' },
          { type: 'heading' },
          { type: 'paragraph' },
          { type: 'blockquote' },
          { type: 'code' },
          { type: 'html' },
          { type: 'list' },
          {
            type: 'list',
            ordered: false,
            children: [{ type: 'listItem', checked: null }, { type: 'listItem' }],
          },
          { type: 'table' },
          {
            type: 'table',
            children: [
              { type: 'tableRow' },
              { type: 'tableRow', children: [{ type: 'tableCell' }] },
            ],
          },
          {
            type: 'paragraph',
            children: [
              { type: 'plugin-inline', value: 'Custom <inline>' },
              { type: 'plugin-inline' },
              { type: 'html' },
            ],
          },
        ],
      } as unknown as Root;
      class ExtendedConverter extends Converter {
        protected parseMarkdown(): Root {
          return extensionTree;
        }
      }
      const { payload } = new ExtendedConverter().convert('Extension input');
      expect(payload['text/plain']).toContain('Custom <block>');
      expect(payload['text/plain']).toContain('Custom <inline>');
      expect(payload['text/plain']).not.toContain('undefined');
      expect(payload['text/plain']).not.toContain('null');
      if (payload['text/html']) {
        expect(htmlText(payload['text/html'])).toContain('Custom <block>');
        expect(htmlText(payload['text/html'])).toContain('Custom <inline>');
        expect(payload['text/html']).not.toMatch(/<(block|inline)>/);
      }
    },
  );

  it('applies combined Unicode styling only when opted in', () => {
    const source = '***Combined***';
    expect(convertMarkdown(source, 'telegram').representations['text/plain']).toBe('Combined');
    expect(
      convertMarkdown(source, 'telegram', { decorativeUnicode: true }).representations[
        'text/plain'
      ],
    ).toBe('𝘾𝙤𝙢𝙗𝙞𝙣𝙚𝙙');
  });

  it('sanitizes raw HTML links and retains alt text on unresolvable images', () => {
    const markdown =
      '<div><a href="javascript:bad()">Unsafe</a><img src="./private" alt="Diagram"></div>';
    const html = convertMarkdown(markdown, 'rich-text').representations['text/html'];
    expect(html).toContain('<a>Unsafe</a>Diagram');
    expect(html).not.toMatch(/javascript|<img/);
  });

  it('preserves task state, ordered lists and nested styles in compatibility output', () => {
    const source = [
      '1. First\n2. Second',
      '- [ ] Pending',
      '*Italic **bold*** and **Bold *italic*** and ***Both __strong__ and _emphasis_***',
      '[](https://example.org)',
      '~~~\ncode\n~~~',
      '### Detail',
    ].join('\n\n');
    for (const platform of [
      Platform.Facebook,
      Platform.Slack,
      Platform.Telegram,
      Platform.Twitter,
      Platform.SMS,
    ]) {
      const { payload } = getConverter(platform).convert(source);
      const text = payload['text/plain'];
      expect(text.normalize('NFKC')).toContain('1. First\n2. Second');
      expect(text).toContain('☐ Pending');
      expect(text).toContain('https://example.org');
      expect(text).toContain('code');
      if (platform !== Platform.SMS && platform !== Platform.Slack) {
        expect(text).toContain('𝙗𝙤𝙡𝙙');
        expect(text).toContain('𝙞𝙩𝙖𝙡𝙞𝙘');
      }
      if (platform === Platform.Telegram) expect(payload['text/html']).toContain('<ol>');
    }
  });

  it('keeps unlabeled media and code with backticks readable', () => {
    const source = [
      '[](https://example.org) ![](https://example.org/image.png)',
      '- [ ] Open task.\n\n  Separate paragraph.',
      '~~~\nfence: ```\n~~~',
      'Inline: ``a`b`` and `code`',
    ].join('\n\n');
    expect(convertMarkdown(source, 'slack-api').representations['text/plain']).toContain(
      'fence: ```',
    );
    expect(convertMarkdown(source, 'slack-api').representations['text/plain']).toContain(
      'Inline: a`b and `code`',
    );
    expect(convertMarkdown(source, 'telegram').representations['text/plain']).toContain(
      '• [ ] Open task.\n\n  Separate paragraph.',
    );
    expect(convertMarkdown(source, 'discord').representations['text/plain']).toContain(
      '[Image](https://example.org/image.png)',
    );
  });

  it('resolves reference media and keeps the first link definition', () => {
    const markdown = [
      '[Relative][link] ![Icon][pic] ![Removed][unsafe] ![]() [Empty]()',
      '',
      '[link]: ./first',
      '[link]: ./second',
      '[pic]: ./diagram "Diagram"',
      '[unsafe]: custom:ignored',
    ].join('\n');
    const result = convertMarkdown(markdown, 'telegram', {
      sourceUrl: 'https://example.org/docs/',
    });
    expect(result.representations['text/plain']).toContain(
      'Relative (https://example.org/docs/first)',
    );
    expect(result.representations['text/html']).toContain(
      'href="https://example.org/docs/diagram"',
    );
    expect(result.representations['text/plain']).toContain('Removed');
    expect(result.representations['text/plain']).not.toMatch(/custom:|second/);
    expect(result.diagnostics).toContainEqual(expect.objectContaining({ code: 'unsupported-url' }));

    const html = '<div>Image <img> <img src="https://example.org/diagram"> end</div>';
    expect(htmlText(html)).toBe('Image   end\n');
    expect(convertMarkdown(html, 'rich-text').representations['text/html']).toContain(
      'Image <a href="https://example.org/diagram">Image</a>',
    );
  });

  it('uses the image label and outer destination for linked Markdown images', () => {
    const source = [
      '[![Diagram][image]][guide]',
      '',
      '[image]: ./diagram.png',
      '[guide]: ./guide',
    ].join('\n');
    const options = { sourceUrl: 'https://example.org/notes/' };
    expect(convertMarkdown(source, 'discord', options).representations['text/plain']).toBe(
      '[Diagram](https://example.org/notes/guide)',
    );
    expect(convertMarkdown(source, 'reddit', options).representations['text/plain']).toBe(
      '[![Diagram](https://example.org/notes/diagram.png)](https://example.org/notes/guide)',
    );
    for (const target of ['telegram', 'rich-text', 'slack-paste'] as const) {
      const result = convertMarkdown(source, target, options);
      expect(result.representations['text/plain']).toBe(
        'Diagram (https://example.org/notes/guide)',
      );
      expect(result.representations['text/html']).toContain(
        '<a href="https://example.org/notes/guide">Diagram</a>',
      );
      expect(result.diagnostics).toContainEqual(
        expect.objectContaining({ code: 'formatting-omitted' }),
      );
    }
    const rtf = convertMarkdown(source, 'telegram', options).representations['text/rtf'];
    expect(rtf?.match(/HYPERLINK/g)).toHaveLength(1);
    expect(rtf).toContain('https://example.org/notes/guide');
  });

  it('uses an image label when linked Markdown media has no alt text', () => {
    const source = '[![](https://example.org/file.png)](https://example.org/guide)';
    expect(convertMarkdown(source, 'discord').representations['text/plain']).toBe(
      '[Image](https://example.org/guide)',
    );
    expect(convertMarkdown(source, 'telegram').representations['text/html']).toBe(
      '<a href="https://example.org/guide">Image</a>',
    );
  });

  it('checks non-web destinations in images inside reference links', () => {
    const source = '[**![Diagram](custom:fixture)**][guide]\n\n[guide]: https://example.org/guide';
    const result = convertMarkdown(source, 'telegram');
    expect(result.representations['text/plain']).toBe('Diagram (https://example.org/guide)');
    expect(result.representations['text/html']).toBe(
      '<a href="https://example.org/guide"><strong>Diagram</strong></a>',
    );
    expect(result.representations['text/rtf']).not.toContain('custom:');
    expect(result.diagnostics).toContainEqual(expect.objectContaining({ code: 'unsupported-url' }));
  });

  it('keeps linked raw HTML images readable without nesting rich hyperlinks', () => {
    const source =
      '<a href="https://example.org/guide"><img src="https://example.org/diagram" alt="Diagram"></a>';
    expect(convertMarkdown(source, 'rich-text').representations['text/html']).toBe(
      '<p><a href="https://example.org/guide">Diagram</a></p>',
    );
  });

  it('starts at one for an ordered syntax tree without an explicit start', () => {
    const tree: Root = {
      type: 'root',
      children: [
        {
          type: 'list',
          ordered: true,
          children: ['First', 'Second'].map((value) => ({
            type: 'listItem',
            children: [{ type: 'paragraph', children: [{ type: 'text', value }] }],
          })),
        },
      ],
    };
    expect(renderText(tree)).toBe('1. First\n2. Second');
  });

  it('does not serialize unresolved phrasing references from a partially built Markdown tree', () => {
    expect(
      renderText({
        type: 'root',
        children: [
          {
            type: 'paragraph',
            children: [
              { type: 'text', value: 'Text ' },
              {
                type: 'imageReference',
                alt: 'Missing',
                identifier: 'missing',
                referenceType: 'full',
              },
              { type: 'html', value: '<img alt="Diagram">' },
            ],
          },
        ],
      }),
    ).toBe('Text Diagram');
  });
});
