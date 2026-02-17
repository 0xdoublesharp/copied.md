import { describe, it, expect } from 'vitest';
import { getConverter, Platform, getAllPlatforms } from '@copied-md/converters';
import { PLATFORM_MENU_IDS, PLATFORM_LABELS, menuIdToPlatform } from '@/lib/constants';

describe('Context menu integration', () => {
  it('has a menu ID for every platform', () => {
    for (const platform of getAllPlatforms()) {
      expect(PLATFORM_MENU_IDS[platform]).toBeDefined();
      expect(PLATFORM_MENU_IDS[platform]).toContain('copied-md-');
    }
  });

  it('has a label for every platform', () => {
    for (const platform of getAllPlatforms()) {
      expect(PLATFORM_LABELS[platform]).toBeDefined();
      expect(PLATFORM_LABELS[platform].length).toBeGreaterThan(0);
    }
  });

  it('maps menu IDs back to platforms correctly', () => {
    for (const [platform, menuId] of Object.entries(PLATFORM_MENU_IDS)) {
      expect(menuIdToPlatform(menuId)).toBe(platform);
    }
  });

  it('returns undefined for unknown menu IDs', () => {
    expect(menuIdToPlatform('unknown')).toBeUndefined();
  });

  it('end-to-end: selection -> convert -> clipboard payload', () => {
    const markdown = '**Hello** _world_!\n\n- Item 1\n- Item 2\n\n[Link](https://example.com)';

    for (const platform of getAllPlatforms()) {
      const converter = getConverter(platform);
      const result = converter.convert(markdown);

      // Every converter should return valid payload
      expect(result.payload['text/plain']).toBeDefined();
      expect(result.payload['text/plain'].length).toBeGreaterThan(0);
      expect(result.originalLength).toBe(markdown.length);
      expect(typeof result.truncated).toBe('boolean');
    }
  });

  it('Slack conversion produces both plain and HTML', () => {
    const converter = getConverter(Platform.Slack);
    const result = converter.convert('**bold text**');
    expect(result.payload['text/plain']).toBe('*bold text*');
    expect(result.payload['text/html']).toContain('<b>bold text</b>');
  });

  it('SMS conversion produces only plain text', () => {
    const converter = getConverter(Platform.SMS);
    const result = converter.convert('**bold text**');
    expect(result.payload['text/plain']).toBe('bold text');
    expect(result.payload['text/html']).toBeUndefined();
  });

  it('Twitter truncates long content', () => {
    const longMarkdown = '**' + 'A'.repeat(300) + '**';
    const converter = getConverter(Platform.Twitter);
    const result = converter.convert(longMarkdown);
    expect(result.truncated).toBe(true);
    expect(result.payload['text/plain'].length).toBeLessThanOrEqual(280);
  });
});
