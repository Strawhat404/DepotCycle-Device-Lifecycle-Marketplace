import { defineConfig } from '@playwright/test';

export default defineConfig({
  testDir: './playwright-tests',
  timeout: 60_000,
  use: {
    baseURL: 'http://127.0.0.1:8080',
    trace: 'on-first-retry',
  },
  webServer: {
    command: 'docker-compose up --build',
    url: 'http://127.0.0.1:8080',
    timeout: 180_000,
    reuseExistingServer: true,
  },
});
