const fs = require('node:fs');
const path = require('node:path');
const root = path.resolve(__dirname, '../..');
const read = name => fs.readFileSync(path.join(root, 'assets/terminal', name), 'utf8');
async function workspace(page, overrides = {}) {
  const config = {workspace: true, theme: {background: '#1c1c1c', foreground: '#d0d0d0'}, tab_position: 'bottom', appearance: 'system', ...overrides};
  let html = read('index.html');
  for (const [token, content] of Object.entries({
    THEME_BACKGROUND: config.theme.background, XTERM_CSS: read('xterm.css'),
    XTERM_JS: read('xterm.js'), ADDON_FIT_JS: read('addon-fit.js'), ADDON_WEBGL_JS: read('addon-webgl.js'),
    TABS_CSS: fs.existsSync(path.join(root,'assets/terminal/tabs.css')) ? read('tabs.css') : '',
    TABS_JS: fs.existsSync(path.join(root,'assets/terminal/tabs.js')) ? read('tabs.js') : '',
    REGIE_JS: read('regie.js').replace('/*__REGIE_CONFIG__*/', JSON.stringify(config))
  })) html = html.replace(`/*__${token}__*/`, content);
  await page.addInitScript(() => {window.messages=[]; window.ipc={postMessage: body => window.messages.push(JSON.parse(body))};});
  await page.goto('about:blank');
  await page.setContent(html);
}
async function apply(page, ...messages) { await page.evaluate(messages => messages.forEach(message => window.regie.apply(message)), messages); }
async function generated_workspace(page, source = 'regie', appearance = 'system') {
  await page.addInitScript(() => {window.messages=[]; window.ipc={postMessage: body => window.messages.push(JSON.parse(body))};});
  await page.goto('about:blank');
  await page.setContent(fs.readFileSync(path.join(root, 'target/terminal-fixtures', `${source}-${appearance}.html`), 'utf8'));
}
module.exports = {workspace, generated_workspace, apply};
