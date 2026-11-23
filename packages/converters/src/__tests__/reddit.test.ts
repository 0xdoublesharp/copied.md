import { describe, it, expect } from 'vitest';
import { RedditConverter } from '../reddit';
import { FIXTURES } from './fixtures';

describe('RedditConverter', () => {
  const converter = new RedditConverter();

  it('handles empty input', () => {
    expect(converter.convert(FIXTURES.empty).payload['text/plain']).toBe('');
  });

  it('preserves bold syntax', () => {
    const result = converter.convert(FIXTURES.bold);
    expect(result.payload['text/plain']).toBe('**bold text**');
  });

  it('preserves italic syntax', () => {
    const result = converter.convert(FIXTURES.italic);
    expect(result.payload['text/plain']).toBe('*italic text*');
  });

  it('preserves strikethrough', () => {
    const result = converter.convert(FIXTURES.strikethrough);
    expect(result.payload['text/plain']).toBe('~~deleted~~');
  });

  it('preserves inline code', () => {
    const result = converter.convert(FIXTURES.inlineCode);
    expect(result.payload['text/plain']).toBe('`const x = 1`');
  });

  it('preserves code blocks', () => {
    const result = converter.convert(FIXTURES.codeBlock);
    expect(result.payload['text/plain']).toContain('```js');
    expect(result.payload['text/plain']).toContain('console.log("hello");');
    expect(result.payload['text/plain']).toContain('```');
  });

  it('preserves headings', () => {
    const result = converter.convert(FIXTURES.heading1);
    expect(result.payload['text/plain']).toBe('# Main Title');
  });

  it('preserves h2', () => {
    const result = converter.convert(FIXTURES.heading2);
    expect(result.payload['text/plain']).toBe('## Sub Title');
  });

  it('preserves links', () => {
    const result = converter.convert(FIXTURES.link);
    expect(result.payload['text/plain']).toBe('[Example](https://example.com)');
  });

  it('preserves unordered lists', () => {
    const result = converter.convert(FIXTURES.unorderedList);
    expect(result.payload['text/plain']).toContain('- Item 1');
    expect(result.payload['text/plain']).toContain('- Item 2');
  });

  it('preserves task list markdown structure', () => {
    const result = converter.convert(FIXTURES.taskList);
    expect(result.payload['text/plain']).toContain('- [ ] Todo');
    expect(result.payload['text/plain']).toContain('- [x] Done');
  });

  it('preserves ordered lists', () => {
    const result = converter.convert(FIXTURES.orderedList);
    expect(result.payload['text/plain']).toContain('1. First');
    expect(result.payload['text/plain']).toContain('2. Second');
  });

  it('preserves blockquotes', () => {
    const result = converter.convert(FIXTURES.blockquote);
    expect(result.payload['text/plain']).toContain('> This is a quote');
  });

  it('preserves tables', () => {
    const result = converter.convert(FIXTURES.table);
    expect(result.payload['text/plain']).toContain('Col A');
    expect(result.payload['text/plain']).toContain('Col B');
    expect(result.payload['text/plain']).toContain('---');
  });

  it('handles nested formatting', () => {
    const result = converter.convert(FIXTURES.nestedFormatting);
    expect(result.payload['text/plain']).toContain('**');
    expect(result.payload['text/plain']).toContain('*italic*');
  });

  it('has no HTML payload (plain text only)', () => {
    const result = converter.convert(FIXTURES.bold);
    expect(result.payload['text/html']).toBeUndefined();
  });
});
