// Unicode Mathematical Sans-Serif character mappings for styled text.
// These variants are generally more readable in chat apps than serif math glyphs.

const BOLD_UPPER: Record<string, string> = {};
const BOLD_LOWER: Record<string, string> = {};
const ITALIC_UPPER: Record<string, string> = {};
const ITALIC_LOWER: Record<string, string> = {};
const BOLD_ITALIC_UPPER: Record<string, string> = {};
const BOLD_ITALIC_LOWER: Record<string, string> = {};
const BOLD_DIGITS: Record<string, string> = {};

// Mathematical Sans-Serif Bold:
// U+1D5D4 (A) to U+1D5ED (Z), U+1D5EE (a) to U+1D607 (z)
for (let i = 0; i < 26; i++) {
  BOLD_UPPER[String.fromCharCode(65 + i)] = String.fromCodePoint(0x1d5d4 + i);
  BOLD_LOWER[String.fromCharCode(97 + i)] = String.fromCodePoint(0x1d5ee + i);
}

// Mathematical Sans-Serif Italic:
// U+1D608 (A) to U+1D621 (Z), U+1D622 (a) to U+1D63B (z)
for (let i = 0; i < 26; i++) {
  ITALIC_UPPER[String.fromCharCode(65 + i)] = String.fromCodePoint(0x1d608 + i);
  ITALIC_LOWER[String.fromCharCode(97 + i)] = String.fromCodePoint(0x1d622 + i);
}

// Mathematical Sans-Serif Bold Italic:
// U+1D63C (A) to U+1D655 (Z), U+1D656 (a) to U+1D66F (z)
for (let i = 0; i < 26; i++) {
  BOLD_ITALIC_UPPER[String.fromCharCode(65 + i)] = String.fromCodePoint(0x1d63c + i);
  BOLD_ITALIC_LOWER[String.fromCharCode(97 + i)] = String.fromCodePoint(0x1d656 + i);
}

// Mathematical Sans-Serif Bold Digits: U+1D7EC (0) to U+1D7F5 (9)
for (let i = 0; i < 10; i++) {
  BOLD_DIGITS[String.fromCharCode(48 + i)] = String.fromCodePoint(0x1d7ec + i);
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
