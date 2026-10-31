const fs = require('node:fs');
const path = require('node:path');
async function settings(page) {
  const root = path.resolve(__dirname, '../..');
  await page.goto('about:blank');
  await page.setContent(fs.readFileSync(path.join(root, 'assets/settings/index.html'), 'utf8'));
  await page.addStyleTag({content: fs.readFileSync(path.join(root, 'assets/settings/settings.css'), 'utf8')});
  await page.addScriptTag({content: fs.readFileSync(path.join(root, 'assets/settings/settings.js'), 'utf8')});
  await page.evaluate(() => { window.requests = []; window.ipc = {postMessage: body => window.requests.push(JSON.parse(body))}; });
  await page.evaluate(() => window.settings.render({fields:[], appearance:'light', warning:null, retryable:false, can_retry:false}));
}
module.exports = {settings};
