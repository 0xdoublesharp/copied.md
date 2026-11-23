import { describe, it, expect } from 'vitest';
import { TelegramConverter } from '../telegram';
import { toBold, toItalic } from '../unicode';
import { FIXTURES } from './fixtures';

describe('TelegramConverter', () => {
  const converter = new TelegramConverter();

  it('handles empty input', () => {
    expect(converter.convert(FIXTURES.empty).payload['text/plain']).toBe('');
  });

  it('renders **bold** with Unicode bold style', () => {
    const result = converter.convert(FIXTURES.bold);
    expect(result.payload['text/plain']).toBe(toBold('bold text'));
  });

  it('renders *italic* with Unicode italic style', () => {
    const result = converter.convert(FIXTURES.italic);
    expect(result.payload['text/plain']).toBe(toItalic('italic text'));
  });

  it('renders ~~strikethrough~~ as plain text (no combining marks)', () => {
    const result = converter.convert(FIXTURES.strikethrough);
    expect(result.payload['text/plain']).toBe('deleted');
    expect(result.payload['text/plain']).not.toContain('\u0336');
  });

  it('renders inline code with plain-safe code brackets', () => {
    const result = converter.convert(FIXTURES.inlineCode);
    expect(result.payload['text/plain']).toBe('「const x = 1」');
  });

  it('renders code blocks with box-style framing', () => {
    const result = converter.convert(FIXTURES.codeBlock);
    expect(result.payload['text/plain']).toContain('┌─ code: js');
    expect(result.payload['text/plain']).toContain('console.log("hello");');
    expect(result.payload['text/plain']).toContain('└─');
  });

  it('renders headings with Telegram-friendly markers', () => {
    const result = converter.convert(FIXTURES.heading1);
    expect(result.payload['text/plain']).toBe('🧭 ' + toBold('Main Title'));
  });

  it('keeps special characters readable in plain text output', () => {
    const result = converter.convert('Hello. World! Test-case');
    expect(result.payload['text/plain']).toContain('Hello. World! Test-case');
  });

  it('renders links with explicit link marker', () => {
    const result = converter.convert(FIXTURES.link);
    expect(result.payload['text/plain']).toBe('🔗 Example (https://example.com)');
  });

  it('preserves blockquotes', () => {
    const result = converter.convert(FIXTURES.blockquote);
    expect(result.payload['text/plain']).toContain('❝');
    expect(result.payload['text/plain']).toContain('quote');
  });

  it('renders task list checkboxes with visual marks', () => {
    const result = converter.convert(FIXTURES.taskList);
    expect(result.payload['text/plain']).toContain('☐ Todo');
    expect(result.payload['text/plain']).toContain('☑ Done');
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

  it('HTML payload preserves block structure for rich paste clients', () => {
    const result = converter.convert('## Heading\n\nLine 1\\\nLine 2\n\n- A\n- B');
    const html = result.payload['text/html']!;
    expect(html).toContain('<p><b>Heading</b></p>');
    expect(html).toContain('<p>Line 1<br>Line 2</p>');
    expect(html).toContain('<ul>');
    expect(html).toContain('<li>');
  });

  it('handles tables as preformatted text', () => {
    const result = converter.convert(FIXTURES.table);
    expect(result.payload['text/plain']).toContain('┌─ table');
    expect(result.payload['text/plain']).toContain('Col A');
    expect(result.payload['text/plain']).toContain('└─');
  });

  it('handles complex markdown', () => {
    const result = converter.convert(FIXTURES.complex);
    expect(result.payload['text/plain']).toContain('🧭 ' + toBold('Report'));
    expect(result.truncated).toBe(false);
  });

  it('strips raw HTML tags from input', () => {
    const result = converter.convert('## <a id="intro"></a>Introduction\n\nSome text');
    const plain = result.payload['text/plain']!;
    expect(plain).not.toContain('<a');
    expect(plain).not.toContain('</a>');
    expect(plain).toContain('✦ ' + toBold('Introduction'));
    expect(plain).toContain('Some text');
  });

  it('preserves structure for table-of-contents style markdown', () => {
    const markdown = `## Table of contents
- [Introduction](#introduction)
- [Features](#features)

## Introduction
Your repo introduction.

## Features
- Feature 1
- Feature 2`;

    const result = converter.convert(markdown);
    const plain = result.payload['text/plain']!;

    expect(plain).toContain('✦ ' + toBold('Table of contents'));
    expect(plain).toContain('\n• 🔗 Introduction (#introduction)');
    expect(plain).toContain('\n• 🔗 Features (#features)');
    expect(plain).toContain('\n\n✦ ' + toBold('Introduction'));
    expect(plain).toContain('\n\n✦ ' + toBold('Features'));
    expect(plain).toContain('• Feature 1');
    expect(plain).toContain('• Feature 2');
  });

  it('keeps a single-line selection on one line', () => {
    const result = converter.convert('## Table of contents - An inline phrase 2. More text');
    expect(result.payload['text/plain']).not.toContain('\n');
  });
});
