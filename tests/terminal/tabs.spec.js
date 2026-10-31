const {test, expect} = require('@playwright/test');
const {workspace, apply} = require('./fixture');
const output = (id, sequence, text) => ({type: 'output', id, sequence, data: Buffer.from(text).toString('base64')});

test('ready_input_resize_and_native_paste_are_real_ipc', async ({page}) => {
  await workspace(page);
  expect(await page.evaluate(()=>window.messages[0])).toEqual({type:'ready'});
  await apply(page,{type:'create_tab',id:1},{type:'select_tab',id:1});
  await page.keyboard.type('x');
  await page.keyboard.press('Meta+v');
  await expect.poll(()=>page.evaluate(()=>window.messages.some(m=>m.type==='input'&&m.id===1&&m.data==='x'))).toBe(true);
  expect(await page.evaluate(()=>window.messages.some(m=>m.type==='paste'&&m.id===1))).toBe(true);
  expect(await page.evaluate(()=>window.messages.some(m=>m.type==='resize'&&m.id===1&&m.cols>0&&m.rows>0))).toBe(true);
});

test('two_tabs_retain_distinct_screens', async ({page}) => {
  await workspace(page);
  await apply(page, {type:'create_tab',id:1},{type:'create_tab',id:2},{type:'select_tab',id:1},output(1,1,'first-screen'),output(2,1,'second-screen'));
  await expect(page.locator('[data-terminal-id="1"] .xterm-accessibility-tree')).toContainText('first-screen');
  await apply(page,{type:'select_tab',id:2});
  await expect(page.locator('[data-terminal-id="2"] .xterm-accessibility-tree')).toContainText('second-screen');
});
test('removed_tab_cannot_receive_late_output', async ({page}) => {
  await workspace(page);
  await apply(page,{type:'create_tab',id:1},{type:'remove_tab',id:1},output(1,1,'late'));
  expect(await page.locator('[data-terminal-id]').count()).toBe(0);
  expect(await page.evaluate(() => window.messages.some(m=>m.type==='output_ack'))).toBe(false);
});
test('inactive_session_receives_output_and_late_write_callback_is_ignored_after_removal', async ({page}) => {
  await workspace(page);
  await apply(page, {type:'create_tab',id:1}, {type:'create_tab',id:2}, {type:'select_tab',id:1}, output(2,1,'inactive-output'));
  await apply(page, {type:'select_tab',id:2});
  await expect(page.locator('[data-terminal-id="2"] .xterm-accessibility-tree')).toContainText('inactive-output');
  await apply(page, output(2,2,'late-callback'), {type:'remove_tab',id:2});
  await page.waitForTimeout(50);
  expect(await page.evaluate(()=>window.messages.some(message=>message.type==='output_ack'&&message.sequence===2))).toBe(false);
});
test('paste_goes_to_requesting_id', async ({page}) => {
  await workspace(page);
  await apply(page,{type:'create_tab',id:1},{type:'create_tab',id:2},{type:'select_tab',id:2},{type:'paste',id:1,text:'hello'});
  await expect.poll(()=>page.evaluate(()=>window.messages.filter(m=>m.type==='input'))).toEqual([{type:'input',id:1,data:'hello'}]);
});
test('osc_titles_report_the_originating_id', async ({page}) => {
  await workspace(page);
  await apply(page,{type:'create_tab',id:1},{type:'create_tab',id:2},output(1,1,'\x1b]2;ssh server\x07'));
  await expect.poll(()=>page.evaluate(()=>window.messages.find(m=>m.type==='title'))).toEqual({type:'title',id:1,text:'ssh server'});
});
test('write_ack_waits_for_parse', async ({page}) => {
  await workspace(page);
  await apply(page,{type:'create_tab',id:1},{type:'select_tab',id:1},output(1,7,'parsed-output'));
  await expect.poll(()=>page.evaluate(()=>window.messages.filter(m=>m.type==='output_ack'))).toEqual([{type:'output_ack',id:1,sequence:7}]);
  await expect(page.locator('[data-terminal-id="1"] .xterm-accessibility-tree')).toContainText('parsed-output');
});
test('switch_refits_only_active_terminal', async ({page}) => {
  await workspace(page);
  await apply(page,{type:'create_tab',id:1},{type:'create_tab',id:2},{type:'select_tab',id:1});
  await page.evaluate(()=>window.messages=[]);
  await page.setViewportSize({width:700,height:400});
  await expect.poll(()=>page.evaluate(()=>window.messages.filter(m=>m.type==='resize').length)).toBeGreaterThan(0);
  expect(await page.evaluate(()=>window.messages.filter(m=>m.type==='resize').every(m=>m.id===1 && m.cols>0 && m.rows>0))).toBe(true);
});
