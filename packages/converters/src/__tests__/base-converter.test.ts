import { describe, it, expect } from 'vitest';
import { getConverter, getAllPlatforms, Platform } from '../index';

describe('Converter registry', () => {
  it('returns a converter for every platform', () => {
    for (const platform of getAllPlatforms()) {
      const converter = getConverter(platform);
      expect(converter).toBeDefined();
      expect(converter.platform).toBe(platform);
    }
  });

  it('throws for unknown platform', () => {
    expect(() => getConverter('unknown' as Platform)).toThrow('No converter for platform');
  });

  it('getAllPlatforms returns all 8 platforms', () => {
    const platforms = getAllPlatforms();
    expect(platforms).toHaveLength(8);
    expect(platforms).toContain(Platform.Facebook);
    expect(platforms).toContain(Platform.Twitter);
    expect(platforms).toContain(Platform.Slack);
    expect(platforms).toContain(Platform.Discord);
    expect(platforms).toContain(Platform.Telegram);
    expect(platforms).toContain(Platform.SMS);
    expect(platforms).toContain(Platform.Reddit);
    expect(platforms).toContain(Platform.HTML);
  });

  it('honors hard line breaks across converters', () => {
    const markdown = 'Line one\\\nLine two';

    for (const platform of getAllPlatforms()) {
      const converter = getConverter(platform);
      const result = converter.convert(markdown);
      const plain = result.payload['text/plain'];

      if (platform === Platform.HTML) {
        expect(plain).toContain('<br>');
      } else {
        expect(plain).toContain('\n');
      }
    }
  });
});
