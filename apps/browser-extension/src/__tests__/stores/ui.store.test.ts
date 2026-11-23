import { describe, it, expect, beforeEach } from 'vitest';
import { setActivePinia, createPinia } from 'pinia';
import { useUiStore } from '@/stores/ui.store';
import { Platform } from '@0xdoublesharp/copied-md-converters';

describe('useUiStore', () => {
  beforeEach(() => {
    setActivePinia(createPinia());
  });

  it('starts with empty state', () => {
    const store = useUiStore();
    expect(store.inputMarkdown).toBe('');
    expect(store.selectedPlatform).toBeNull();
    expect(store.copied).toBe(false);
  });

  it('selects a platform', () => {
    const store = useUiStore();
    store.selectPlatform(Platform.Slack);
    expect(store.selectedPlatform).toBe('slack-paste');
  });

  it('resets copied flag when changing platform', () => {
    const store = useUiStore();
    store.copied = true;
    store.selectPlatform(Platform.Discord);
    expect(store.copied).toBe(false);
  });

  it('returns null conversionResult when no input', () => {
    const store = useUiStore();
    store.selectPlatform(Platform.Slack);
    expect(store.conversionResult).toBeNull();
  });

  it('returns null conversionResult when no platform selected', () => {
    const store = useUiStore();
    store.inputMarkdown = '**hello**';
    expect(store.conversionResult).toBeNull();
  });

  it('computes conversion result when both input and platform are set', () => {
    const store = useUiStore();
    store.inputMarkdown = '**hello**';
    store.selectPlatform(Platform.Slack);
    expect(store.conversionResult).not.toBeNull();
    expect(store.previewText).toBe('hello');
  });

  it('computes HTML converter result', () => {
    const store = useUiStore();
    store.inputMarkdown = '**hello**';
    store.selectPlatform('html-source');
    expect(store.previewText).toContain('<strong>hello</strong>');
  });

  it('does not truncate for Twitter', () => {
    const store = useUiStore();
    store.inputMarkdown = '**' + 'A'.repeat(300) + '**';
    store.selectPlatform(Platform.Twitter);
    expect(store.previewText).toHaveLength(300);
    expect(store.conversionResult?.diagnostics[0]?.code).toBe('length-exceeded');
  });

  it('detects HTML payload', () => {
    const store = useUiStore();
    store.inputMarkdown = '**hello**';
    store.selectPlatform(Platform.Slack);
    expect(store.hasHtml).toBe(true);
  });

  it('no HTML payload for SMS', () => {
    const store = useUiStore();
    store.inputMarkdown = '**hello**';
    store.selectPlatform(Platform.SMS);
    expect(store.hasHtml).toBe(false);
  });

  it('resets all state', () => {
    const store = useUiStore();
    store.inputMarkdown = 'test';
    store.selectPlatform(Platform.Slack);
    store.copied = true;
    store.reset();
    expect(store.inputMarkdown).toBe('');
    expect(store.selectedPlatform).toBeNull();
    expect(store.copied).toBe(false);
  });

  it('marks as copied', () => {
    const store = useUiStore();
    store.markCopied();
    expect(store.copied).toBe(true);
  });
});
