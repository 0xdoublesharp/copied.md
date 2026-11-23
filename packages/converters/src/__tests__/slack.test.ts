import { describe, it, expect } from 'vitest';
import { SlackConverter } from '../slack';
import { FIXTURES } from './fixtures';

describe('SlackConverter', () => {
  const converter = new SlackConverter();

  it('handles empty input', () => {
    expect(converter.convert(FIXTURES.empty).payload['text/plain']).toBe('');
  });

  it('converts **bold** to *bold*', () => {
    const result = converter.convert(FIXTURES.bold);
    expect(result.payload['text/plain']).toBe('*bold text*');
  });

  it('preserves _italic_ as _italic_', () => {
    const result = converter.convert(FIXTURES.italic);
    expect(result.payload['text/plain']).toBe('_italic text_');
  });

  it('converts ~~strikethrough~~ to ~strikethrough~', () => {
    const result = converter.convert(FIXTURES.strikethrough);
    expect(result.payload['text/plain']).toBe('~deleted~');
  });

  it('preserves inline code', () => {
    const result = converter.convert(FIXTURES.inlineCode);
    expect(result.payload['text/plain']).toBe('`const x = 1`');
  });

  it('preserves code blocks (Slack supports them)', () => {
    const result = converter.convert(FIXTURES.codeBlock);
    expect(result.payload['text/plain']).toContain('```');
    expect(result.payload['text/plain']).toContain('console.log("hello");');
  });

  it('converts headings to bold (no heading support in Slack)', () => {
    const result = converter.convert(FIXTURES.heading1);
    expect(result.payload['text/plain']).toBe('*◆ Main Title*');
  });

  it('converts [text](url) to <url|text>', () => {
    const result = converter.convert(FIXTURES.link);
    expect(result.payload['text/plain']).toBe('<https://example.com|Example>');
  });

  it('preserves blockquotes', () => {
    const result = converter.convert(FIXTURES.blockquote);
    expect(result.payload['text/plain']).toContain('> This is a quote');
  });

  it('uses bullet character for unordered lists', () => {
    const result = converter.convert(FIXTURES.unorderedList);
    expect(result.payload['text/plain']).toContain('▸ Item 1');
  });

  it('renders task list checkboxes with visual marks', () => {
    const result = converter.convert(FIXTURES.taskList);
    expect(result.payload['text/plain']).toContain('☐ Todo');
    expect(result.payload['text/plain']).toContain('☑ Done');
  });

  it('preserves ordered list numbers', () => {
    const result = converter.convert(FIXTURES.orderedList);
    expect(result.payload['text/plain']).toContain('1. First');
    expect(result.payload['text/plain']).toContain('2. Second');
  });

  it('renders tables as preformatted text', () => {
    const result = converter.convert(FIXTURES.table);
    expect(result.payload['text/plain']).toContain('```');
    expect(result.payload['text/plain']).toContain('Col A');
  });

  it('provides HTML payload for rich paste', () => {
    const result = converter.convert(FIXTURES.bold);
    expect(result.payload['text/html']).toBeDefined();
    expect(result.payload['text/html']).toContain('<b>bold text</b>');
  });

  it('HTML payload has proper structure for links', () => {
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

  it('handles complex markdown', () => {
    const result = converter.convert(FIXTURES.complex);
    const text = result.payload['text/plain'];
    expect(text).toContain('*◆ Report*'); // heading as bold
    expect(text).toContain('*Summary*'); // bold
    expect(text).toContain('_important_'); // italic
    expect(text).toContain('<https://example.com|Read more>'); // link
  });
});
