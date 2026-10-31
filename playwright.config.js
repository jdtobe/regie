const { defineConfig } = require('@playwright/test');
module.exports = defineConfig({
  testDir: './tests',
  workers: 1,
  reporter: [['list'], ['html', { open: 'never' }]],
  use: { trace: 'retain-on-failure', screenshot: 'only-on-failure', video: 'retain-on-failure' },
  projects: [{name: 'webkit', use: {browserName: 'webkit', viewport: {width: 1000, height: 600}}}]
});
