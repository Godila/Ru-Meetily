/// <reference types="vitest" />
import { defineConfig } from 'vitest/config';
import react from '@vitejs/plugin-react';
import path from 'node:path';

// Vitest configuration for the Convoic frontend.
//
// - `@/` alias mirrors tsconfig.json `paths` so imports like
//   `@/lib/summary-languages` resolve in tests.
// - jsdom environment gives us `window`/`localStorage`/DOM APIs for the
//   React-hook tests and the localStorage-backed preferences helpers.
// - globals (`describe`, `it`, `expect`) match the vitest convention; the
//   matching types come from vitest/globals in tsconfig.vitest.json.
export default defineConfig({
  plugins: [react()],
  resolve: {
    alias: {
      '@': path.resolve(__dirname, './src'),
    },
  },
  test: {
    globals: true,
    environment: 'jsdom',
    setupFiles: ['./vitest.setup.ts'],
    // Exclude build artefacts; the src-tauri Rust crate is tested with cargo.
    exclude: ['**/node_modules/**', '**/dist/**', '**/src-tauri/**'],
    // Use the same Node version that ships with the test runner. Keeping the
    // pool explicit avoids surprises if a default changes upstream.
    pool: 'forks',
  },
});
