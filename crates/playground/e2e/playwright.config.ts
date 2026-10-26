import { defineConfig, devices } from '@playwright/test';
import path from 'node:path';

const root = path.resolve(import.meta.dirname, '../../..');
const publicDir = process.env.DXPLAYGROUND_DIR || path.join(root, 'target/dx/dxplayground/release/web/public');
const port = Number(process.env.DXPLAYGROUND_PORT || 4174);

export default defineConfig({
  testDir: '.', testMatch: '*.spec.ts', fullyParallel: false, workers: 1,
  timeout: 30_000, expect: { timeout: 10_000 }, retries: process.env.CI ? 1 : 0,
  reporter: [['list'], ['html', { open: 'never' }]],
  use: { baseURL: `http://127.0.0.1:${port}/dxutil/`, trace: 'retain-on-failure', screenshot: 'only-on-failure' },
  webServer: {
    command: 'python3 serve.py', url: `http://127.0.0.1:${port}/dxutil/`, reuseExistingServer: !process.env.CI,
    env: { DXPLAYGROUND_DIR: publicDir, DXPLAYGROUND_PORT: String(port) },
  },
  projects: [
    { name: 'chromium', use: { ...devices['Desktop Chrome'], launchOptions: process.env.PLAYWRIGHT_CHROMIUM_PATH ? { executablePath: process.env.PLAYWRIGHT_CHROMIUM_PATH } : {} } },
  ],
});
