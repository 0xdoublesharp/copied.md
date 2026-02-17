import { defineConfig } from 'wxt';
import tailwindcss from '@tailwindcss/vite';

export default defineConfig({
  srcDir: 'src',
  modules: ['@wxt-dev/module-vue'],
  manifest: ({ browser }) => ({
    name: 'copied.md',
    description: 'Copy Markdown formatted for any platform',
    version: '1.0.0',
    permissions: [
      'contextMenus',
      'clipboardWrite',
      'activeTab',
      ...(browser === 'chrome' ? ['offscreen' as const] : []),
    ],
    icons: {
      '16': 'icon-16.png',
      '32': 'icon-32.png',
      '48': 'icon-48.png',
      '128': 'icon-128.png',
    },
  }),
  vite: () => ({
    plugins: [tailwindcss()],
  }),
});
