import { describe, it, expect } from 'vitest';
import { HtmlConverter } from '../html';
import { FIXTURES } from './fixtures';

describe('HtmlConverter', () => {
  const converter = new HtmlConverter();

  it('handles empty input', () => {
    const result = converter.convert(FIXTURES.empty);
    expect(result.payload['text/plain']).toBe('');
    expect(result.payload['text/html']).toBe('');
  });

  it('converts bold to <strong>', () => {
    const result = converter.convert(FIXTURES.bold);
    expect(result.payload['text/plain']).toContain('<strong>bold text</strong>');
  });

  it('converts italic to <em>', () => {
    const result = converter.convert(FIXTURES.italic);
    expect(result.payload['text/plain']).toContain('<em>italic text</em>');
  });

  it('converts strikethrough to <del>', () => {
    const result = converter.convert(FIXTURES.strikethrough);
    expect(result.payload['text/plain']).toContain('<del>deleted</del>');
  });

  it('converts inline code to <code>', () => {
    const result = converter.convert(FIXTURES.inlineCode);
    expect(result.payload['text/plain']).toContain('<code>const x = 1</code>');
  });

  it('converts code blocks to <pre><code>', () => {
    const result = converter.convert(FIXTURES.codeBlock);
    expect(result.payload['text/plain']).toContain('<pre>');
    expect(result.payload['text/plain']).toContain('<code');
    expect(result.payload['text/plain']).toContain('console.log');
  });

  it('converts heading to <h1>', () => {
    const result = converter.convert(FIXTURES.heading1);
    expect(result.payload['text/plain']).toContain('<h1>Main Title</h1>');
  });

  it('converts heading to <h2>', () => {
    const result = converter.convert(FIXTURES.heading2);
    expect(result.payload['text/plain']).toContain('<h2>Sub Title</h2>');
  });

  it('converts links to <a>', () => {
    const result = converter.convert(FIXTURES.link);
    expect(result.payload['text/plain']).toContain('<a href="https://example.com">Example</a>');
  });

  it('converts unordered lists to <ul><li>', () => {
    const result = converter.convert(FIXTURES.unorderedList);
    expect(result.payload['text/plain']).toContain('<ul>');
    expect(result.payload['text/plain']).toContain('<li>');
    expect(result.payload['text/plain']).toContain('Item 1');
  });

  it('converts ordered lists to <ol><li>', () => {
    const result = converter.convert(FIXTURES.orderedList);
    expect(result.payload['text/plain']).toContain('<ol>');
    expect(result.payload['text/plain']).toContain('<li>');
    expect(result.payload['text/plain']).toContain('First');
  });

  it('converts blockquotes to <blockquote>', () => {
    const result = converter.convert(FIXTURES.blockquote);
    expect(result.payload['text/plain']).toContain('<blockquote>');
    expect(result.payload['text/plain']).toContain('This is a quote');
  });

  it('converts tables to <table>', () => {
    const result = converter.convert(FIXTURES.table);
    expect(result.payload['text/plain']).toContain('<table>');
    expect(result.payload['text/plain']).toContain('<th>');
    expect(result.payload['text/plain']).toContain('Col A');
  });

  it('has matching text/plain and text/html', () => {
    const result = converter.convert(FIXTURES.bold);
    expect(result.payload['text/plain']).toBe(result.payload['text/html']);
  });

  it('handles complex markdown', () => {
    const result = converter.convert(FIXTURES.complex);
    const html = result.payload['text/plain'];
    expect(html).toContain('<h1>');
    expect(html).toContain('<strong>');
    expect(html).toContain('<em>');
    expect(html).toContain('<ul>');
    expect(html).toContain('<blockquote>');
    expect(html).toContain('<pre>');
    expect(html).toContain('<a href=');
  });
});
