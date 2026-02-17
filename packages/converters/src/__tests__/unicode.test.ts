import { describe, it, expect } from 'vitest';
import { toBold, toItalic, toBoldItalic } from '../unicode';

describe('Unicode text styling', () => {
  describe('toBold', () => {
    it('converts uppercase letters to mathematical bold', () => {
      expect(toBold('ABC')).toBe('\u{1D400}\u{1D401}\u{1D402}');
    });

    it('converts lowercase letters to mathematical bold', () => {
      expect(toBold('abc')).toBe('\u{1D41A}\u{1D41B}\u{1D41C}');
    });

    it('converts digits to bold digits', () => {
      expect(toBold('123')).toBe('\u{1D7CF}\u{1D7D0}\u{1D7D1}');
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
    it('converts uppercase letters to mathematical italic', () => {
      expect(toItalic('A')).toBe('\u{1D434}');
    });

    it('converts lowercase letters to mathematical italic', () => {
      expect(toItalic('a')).toBe('\u{1D44E}');
    });

    it('handles the special case for h (planck constant)', () => {
      expect(toItalic('h')).toBe('\u210E');
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
    it('converts uppercase letters to mathematical bold italic', () => {
      expect(toBoldItalic('A')).toBe('\u{1D468}');
    });

    it('converts lowercase letters to mathematical bold italic', () => {
      expect(toBoldItalic('a')).toBe('\u{1D482}');
    });

    it('converts digits to bold digits', () => {
      expect(toBoldItalic('0')).toBe('\u{1D7CE}');
    });

    it('handles empty string', () => {
      expect(toBoldItalic('')).toBe('');
    });
  });
});
