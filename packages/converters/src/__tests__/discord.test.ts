import { describe, it, expect } from 'vitest';
import { DiscordConverter } from '../discord';
import { FIXTURES } from './fixtures';

describe('DiscordConverter', () => {
  const converter = new DiscordConverter();

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

  it('preserves code blocks with language', () => {
    const result = converter.convert(FIXTURES.codeBlock);
    expect(result.payload['text/plain']).toContain('```js');
    expect(result.payload['text/plain']).toContain('console.log("hello");');
  });

  it('converts headings to bold (Discord has no heading support)', () => {
    const result = converter.convert(FIXTURES.heading1);
    expect(result.payload['text/plain']).toBe('# Main Title');
  });

  it('uses native markdown link syntax', () => {
    const result = converter.convert(FIXTURES.link);
    expect(result.payload['text/plain']).toBe('[Example](https://example.com)');
  });

  it('preserves image alt text with URL', () => {
    const result = converter.convert(FIXTURES.image);
    expect(result.payload['text/plain']).toBe('[Alt text](https://example.com/img.png)');
  });

  it('preserves blockquotes', () => {
    const result = converter.convert(FIXTURES.blockquote);
    expect(result.payload['text/plain']).toContain('> This is a quote');
  });

  it('renders tables as code blocks', () => {
    const result = converter.convert(FIXTURES.table);
    expect(result.payload['text/plain']).toContain('```');
    expect(result.payload['text/plain']).toContain('Col A');
  });

  it('preserves list structure', () => {
    const result = converter.convert(FIXTURES.unorderedList);
    expect(result.payload['text/plain']).toContain('- Item 1');
  });

  it('renders task list checkboxes with visual marks', () => {
    const result = converter.convert(FIXTURES.taskList);
    expect(result.payload['text/plain']).toContain('- [ ] Todo');
    expect(result.payload['text/plain']).toContain('- [x] Done');
  });

  it('handles nested formatting', () => {
    const result = converter.convert(FIXTURES.nestedFormatting);
    expect(result.payload['text/plain']).toContain('**');
    expect(result.payload['text/plain']).toContain('*italic*');
  });

  it('has no HTML payload', () => {
    const result = converter.convert(FIXTURES.bold);
    expect(result.payload['text/html']).toBeUndefined();
  });
});
