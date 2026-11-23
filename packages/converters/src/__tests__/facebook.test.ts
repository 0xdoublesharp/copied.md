import { describe, it, expect } from 'vitest';
import { FacebookConverter } from '../facebook';
import { toBold, toItalic, toBoldItalic } from '../unicode';
import { FIXTURES } from './fixtures';

describe('FacebookConverter', () => {
  const converter = new FacebookConverter();

  it('handles empty input', () => {
    expect(converter.convert(FIXTURES.empty).payload['text/plain']).toBe('');
  });

  it('converts bold to Unicode bold characters', () => {
    const result = converter.convert(FIXTURES.bold);
    expect(result.payload['text/plain']).toBe(toBold('bold text'));
  });

  it('converts italic to Unicode italic characters', () => {
    const result = converter.convert(FIXTURES.italic);
    expect(result.payload['text/plain']).toBe(toItalic('italic text'));
  });

  it('converts bold+italic to Unicode bold italic characters', () => {
    const result = converter.convert(FIXTURES.boldItalic);
    expect(result.payload['text/plain']).toBe(toBoldItalic('bold and italic'));
  });

  it('strips strikethrough (no Unicode equivalent)', () => {
    const result = converter.convert(FIXTURES.strikethrough);
    expect(result.payload['text/plain']).toBe('deleted');
  });

  it('preserves inline code as plain text', () => {
    const result = converter.convert(FIXTURES.inlineCode);
    expect(result.payload['text/plain']).toBe('const x = 1');
  });

  it('converts headings to bold Unicode', () => {
    const result = converter.convert(FIXTURES.heading1);
    expect(result.payload['text/plain']).toBe('◆ ' + toBold('Main Title'));
  });

  it('includes URL for links', () => {
    const result = converter.convert(FIXTURES.link);
    expect(result.payload['text/plain']).toContain('Example');
    expect(result.payload['text/plain']).toContain('https://example.com');
  });

  it('uses bullet for unordered lists', () => {
    const result = converter.convert(FIXTURES.unorderedList);
    expect(result.payload['text/plain']).toContain('\u2022 Item 1');
  });

  it('renders task list checkboxes with visual marks', () => {
    const result = converter.convert(FIXTURES.taskList);
    expect(result.payload['text/plain']).toContain('☐ Todo');
    expect(result.payload['text/plain']).toContain('☑ Done');
  });

  it('uses numbers for ordered lists', () => {
    const result = converter.convert(FIXTURES.orderedList);
    expect(result.payload['text/plain']).toContain('1. First');
  });

  it('handles blockquotes with decorative quote', () => {
    const result = converter.convert(FIXTURES.blockquote);
    expect(result.payload['text/plain']).toContain('\u275D');
    expect(result.payload['text/plain']).toContain('This is a quote');
  });

  it('provides HTML payload for rich paste', () => {
    const result = converter.convert(FIXTURES.bold);
    expect(result.payload['text/html']).toBeDefined();
    expect(result.payload['text/html']).toContain('<b>bold text</b>');
  });

  it('HTML payload has proper heading tags', () => {
    const result = converter.convert(FIXTURES.heading1);
    expect(result.payload['text/html']).toContain('<h1>Main Title</h1>');
  });

  it('HTML payload has proper link tags', () => {
    const result = converter.convert(FIXTURES.link);
    expect(result.payload['text/html']).toContain('<a href="https://example.com">Example</a>');
  });

  it('HTML payload renders tables as proper HTML table elements', () => {
    const result = converter.convert(FIXTURES.table);
    const html = result.payload['text/html']!;
    expect(html).toContain('<table>');
    expect(html).toContain('<tr>');
    expect(html).toContain('<td>Col A</td>');
    expect(html).toContain('<td>Col B</td>');
    expect(html).toContain('</table>');
  });

  it('handles nested formatting (bold with italic inside)', () => {
    const result = converter.convert(FIXTURES.nestedFormatting);
    const text = result.payload['text/plain'];
    // "bold with " should be bold, "italic" should be bold+italic, " inside" should be bold
    expect(text).toContain(toBold('bold with '));
    expect(text).toContain(toBoldItalic('italic'));
    expect(text).toContain(toBold(' inside'));
  });

  it('handles complex markdown', () => {
    const result = converter.convert(FIXTURES.complex);
    const text = result.payload['text/plain'];
    expect(text).toContain(toBold('Report')); // heading
    expect(text).toContain(toBold('Summary')); // bold
    expect(text).toContain(toItalic('important')); // italic
  });
});
