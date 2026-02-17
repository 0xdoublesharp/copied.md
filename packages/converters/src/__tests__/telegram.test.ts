import { describe, it, expect } from 'vitest';
import { TelegramConverter } from '../telegram';
import { FIXTURES } from './fixtures';

describe('TelegramConverter', () => {
  const converter = new TelegramConverter();

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

  it('preserves code blocks', () => {
    const result = converter.convert(FIXTURES.codeBlock);
    expect(result.payload['text/plain']).toContain('```js');
    expect(result.payload['text/plain']).toContain('console.log("hello");');
  });

  it('converts headings to bold', () => {
    const result = converter.convert(FIXTURES.heading1);
    expect(result.payload['text/plain']).toBe('*Main Title*');
  });

  it('escapes special characters in text', () => {
    const result = converter.convert('Hello. World! Test-case');
    expect(result.payload['text/plain']).toContain('\\.');
    expect(result.payload['text/plain']).toContain('\\!');
    expect(result.payload['text/plain']).toContain('\\-');
  });

  it('preserves link syntax', () => {
    const result = converter.convert(FIXTURES.link);
    expect(result.payload['text/plain']).toContain('[Example]');
    expect(result.payload['text/plain']).toContain('example\\.com');
  });

  it('preserves blockquotes', () => {
    const result = converter.convert(FIXTURES.blockquote);
    expect(result.payload['text/plain']).toContain('>');
    expect(result.payload['text/plain']).toContain('quote');
  });

  it('provides HTML payload for rich paste', () => {
    const result = converter.convert(FIXTURES.bold);
    expect(result.payload['text/html']).toBeDefined();
    expect(result.payload['text/html']).toContain('<b>bold text</b>');
  });

  it('HTML payload converts italic properly', () => {
    const result = converter.convert(FIXTURES.italic);
    expect(result.payload['text/html']).toContain('<i>italic text</i>');
  });

  it('handles tables as preformatted text', () => {
    const result = converter.convert(FIXTURES.table);
    expect(result.payload['text/plain']).toContain('```');
    expect(result.payload['text/plain']).toContain('Col A');
  });

  it('handles complex markdown', () => {
    const result = converter.convert(FIXTURES.complex);
    expect(result.payload['text/plain']).toContain('*Report*');
    expect(result.truncated).toBe(false);
  });
});
