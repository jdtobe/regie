const {test, expect} = require('@playwright/test');
const {generated_workspace, workspace, apply} = require('./fixture');

const palette = ['36483a','e08a83','a8c59a','d6bf86','92b4ce','b6a1cc','8cbfba','d8dfd3',
  '819082','f1aaa1','c1dbaf','e9d4a0','b2d0e8','d0bce3','acd8d1','f3f4eb'];

async function start(page, source = 'regie', appearance = 'system', fallback = false) {
  if (fallback) await page.addInitScript(() => {
    const getContext = HTMLCanvasElement.prototype.getContext;
    HTMLCanvasElement.prototype.getContext = function(type, ...args) {
      return type.startsWith('webgl') ? null : getContext.call(this, type, ...args);
    };
  });
  await generated_workspace(page, source, appearance);
  await apply(page, {type:'create_tab',id:1}, {type:'select_tab',id:1},
    {type:'tab_metadata',tabs:[{id:1,title:'palette',manual:null,busy:false}],active:1});
}
async function output(page, data) {
  await page.evaluate(() => {window.messages = window.messages.filter(m => m.type !== 'output_ack');});
  await apply(page, {type:'output',id:1,sequence:1,data:Buffer.from(data).toString('base64')});
  await expect.poll(() => page.evaluate(() => window.messages.some(m => m.type === 'output_ack'))).toBe(true);
}
// Decode actual rendered pixels with the browser's PNG decoder, not xterm internals.
async function pixels(page) {
  const png = await page.locator('.xterm-screen').first().screenshot();
  return page.evaluate(async base64 => {
    const image = new Image(); image.src = `data:image/png;base64,${base64}`; await image.decode();
    const canvas = document.createElement('canvas'); canvas.width=image.width; canvas.height=image.height;
    const ctx=canvas.getContext('2d'); ctx.drawImage(image,0,0);
    const data=ctx.getImageData(0,0,image.width,image.height).data, counts={};
    for(let i=0;i<data.length;i+=4) {
      const key = [...data.slice(i,i+3)].map(c=>c.toString(16).padStart(2,'0')).join('');
      counts[key]=(counts[key]||0)+1;
    }
    return counts;
  }, png.toString('base64'));
}

for (const fallback of [false, true]) {
  test(`all_sixteen_ansi_slots_render (${fallback ? 'DOM fallback' : 'WebGL'})`, async ({page}) => {
    await start(page, 'regie', 'system', fallback);
    if (!fallback) test.skip(await page.locator('.xterm-screen canvas').count() === 0, 'WebGL unavailable in this WebKit environment');
    for (const background of [false, true]) {
      const text = palette.map((_, i) => `\x1b[${i < 8 ? (background ? 40 : 30)+i : (background ? 100 : 90)+i-8}m${background ? '    ' : '████'}\x1b[0m`).join('');
      await output(page, '\x1b[2J\x1b[H'+text+'\r\n');
      const counts = await pixels(page);
      for(const color of palette) expect(counts[color] || 0, `${background?'background':'foreground'} ${color}`).toBeGreaterThan(40);
    }
  });
}

test('ui_appearance_never_mutates_regie_palette', async ({page}) => {
  for(const appearance of ['system','light','dark']) {
    await start(page, 'regie', appearance, true);
    const strips = [];
    for(const colorScheme of ['light','dark']) {
      await page.emulateMedia({colorScheme});
      await output(page, '\x1b[2J\x1b[H████  normal \x1b[1mbold\x1b[0m \x1b[2mdim\x1b[0m \x1b[7minverse\x1b[0m \x1b[4munderline\x1b[0m\r\n');
      const counts = await pixels(page);
      expect(counts['121615']).toBeGreaterThan(1000);
      expect(counts['dfe6dc']).toBeGreaterThan(100);
      const strip = await page.locator('#tabstrip').evaluate(node=>getComputedStyle(node).backgroundColor);
      expect(strip).not.toBe('rgb(18, 22, 21)');
      strips.push(strip);
    }
    if(appearance === 'system') expect(strips[0]).not.toBe(strips[1]);
    else expect(strips[0]).toBe(strips[1]);
  }
});

test('macos_fixture_does_not_inherit_regie_selection', async ({page}) => {
  await start(page, 'macos', 'light', true);
  await output(page, '████ profile colors\r\n');
  let counts = await pixels(page);
  expect(counts['212121']).toBeGreaterThan(1000);
  expect(counts['d0d0d0']).toBeGreaterThan(100);
  expect(counts['344a3b'] || 0).toBe(0);
});

test('theme_updates_all_existing_sessions_and_preserves_their_buffers', async ({page}) => {
  await workspace(page);
  await apply(page, {type:'create_tab',id:1}, {type:'create_tab',id:2}, {type:'select_tab',id:1});
  await output(page, 'persistent output');
  const before = await page.locator('.xterm-screen').first().screenshot();
  await apply(page, {type:'terminal_theme',theme:JSON.stringify({theme:{background:'#123456',foreground:'#abcdef'},fontFamily:'Menlo',fontSize:15})});
  expect(await page.locator('.xterm-screen').count()).toBe(2);
  const after = await page.locator('.xterm-screen').first().screenshot();
  expect(after.equals(before)).toBe(false);
  expect((await pixels(page))['123456'] || 0).toBeGreaterThan(100);
  await expect(page.locator('[data-terminal-id="1"] .xterm-accessibility-tree')).toContainText('persistent output');
  expect(await page.locator('[data-terminal-id="1"] .xterm').evaluate(node=>getComputedStyle(node).fontFamily)).toContain('Menlo');
  const selectedFontSize = await page.locator('[data-terminal-id="1"] .xterm').evaluate(node=>getComputedStyle(node).fontSize);
  await apply(page,{type:'terminal_theme',theme:JSON.stringify({theme:{background:'#654321',foreground:'#abcdef'}})});
  expect(await page.locator('[data-terminal-id="1"] .xterm').evaluate(node=>getComputedStyle(node).fontFamily)).toContain('Menlo');
  expect(await page.locator('[data-terminal-id="1"] .xterm').evaluate(node=>getComputedStyle(node).fontSize)).toBe(selectedFontSize);
  await apply(page,{type:'select_tab',id:2});
  await expect(page.locator('[data-terminal-id="2"] .xterm')).toBeVisible();
});

test('regie_drag_selection_and_cursor_use_candidate_colors', async ({page}) => {
  await start(page, 'regie', 'dark', true);
  await output(page, '████████████\r\n');
  // Blink is deliberately unchanged; poll until its visible phase.
  await expect.poll(async () => (await pixels(page))['c6d8be'] || 0).toBeGreaterThan(20);
  const box = await page.locator('.xterm-screen').boundingBox();
  await page.mouse.move(box.x+1, box.y+8); await page.mouse.down();
  await page.mouse.move(box.x+90, box.y+8, {steps:10}); await page.mouse.up();
  await expect.poll(async () => (await pixels(page))['344a3b'] || 0).toBeGreaterThan(20);
  const counts = await pixels(page);
  expect(counts['f2f4ed']).toBeGreaterThan(20);
});
