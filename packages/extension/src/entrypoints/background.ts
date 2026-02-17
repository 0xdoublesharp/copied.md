// Must be first — shims `document` for micromark's entity decoder in service worker
import '@/lib/dom-polyfill';

import { getConverter } from '@copied-md/converters';
import {
  MENU_PARENT_ID,
  PLATFORM_MENU_IDS,
  PLATFORM_LABELS,
  menuIdToPlatform,
} from '@/lib/constants';
import { writeToClipboard } from '@/lib/clipboard-service';

export default defineBackground(() => {
  // Create context menus on extension install/update
  browser.runtime.onInstalled.addListener(() => {
    // Parent menu item
    browser.contextMenus.create({
      id: MENU_PARENT_ID,
      title: 'Copy Markdown To...',
      contexts: ['selection'],
    });

    // Child menu item for each platform
    for (const [platform, menuId] of Object.entries(PLATFORM_MENU_IDS)) {
      browser.contextMenus.create({
        id: menuId,
        parentId: MENU_PARENT_ID,
        title: PLATFORM_LABELS[platform as keyof typeof PLATFORM_LABELS],
        contexts: ['selection'],
      });
    }
  });

  // Handle context menu clicks
  browser.contextMenus.onClicked.addListener(async (info, tab) => {
    const selectedText = info.selectionText;
    if (!selectedText) return;

    const platform = menuIdToPlatform(info.menuItemId as string);
    if (!platform) return;

    try {
      const converter = getConverter(platform);
      const result = converter.convert(selectedText);

      // Send payload to content script for clipboard write + toast notification
      await writeToClipboard(
        result.payload,
        tab?.id,
        PLATFORM_LABELS[platform],
        result.truncated,
      );
    } catch (error) {
      console.error('copied.md: conversion failed', error);
    }
  });
});
