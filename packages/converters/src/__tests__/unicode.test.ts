import { describe, it, expect } from 'vitest';
import { toBold, toItalic, toBoldItalic } from '../unicode';

describe('Unicode text styling', () => {
  describe('toBold', () => {
    it('converts uppercase letters to sans-serif bold', () => {
      expect(toBold('ABC')).toBe('\u{1D5D4}\u{1D5D5}\u{1D5D6}');
    });

    it('converts lowercase letters to sans-serif bold', () => {
      expect(toBold('abc')).toBe('\u{1D5EE}\u{1D5EF}\u{1D5F0}');
    });

    it('converts digits to bold digits', () => {
      expect(toBold('123')).toBe('\u{1D7ED}\u{1D7EE}\u{1D7EF}');
    });

    it('preserves non-alphanumeric characters', () => {
      expect(toBold('Hello, World!')).toContain(',');
      expect(toBold('Hello, World!')).toContain('!');
      expect(toBold('Hello, World!')).toContain(' ');
    });

    it('handles empty string', () => {
      expect(toBold('')).toBe('');
    });
  });

  describe('toItalic', () => {
    it('converts uppercase letters to sans-serif italic', () => {
      expect(toItalic('A')).toBe('\u{1D608}');
    });

    it('converts lowercase letters to sans-serif italic', () => {
      expect(toItalic('a')).toBe('\u{1D622}');
    });

    it('keeps h as sans-serif italic h', () => {
      expect(toItalic('h')).toBe('\u{1D629}');
    });

    it('preserves non-letter characters', () => {
      expect(toItalic('123')).toBe('123');
      expect(toItalic('!?')).toBe('!?');
    });

    it('handles empty string', () => {
      expect(toItalic('')).toBe('');
    });
  });

  describe('toBoldItalic', () => {
    it('converts uppercase letters to sans-serif bold italic', () => {
      expect(toBoldItalic('A')).toBe('\u{1D63C}');
    });

    it('converts lowercase letters to sans-serif bold italic', () => {
      expect(toBoldItalic('a')).toBe('\u{1D656}');
    });

    it('converts digits to bold digits', () => {
      expect(toBoldItalic('0')).toBe('\u{1D7EC}');
    });

    it('handles empty string', () => {
      expect(toBoldItalic('')).toBe('');
    });
  });
});
