import { defineConfig, devices } from '@playwright/test';
import path from 'node:path';

const root = path.resolve(import.meta.dirname, '../../..');
const publicDir = process.env.DXEDITOR_HARNESS_DIR || path.join(root, 'target/dx/dxeditor_harness/debug/web/public');
const port = Number(process.env.DXEDITOR_PORT || 4173);
const outputDir = path.resolve(import.meta.dirname, process.env.DXEDITOR_RESULTS_DIR || 'test-results');
export default defineConfig({
  outputDir,
  testDir: '.', testMatch: '**/*.spec.ts', fullyParallel: false, workers: 1,
  timeout: 30_000, expect: { timeout: 8_000 }, retries: process.env.CI ? 1 : 0,
  reporter: [['list'], ['html', { open: 'never' }], ['json', { outputFile: path.join(outputDir, 'results.json') }]],
  use: { baseURL: `http://127.0.0.1:${port}`, trace: 'retain-on-failure', screenshot: 'only-on-failure' },
  webServer: {
    command: 'python3 serve.py', url: `http://127.0.0.1:${port}`, reuseExistingServer: !process.env.CI,
    env: { DXEDITOR_HARNESS_DIR: publicDir, DXEDITOR_PORT: String(port) },
  },
  projects: [
    { name: 'chromium', use: { ...devices['Desktop Chrome'], launchOptions: process.env.PLAYWRIGHT_CHROMIUM_PATH ? { executablePath: process.env.PLAYWRIGHT_CHROMIUM_PATH } : {} } },
    { name: 'firefox', use: { ...devices['Desktop Firefox'] } },
    { name: 'webkit', use: { ...devices['Desktop Safari'] } },
  ],
});
