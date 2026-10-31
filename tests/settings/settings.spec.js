const {test, expect} = require('@playwright/test');
const {settings} = require('./fixture');

test('renders the eleven settings as labeled controls', async ({page}) => {
  await settings(page);
  await expect(page.locator('[data-setting]')).toHaveCount(11);
  await expect(page.getByRole('searchbox', {name:'Search settings'})).toBeVisible();
});

test('failed changes expose an explicit retry button and send a retry request', async ({page}) => {
  await settings(page);
  await page.evaluate(() => window.settings.render({fields:[], appearance:'light', warning:'Disk full', retryable:true, can_retry:true}));
  const retry = page.getByRole('button', {name:'Retry saving settings'});
  await expect(retry).toBeVisible();
  await retry.click();
  await expect.poll(() => page.evaluate(() => window.requests.at(-1))).toEqual({type:'retry'});
});

test('retry stays visible but disabled while retry is not safe', async ({page}) => {
  await settings(page);
  await page.evaluate(() => window.settings.render({fields:[], appearance:'light', warning:'Configuration needs attention', retryable:true, can_retry:false}));
  const retry = page.getByRole('button', {name:'Retry saving settings'});
  await expect(retry).toBeVisible();
  await expect(retry).toBeDisabled();
});

test('global shortcut is captured as a key combination rather than typed text', async ({page}) => {
  await settings(page);
  await page.getByRole('button', {name: 'Record global hotkey'}).click();
  await page.keyboard.press('Control+Shift+K');
  await expect.poll(() => page.evaluate(() => window.requests.at(-1))).toEqual({
    type: 'edit', key: 'hotkey', value: 'Ctrl+Shift+K'
  });
});

test('function keys can be recorded without modifiers', async ({page}) => {
  await settings(page);
  await page.getByRole('button', {name: 'Record global hotkey'}).click();
  await page.keyboard.press('F12');
  await expect.poll(() => page.evaluate(() => window.requests.at(-1))).toEqual({type:'edit', key:'hotkey', value:'F12'});
});

test('search filters settings across sections and clear restores the list', async ({page}) => {
  await settings(page);
  await page.getByRole('searchbox', {name: 'Search settings'}).fill('terminal color');
  await expect(page.locator('[data-setting]:visible')).toHaveCount(1);
  await expect(page.locator('[data-setting="terminal_theme"]')).toBeVisible();
  await page.getByRole('button', {name: /clear search/i}).click();
  await expect(page.locator('[data-setting]:visible')).toHaveCount(11);
});

test('asynchronous updates do not discard a number being edited', async ({page}) => {
  await settings(page);
  const width = page.getByRole('spinbutton', {name: 'Width value'});
  await width.fill('67');
  await page.evaluate(() => window.settings.render({fields:[{key:'width_percent',value:100,default:100}], appearance:'light'}));
  await expect(page.getByRole('spinbutton', {name: 'Width value'})).toHaveValue('67');
});

test('empty search state and reset confirmation are actionable', async ({page}) => {
  await settings(page);
  await page.getByRole('searchbox', {name: 'Search settings'}).fill('no such preference');
  await expect(page.getByText('No settings match your search.')).toBeVisible();
  await page.getByRole('button', {name: 'Reset All…'}).click();
  await expect(page.getByRole('dialog')).toBeVisible();
  await page.getByRole('button', {name: 'Reset All', exact: true}).click();
  await expect.poll(() => page.evaluate(() => window.requests.at(-1))).toEqual({type:'reset_all'});
});
