import { defineConfig } from 'vitest/config';
import react from '@vitejs/plugin-react';
import path from 'path';

export default defineConfig({
  plugins: [react()],
  test: {
    globals: true,
    environment: 'jsdom',
    setupFiles: ['./src/test/setup.ts'],
    include: ['src/**/*.{test,spec}.{ts,tsx}', 'tests/integration/**/*.{test,spec}.{ts,tsx}'],
    exclude: ['node_modules', 'dist', 'tests/e2e/**'],
    coverage: {
      provider: 'v8',
      reporter: ['text', 'html', 'lcov'],
      // Emit LCOV even when tests fail. Vitest's default is to skip coverage
      // on failure, which hides the coverage of the passing tests and starves
      // SonarCloud of data. We'd rather see partial coverage than none.
      reportOnFailure: true,
      exclude: [
        'node_modules/',
        // Test code (fixtures, helpers, specs) is not production code.
        'src/test/',
        '**/*.test.{ts,tsx}',
        '**/*.spec.{ts,tsx}',
        'tests/**',
        // Pure type declarations / config / entry points.
        '**/*.d.ts',
        '**/*.config.*',
        'src/main.tsx',
        'src/vite-env.d.ts',
        // Barrel files — re-export only, nothing to test.
        '**/index.ts',
        // Pure TypeScript interfaces — no runtime code to test.
        'src/shared/types/**',
        // Pure constants — no logic to test.
        'src/shared/constants/**',
        // shadcn/ui primitives — thin re-exports of Radix UI, no app logic.
        'src/shared/ui/badge.tsx',
        'src/shared/ui/button.tsx',
        'src/shared/ui/card.tsx',
        'src/shared/ui/dialog.tsx',
        'src/shared/ui/dropdown-menu.tsx',
        'src/shared/ui/input.tsx',
        'src/shared/ui/label.tsx',
        'src/shared/ui/modal-overlay.tsx',
        'src/shared/ui/scroll-area.tsx',
        'src/shared/ui/select.tsx',
        'src/shared/ui/separator.tsx',
        'src/shared/ui/switch.tsx',
        'src/shared/ui/tabs.tsx',
        'src/shared/ui/textarea.tsx',
        'src/shared/ui/tooltip.tsx',
        'src/shared/ui/utils.ts',
        // Pure TypeScript interface declarations — no runtime code.
        'src/services/transport/interface.ts',
        // D3-force knowledge graph — renders SVG using D3 APIs not available in jsdom.
        'src/features/observatory/GraphCanvas.tsx',
        // Large settings/integrations panels with complex form state — covered indirectly
        // via integration tests; excluding avoids counting dead interactive branches
        // that require full browser event chains unavailable in jsdom.
        'src/features/integrations/WebIntegrationsPanel.tsx',
      ],
    },
  },
  resolve: {
    alias: {
      '@': path.resolve(__dirname, './src'),
    },
  },
});
