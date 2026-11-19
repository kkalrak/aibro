(() => {
  const token = __SNAPSHOT_TOKEN__;
  const state = {token, nodes: new Map()};
  globalThis.__aibro = state;
  const elements = [];
  const visible = el => {const r=el.getBoundingClientRect(),s=getComputedStyle(el);return r.width>0&&r.height>0&&s.visibility!=='hidden'&&s.display!=='none'};
  const clean = s => (s||'').replace(/\s+/g,' ').trim().slice(0,240);
  const label = el => clean(el.getAttribute('aria-label') || (el.getAttribute('aria-labelledby')||'').split(/\s+/).map(id=>document.getElementById(id)?.textContent||'').join(' ') || Array.from(el.labels||[]).map(l=>{const copy=l.cloneNode(true);copy.querySelectorAll('input,textarea,select,button').forEach(n=>n.remove());return copy.textContent}).join(' ') || el.getAttribute('alt') || el.innerText || el.getAttribute('placeholder') || el.getAttribute('title'));
  const role = el => el.getAttribute('role') || ({A:'link',BUTTON:'button',SELECT:'combobox',TEXTAREA:'textbox',SUMMARY:'button',H1:'heading',H2:'heading',H3:'heading',H4:'heading',H5:'heading',H6:'heading',IMG:'img'}[el.tagName]) || (el.tagName==='INPUT'?({checkbox:'checkbox',radio:'radio',submit:'button',button:'button',range:'slider'}[el.type]||'textbox'):'generic');
  let count=0, truncated=false;
  function walk(root) {
    for(const el of root.querySelectorAll('*')) {
      if(el.shadowRoot) walk(el.shadowRoot);
      if(!visible(el)) continue;
      if(!el.matches('a[href],button,input:not([type=hidden]),select,textarea,summary,[role],[contenteditable=true],[tabindex],h1,h2,h3,h4,h5,h6,img[alt]')) continue;
      if(count>=600) {truncated=true;continue}
      const id=`${token}:e${++count}`;
      state.nodes.set(id,el);
      const item={ref:id,role:role(el),name:label(el)};
      if(el.disabled||el.getAttribute('aria-disabled')==='true') item.disabled=true;
      if('checked' in el) item.checked=el.checked;
      if('value' in el && el.type!=='password') item.value=String(el.value).slice(0,240);
      if(el.type==='password') item.value='[redacted]';
      if(el.href) item.href=el.href;
      if(el.tagName==='SELECT') item.options=Array.from(el.options).map(o=>({value:o.value,label:o.label,selected:o.selected}));
      elements.push(item);
    }
  }
  walk(document);
  return {snapshot:token,url:location.href,title:document.title,readyState:document.readyState,
    text:(document.body?.innerText||'').slice(0,16000),elements,truncated,
    frames:Array.from(document.querySelectorAll('iframe')).map(f=>({src:f.src,title:f.title})),
    limitations:['DOM summary, not a complete accessibility tree','Frame contents require separate support; refs cover the main document and open shadow roots']};
})()
