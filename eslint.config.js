// ESLint 9 flat config.
//
// Why this file exists
// --------------------
// `package.json` has always declared a `lint` script and CI has always called
// it, but no config file was ever committed. ESLint 9 dropped the implicit
// `.eslintrc` lookup, so `npm run lint` failed with "couldn't find an
// eslint.config.js" — meaning the lint gate in CI could never have passed.
//
// Scope is deliberately narrow: `src` only. The Rust side is covered by clippy,
// `e2e/` is Playwright's, and `npm-package/` is plain CommonJS Node shipped to
// users as-is.

import js from '@eslint/js';
import globals from 'globals';
import tseslint from 'typescript-eslint';
import reactHooks from 'eslint-plugin-react-hooks';
import reactRefresh from 'eslint-plugin-react-refresh';

export default tseslint.config(
  {
    // Build output, dependencies and generated bundles are not ours to lint.
    ignores: ['dist/**', 'node_modules/**', 'src-tauri/**', 'test-results/**', 'playwright-report/**'],
  },
  js.configs.recommended,
  ...tseslint.configs.recommended,
  {
    files: ['src/**/*.{ts,tsx}'],
    languageOptions: {
      ecmaVersion: 2022,
      globals: globals.browser,
      parserOptions: {
        ecmaFeatures: { jsx: true },
      },
    },
    plugins: {
      'react-hooks': reactHooks,
      'react-refresh': reactRefresh,
    },
    rules: {
      ...reactHooks.configs.recommended.rules,
      'react-refresh/only-export-components': ['warn', { allowConstantExport: true }],

      // `noUnusedLocals`/`noUnusedParameters` in tsconfig.json already fail the
      // type check on genuinely dead bindings. Here we only add the convention
      // that a leading underscore marks an intentionally unused binding.
      '@typescript-eslint/no-unused-vars': [
        'error',
        {
          argsIgnorePattern: '^_',
          varsIgnorePattern: '^_',
          caughtErrorsIgnorePattern: '^_',
        },
      ],

      // The whole point of the codebase's type discipline. `as any` and friends
      // are a hard block, so surface them as errors rather than warnings.
      '@typescript-eslint/no-explicit-any': 'error',
    },
  },
  {
    // Tests reach into internals and assert on loose shapes; the strictness
    // that protects application code just gets in the way here.
    files: ['src/**/*.test.{ts,tsx}'],
    rules: {
      '@typescript-eslint/no-explicit-any': 'off',
    },
  },
);
