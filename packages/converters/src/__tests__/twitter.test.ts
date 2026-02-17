import { describe, it, expect } from 'vitest';
import { TwitterConverter } from '../twitter';
import { toBold, toItalic } from '../unicode';
import { FIXTURES } from './fixtures';

describe('TwitterConverter', () => {
  const converter = new TwitterConverter();

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

  it('converts headings to bold Unicode', () => {
    const result = converter.convert(FIXTURES.heading1);
    expect(result.payload['text/plain']).toBe(toBold('Main Title'));
  });

  it('extracts URL from links (no anchor text)', () => {
    const result = converter.convert(FIXTURES.link);
    expect(result.payload['text/plain']).toBe('https://example.com');
  });

  it('strips inline code markers', () => {
    const result = converter.convert(FIXTURES.inlineCode);
    expect(result.payload['text/plain']).toBe('const x = 1');
  });

  it('strips code block markers', () => {
    const result = converter.convert(FIXTURES.codeBlock);
    expect(result.payload['text/plain']).toContain('console.log("hello");');
  });

  it('uses bullet for unordered lists', () => {
    const result = converter.convert(FIXTURES.unorderedList);
    expect(result.payload['text/plain']).toContain('\u2022 Item 1');
  });

  it('truncates at 280 characters', () => {
    const longText = '**' + 'A'.repeat(300) + '**';
    const result = converter.convert(longText);
    expect(result.payload['text/plain'].length).toBeLessThanOrEqual(280);
    expect(result.truncated).toBe(true);
  });

  it('does not truncate short text', () => {
    const result = converter.convert(FIXTURES.bold);
    expect(result.truncated).toBe(false);
  });

  it('has no HTML payload', () => {
    const result = converter.convert(FIXTURES.bold);
    expect(result.payload['text/html']).toBeUndefined();
  });

  it('handles complex markdown', () => {
    const result = converter.convert(FIXTURES.complex);
    const text = result.payload['text/plain'];
    expect(text).toContain(toBold('Report'));
    expect(text).toContain(toBold('Summary'));
  });
});
