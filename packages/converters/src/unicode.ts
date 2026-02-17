// Unicode Mathematical character mappings for styled text
// These characters render as visually bold/italic in plain text contexts

const BOLD_UPPER: Record<string, string> = {};
const BOLD_LOWER: Record<string, string> = {};
const ITALIC_UPPER: Record<string, string> = {};
const ITALIC_LOWER: Record<string, string> = {};
const BOLD_ITALIC_UPPER: Record<string, string> = {};
const BOLD_ITALIC_LOWER: Record<string, string> = {};
const BOLD_DIGITS: Record<string, string> = {};

// Mathematical Bold: U+1D400 (A) to U+1D419 (Z), U+1D41A (a) to U+1D433 (z)
for (let i = 0; i < 26; i++) {
  BOLD_UPPER[String.fromCharCode(65 + i)] = String.fromCodePoint(0x1d400 + i);
  BOLD_LOWER[String.fromCharCode(97 + i)] = String.fromCodePoint(0x1d41a + i);
}

// Mathematical Italic: U+1D434 (A) to U+1D44D (Z), U+1D44E (a) to U+1D467 (z)
// Note: 'h' (U+1D462) is replaced by the planck constant ℎ (U+210E) in Unicode
for (let i = 0; i < 26; i++) {
  ITALIC_UPPER[String.fromCharCode(65 + i)] = String.fromCodePoint(0x1d434 + i);
  if (i === 7) {
    // 'h' special case
    ITALIC_LOWER[String.fromCharCode(97 + i)] = '\u210E';
  } else {
    ITALIC_LOWER[String.fromCharCode(97 + i)] = String.fromCodePoint(0x1d44e + i);
  }
}

// Mathematical Bold Italic: U+1D468 (A) to U+1D481 (Z), U+1D482 (a) to U+1D49B (z)
for (let i = 0; i < 26; i++) {
  BOLD_ITALIC_UPPER[String.fromCharCode(65 + i)] = String.fromCodePoint(0x1d468 + i);
  BOLD_ITALIC_LOWER[String.fromCharCode(97 + i)] = String.fromCodePoint(0x1d482 + i);
}

// Mathematical Bold Digits: U+1D7CE (0) to U+1D7D7 (9)
for (let i = 0; i < 10; i++) {
  BOLD_DIGITS[String.fromCharCode(48 + i)] = String.fromCodePoint(0x1d7ce + i);
}

function mapChar(
  char: string,
  upperMap: Record<string, string>,
  lowerMap: Record<string, string>,
  digitMap?: Record<string, string>,
): string {
  if (upperMap[char]) return upperMap[char];
  if (lowerMap[char]) return lowerMap[char];
  if (digitMap && digitMap[char]) return digitMap[char];
  return char;
}

export function toBold(text: string): string {
  return Array.from(text)
    .map((c) => mapChar(c, BOLD_UPPER, BOLD_LOWER, BOLD_DIGITS))
    .join('');
}

export function toItalic(text: string): string {
  return Array.from(text)
    .map((c) => mapChar(c, ITALIC_UPPER, ITALIC_LOWER))
    .join('');
}

export function toBoldItalic(text: string): string {
  return Array.from(text)
    .map((c) => mapChar(c, BOLD_ITALIC_UPPER, BOLD_ITALIC_LOWER, BOLD_DIGITS))
    .join('');
}
