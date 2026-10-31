const {test,expect}=require('@playwright/test');
const {workspace,apply}=require('./fixture');
const tab=(id,title=`tab-${id}`)=>({id,title,manual:null,busy:false});
const metadata=(tabs,active=1)=>({type:'tab_metadata',tabs,active});

test('bottom_is_default_top_is_configurable',async({page})=>{
  for(const position of ['bottom','top']) {
    await workspace(page,{tab_position:position});
    await apply(page,{type:'create_tab',id:1},{type:'select_tab',id:1},metadata([tab(1)]));
    const strip=await page.locator('#tabstrip').boundingBox(), term=await page.locator('#terminal').boundingBox();
    expect(position==='bottom' ? strip.y>=term.y+term.height-1 : strip.y+strip.height<=term.y+1).toBe(true);
  }
});
test('bar_visible_for_single_tab',async({page})=>{
  await workspace(page);await apply(page,metadata([tab(1)]));
  await expect(page.getByRole('tab',{name:'tab-1'})).toBeVisible();
  await expect(page.getByRole('button',{name:'New tab'})).toBeVisible();
});
test('plus_remains_reachable_when_overflowing',async({page})=>{
  await workspace(page);await page.setViewportSize({width:320,height:400});
  await apply(page,metadata(Array.from({length:30},(_,i)=>tab(i+1))));
  const plus=page.getByRole('button',{name:'New tab'});const rect=await plus.boundingBox();
  expect(rect.x+rect.width).toBeLessThanOrEqual(320);
  await plus.click();expect(await page.evaluate(()=>window.messages.at(-1))).toEqual({type:'new_tab'});
});
test('selected_tab_scrolls_into_view',async({page})=>{
  await workspace(page);await apply(page,metadata(Array.from({length:30},(_,i)=>tab(i+1)),28));
  const box=await page.getByRole('tab',{name:'tab-28',exact:true}).boundingBox();
  expect(box.x).toBeGreaterThanOrEqual(0);expect(box.x+box.width).toBeLessThanOrEqual(1000);
});
test('rename_enter_saves_escape_cancels_empty_restores_auto',async({page})=>{
  await workspace(page);await apply(page,metadata([tab(1)]));
  await page.getByText('tab-1',{exact:true}).dblclick();
  let input=page.getByRole('textbox',{name:'Tab name'});await input.fill('My server');await input.press('Enter');
  expect(await page.evaluate(()=>window.messages.at(-1))).toEqual({type:'rename_tab',id:1,text:'My server'});
  await apply(page,metadata([{...tab(1,'My server'),manual:'My server'}]));
  await page.getByText('My server',{exact:true}).dblclick();input=page.getByRole('textbox',{name:'Tab name'});
  await input.fill('cancel this');await input.press('Escape');await expect(page.getByText('My server',{exact:true})).toBeVisible();
  await page.getByText('My server',{exact:true}).dblclick();input=page.getByRole('textbox',{name:'Tab name'});
  await input.fill('');await input.press('Enter');
  expect(await page.evaluate(()=>window.messages.at(-1))).toEqual({type:'rename_tab',id:1,text:''});
});
test('title_html_is_rendered_as_text',async({page})=>{
  await workspace(page);await apply(page,metadata([tab(1,'<img src=x onerror="window.pwned=1">')]));
  expect(await page.locator('#tabstrip img').count()).toBe(0);
  expect(await page.evaluate(()=>window.pwned)).toBeUndefined();
  await expect(page.getByRole('tab')).toContainText('<img');
});
test('appearance_does_not_change_terminal_palette',async({page})=>{
  const backgrounds=[];
  for(const appearance of ['light','dark']) {
    await workspace(page,{appearance});
    // Exercise real DOM-renderer colours when WebGL is unavailable. The hidden
    // legacy .xterm-viewport stays black in xterm 6; it is not the renderer.
    await page.evaluate(()=>{WebglAddon.WebglAddon=class{constructor(){throw Error('WebGL unavailable')}}});
    await apply(page,{type:'create_tab',id:1},{type:'select_tab',id:1},metadata([tab(1)]));
    expect(await page.locator('body').evaluate(el=>getComputedStyle(el).backgroundColor)).toBe('rgb(28, 28, 28)');
    expect(await page.locator('.xterm-rows').evaluate(el=>getComputedStyle(el).color)).toBe('rgb(208, 208, 208)');
    backgrounds.push(await page.locator('#tabstrip').evaluate(el=>getComputedStyle(el).backgroundColor));
  }
  expect(backgrounds[0]).not.toBe(backgrounds[1]);
});
test('system_colour_scheme_changes_live',async({page})=>{
  await page.emulateMedia({colorScheme:'light'});await workspace(page);await apply(page,metadata([tab(1)]));
  const light=await page.locator('#tabstrip').evaluate(el=>getComputedStyle(el).backgroundColor);
  await page.emulateMedia({colorScheme:'dark'});
  await expect.poll(()=>page.locator('#tabstrip').evaluate(el=>getComputedStyle(el).backgroundColor)).not.toBe(light);
});
test('repeated_keys_do_not_create_duplicate_intents',async({page})=>{
  await workspace(page);await apply(page,metadata([tab(1),tab(2)]));
  await page.evaluate(()=>{document.dispatchEvent(new KeyboardEvent('keydown',{key:'2',metaKey:true,bubbles:true}));document.dispatchEvent(new KeyboardEvent('keydown',{key:'2',metaKey:true,repeat:true,bubbles:true}));});
  expect(await page.evaluate(()=>window.messages.filter(m=>m.type==='select_number'))).toEqual([{type:'select_number',number:2}]);
});
test('rename_editor_does_not_swallow_normal_text_or_close_its_tab',async({page})=>{
  await workspace(page);await apply(page,metadata([tab(1)]));await page.getByText('tab-1',{exact:true}).dblclick();
  const input=page.getByRole('textbox',{name:'Tab name'});await input.fill('abc');await input.press('Control+Tab');
  expect(await page.evaluate(()=>window.messages.some(m=>m.type==='cycle_tab'))).toBe(false);
  expect(await page.getByRole('tab').count()).toBe(1);
});

test('rename_survives_async_selection_round_trips',async({page})=>{
  for (const delay of [0,30]) {
    await workspace(page);await apply(page,{type:'create_tab',id:1},{type:'create_tab',id:2},{type:'select_tab',id:1},metadata([tab(1),tab(2)]));
    await page.evaluate(delay=>{
      const original=window.ipc.postMessage;
      window.ipc.postMessage=body=>{original(body);const message=JSON.parse(body);
        if(message.type==='select_tab')setTimeout(()=>{window.regie.apply({type:'select_tab',id:message.id});window.regie.apply({type:'tab_metadata',tabs:[1,2].map(id=>({id,title:`tab-${id}`,manual:null,busy:false})),active:message.id});},delay);
      };
    },delay);
    await page.getByText('tab-2',{exact:true}).dblclick();
    const input=page.getByRole('textbox',{name:'Tab name'});
    await expect(input).toBeVisible();await input.fill('Still editing');
    await expect(input).toBeFocused();await input.press('Enter');
    await expect.poll(()=>page.evaluate(()=>window.messages.some(m=>m.type==='rename_tab'&&m.id===2&&m.text==='Still editing'))).toBe(true);
  }
});
