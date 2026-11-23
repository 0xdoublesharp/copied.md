import { describe, it, expect } from 'vitest';
import { SmsConverter } from '../sms';
import { FIXTURES } from './fixtures';

describe('SmsConverter', () => {
  const converter = new SmsConverter();

  it('handles empty input', () => {
    expect(converter.convert(FIXTURES.empty).payload['text/plain']).toBe('');
  });

  it('handles whitespace-only input', () => {
    expect(converter.convert(FIXTURES.whitespaceOnly).payload['text/plain']).toBe('');
  });

  it('strips bold formatting', () => {
    const result = converter.convert(FIXTURES.bold);
    expect(result.payload['text/plain']).toBe('bold text');
  });

  it('strips italic formatting', () => {
    const result = converter.convert(FIXTURES.italic);
    expect(result.payload['text/plain']).toBe('italic text');
  });

  it('strips bold italic formatting', () => {
    const result = converter.convert(FIXTURES.boldItalic);
    expect(result.payload['text/plain']).toBe('bold and italic');
  });

  it('strips strikethrough', () => {
    const result = converter.convert(FIXTURES.strikethrough);
    expect(result.payload['text/plain']).toBe('deleted');
  });

  it('strips inline code markers', () => {
    const result = converter.convert(FIXTURES.inlineCode);
    expect(result.payload['text/plain']).toBe('const x = 1');
  });

  it('strips code block markers', () => {
    const result = converter.convert(FIXTURES.codeBlock);
    expect(result.payload['text/plain']).toContain('console.log("hello");');
  });

  it('converts headings to uppercase', () => {
    const result = converter.convert(FIXTURES.heading1);
    expect(result.payload['text/plain']).toBe('◆ MAIN TITLE');
  });

  it('preserves link text and URL', () => {
    const result = converter.convert(FIXTURES.link);
    expect(result.payload['text/plain']).toBe('Example (https://example.com)');
  });

  it('preserves image alt text and URL', () => {
    const result = converter.convert(FIXTURES.image);
    expect(result.payload['text/plain']).toBe('Alt text (https://example.com/img.png)');
  });

  it('preserves list structure', () => {
    const result = converter.convert(FIXTURES.unorderedList);
    expect(result.payload['text/plain']).toContain('▸ Item 1');
    expect(result.payload['text/plain']).toContain('▸ Item 2');
  });

  it('renders task list checkboxes with visual marks', () => {
    const result = converter.convert(FIXTURES.taskList);
    expect(result.payload['text/plain']).toContain('☐ Todo');
    expect(result.payload['text/plain']).toContain('☑ Done');
  });

  it('preserves ordered list structure', () => {
    const result = converter.convert(FIXTURES.orderedList);
    expect(result.payload['text/plain']).toContain('1. First');
    expect(result.payload['text/plain']).toContain('2. Second');
  });

  it('converts blockquotes', () => {
    const result = converter.convert(FIXTURES.blockquote);
    expect(result.payload['text/plain']).toContain('This is a quote');
  });

  it('handles complex markdown', () => {
    const result = converter.convert(FIXTURES.complex);
    const text = result.payload['text/plain'];
    expect(text).toContain('REPORT');
    expect(text).toContain('Summary');
    expect(text).toContain('important');
    expect(text).toContain('Point 1');
  });

  it('has no HTML payload', () => {
    const result = converter.convert(FIXTURES.bold);
    expect(result.payload['text/html']).toBeUndefined();
  });

  it('reports correct originalLength', () => {
    const result = converter.convert(FIXTURES.bold);
    expect(result.originalLength).toBe(FIXTURES.bold.length);
  });
});
