import { defineConfig, devices } from '@playwright/test';

const targetPort = Number(process.env.TARGET_PORT || '4173');

export default defineConfig({
  testDir: './tests/e2e',
  testIgnore: process.env.COWOK_ACCEPTANCE_RUN === '1' ? undefined : ['**/acceptance-*.spec.ts'],
  timeout: 60000,
  expect: {
    timeout: 10000
  },
  use: {
    baseURL: process.env.BASE_URL || `http://127.0.0.1:${targetPort}`,
    trace: 'retain-on-failure',
    screenshot: 'only-on-failure',
    video: process.env.CI ? 'retain-on-failure' : 'off',
  },
  webServer: {
    command: `pnpm run preview --port ${targetPort} --host 127.0.0.1`,
    port: targetPort,
    reuseExistingServer: !process.env.CI,
    timeout: 30000,
  },
  projects: [
    {
      name: 'chromium',
      use: { ...devices['Desktop Chrome'] },
    },
  ],
});
