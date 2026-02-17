import js from '@eslint/js';
import tseslint from 'typescript-eslint';
import pluginVue from 'eslint-plugin-vue';
import prettier from 'eslint-config-prettier';

export default [
  js.configs.recommended,
  ...tseslint.configs.recommended,
  ...pluginVue.configs['flat/recommended'],
  prettier,
  {
    files: ['**/*.vue'],
    languageOptions: {
      parserOptions: {
        parser: tseslint.parser,
      },
    },
  },
  {
    files: ['**/*.ts', '**/*.vue'],
    rules: {
      '@typescript-eslint/no-unused-vars': ['error', { argsIgnorePattern: '^_' }],
      '@typescript-eslint/explicit-function-return-type': 'off',
      'vue/multi-word-component-names': 'off',
    },
  },
  {
    files: ['**/*.vue'],
    languageOptions: {
      globals: {
        navigator: 'readonly',
        document: 'readonly',
        ClipboardItem: 'readonly',
        Blob: 'readonly',
        setTimeout: 'readonly',
        requestAnimationFrame: 'readonly',
        crypto: 'readonly',
        console: 'readonly',
      },
    },
  },
  {
    ignores: [
      '.output/',
      '**/.output/',
      '**/.wxt/',
      'node_modules/',
      '**/node_modules/',
      'dist/',
      'coverage/',
    ],
  },
];
