import { expect, it } from 'vitest';
import { preferredFormat, visibleFormats } from '@/lib/copy-formats';

it('keeps existing formats visible and honors saved visibility overrides', () => {
  expect(visibleFormats().some(({ id }) => id === 'signal')).toBe(true);
  expect(visibleFormats({ signal: false }).some(({ id }) => id === 'signal')).toBe(false);
  expect(visibleFormats({ signal: true }).some(({ id }) => id === 'signal')).toBe(true);
  expect(visibleFormats().some(({ id }) => id === 'teams')).toBe(false);
  expect(visibleFormats({ teams: true }).some(({ id }) => id === 'teams')).toBe(true);
});

it('chooses the first visible preference and falls back to a usable target', () => {
  expect(preferredFormat({ signal: false }, 'signal', 'telegram')).toBe('telegram');
  expect(preferredFormat({}, 'slack')).toBe('slack-paste');
  expect(preferredFormat({}, null, 42, 'unused')).toBe('rich-text');
  const hidden = Object.fromEntries(visibleFormats().map(({ id }) => [id, false]));
  expect(visibleFormats(hidden).map(({ id }) => id)).toEqual(['rich-text']);
});
