import { defineConfig } from 'wxt';
import tailwindcss from '@tailwindcss/vite';
import { fileURLToPath } from 'node:url';

export default defineConfig({
  srcDir: 'src',
  publicDir: 'src/public',
  outDir: 'build',
  zip: {
    name: 'copied-md',
    sourcesRoot: fileURLToPath(new URL('../../', import.meta.url)),
    includeSources: [
      'README.md',
      'assets/*.png',
      'docs/portable-macos-app.md',
      'package.json',
      'pnpm-workspace.yaml',
      'pnpm-lock.yaml',
      'tsconfig.base.json',
      'apps/*/package.json',
      'apps/browser-extension/wxt.config.ts',
      'apps/browser-extension/tsconfig*.json',
      'apps/browser-extension/src/**',
      'packages/converters/package.json',
      'packages/converters/tsconfig.json',
      'packages/converters/src/**',
    ],
  },
  modules: ['@wxt-dev/module-vue'],
  manifest: ({ browser }) => ({
    name: 'copied.md',
    description: 'Copy Markdown formatted for any platform',
    permissions: [
      'contextMenus',
      'clipboardWrite',
      'activeTab',
      'storage',
      ...(['chrome', 'edge'].includes(browser) ? ['offscreen' as const, 'scripting' as const] : []),
    ],
    icons: {
      '16': 'icon-16.png',
      '32': 'icon-32.png',
      '48': 'icon-48.png',
      '128': 'icon-128.png',
    },
    commands: {
      'copy-last-target': { description: 'Copy selection using the last format' },
    },
    ...(browser === 'firefox'
      ? {
          browser_specific_settings: {
            gecko: {
              id: 'copied-md@copied.md',
              data_collection_permissions: { required: ['none' as const] },
            },
          },
        }
      : { minimum_chrome_version: '116' }),
  }),
  vite: () => ({
    plugins: [tailwindcss()],
  }),
});
