window.createTabStrip = (post, config) => {
  'use strict';
  if (!config.workspace) return null;
  const root = document.getElementById('tabstrip');
  root.hidden = false;
  document.body.dataset.tabPosition = config.tab_position || 'bottom';
  document.body.dataset.appearance = config.appearance || 'system';
  const list = document.createElement('div');
  list.className = 'tab-list'; list.setAttribute('role','tablist'); list.setAttribute('aria-label','Terminal sessions');
  const plus = document.createElement('button');
  plus.className = 'new-tab'; plus.textContent = '+'; plus.setAttribute('aria-label','New tab'); plus.title = 'New tab (⌘T)';
  plus.onclick = () => post({type:'new_tab'});
  root.append(list, plus);
  let tabs = [], active = null, editor = null;
  function render() {
    const existing=new Map([...list.children].map(item=>[Number(item.dataset.tabId),item]));
    let index=0;
    for (const tab of tabs) {
      const retained=existing.get(tab.id);
      if(retained){
        const button=retained.querySelector('.tab');
        button.setAttribute('aria-selected',tab.id===active);button.setAttribute('aria-label',tab.title);button.title=tab.title;
        let name=retained.querySelector('.tab-name');
        if(!name){name=document.createElement('span');name.className='tab-name';retained.querySelector('input').replaceWith(name);}
        name.textContent=tab.title;
        retained.querySelector('.close-tab').setAttribute('aria-label',`Close ${tab.title}`);
        const dot=button.querySelector('.busy');
        if(dot&&!tab.busy)dot.remove();
        else if(!dot&&tab.busy){const dot=document.createElement('span');dot.className='busy';dot.setAttribute('aria-hidden','true');button.prepend(dot);}
        retained.classList.toggle('active',tab.id===active);
        if(list.children[index]!==retained)list.insertBefore(retained,list.children[index]||null);
        index++;
        if(tab.id===active)requestAnimationFrame(()=>retained.scrollIntoView({block:'nearest',inline:'nearest'}));
        continue;
      }
      const item = document.createElement('div'); item.className = 'tab-item';
      item.dataset.tabId=String(tab.id);
      const button = document.createElement('button');
      button.className = 'tab'; button.setAttribute('role','tab'); button.setAttribute('aria-selected',tab.id === active);
      button.setAttribute('aria-label',tab.title); button.title = tab.title;
      if (tab.busy) {const dot=document.createElement('span');dot.className='busy';dot.setAttribute('aria-hidden','true');button.append(dot);}
      const name = document.createElement('span');name.className='tab-name';name.textContent=tab.title;button.append(name);
      button.onclick = () => post({type:'select_tab',id:tab.id});
      button.ondblclick = () => {
        if (editor) return;
        const name=button.querySelector('.tab-name');
        if(!name)return;
        const input = document.createElement('input');input.className='tab-name-editor';input.setAttribute('aria-label','Tab name');
        input.value=tabs.find(t=>t.id===tab.id)?.manual || '';input.placeholder='Automatic name';input.maxLength=512;
        editor={id:tab.id,input};name.replaceWith(input);
        const finish = save => {
          if (!editor) return;
          const text=input.value;editor=null;render();
          if (save) post({type:'rename_tab',id:tab.id,text});
        };
        input.onclick=event=>event.stopPropagation();input.ondblclick=event=>event.stopPropagation();
        input.onkeydown=event=>{
          event.stopPropagation();
          if(event.key==='Enter'){event.preventDefault();finish(true);}
          else if(event.key==='Escape'){event.preventDefault();finish(false);}
        };
        input.onblur=()=>finish(false);
        input.focus();input.select();
      };
      const close=document.createElement('button');close.className='close-tab';close.textContent='×';close.setAttribute('aria-label',`Close ${tab.title}`);
      close.onclick=()=>post({type:'close_tab',id:tab.id});
      item.classList.toggle('active',tab.id===active);item.append(button,close);list.insertBefore(item,list.children[index]||null);index++;
      if(tab.id===active) requestAnimationFrame(()=>item.scrollIntoView({block:'nearest',inline:'nearest'}));
    }
    for(const [id,item] of existing){if(!tabs.some(tab=>tab.id===id))item.remove();}
  }
  document.addEventListener('keydown',event=>{
    if(event.target.closest?.('input, [contenteditable="true"]') && !event.target.classList.contains('xterm-helper-textarea')) return;
    let message=null;
    if(event.metaKey && !event.ctrlKey && !event.altKey && /^[1-9]$/.test(event.key)) message={type:'select_number',number:Number(event.key)};
    else if(event.ctrlKey && !event.metaKey && !event.altKey && event.key==='Tab') message={type:'cycle_tab',backwards:event.shiftKey};
    if(message){event.preventDefault();event.stopImmediatePropagation();if(!event.repeat)post(message);}
  },true);
  return {update(next,selected){
    tabs=next;active=selected;
    if(editor&&editor.id===selected&&tabs.some(t=>t.id===editor.id)){
      for(const item of list.children){const selected=Number(item.dataset.tabId)===active;item.classList.toggle('active',selected);item.querySelector('.tab').setAttribute('aria-selected',selected);}
      return;
    }
    const wasEditing=!!editor;editor=null;render();
    if(wasEditing)window.regie?.focus(selected);
  }};
};
