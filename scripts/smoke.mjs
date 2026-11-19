import http from 'node:http';
import fs from 'node:fs';
import assert from 'node:assert/strict';
import {spawn} from 'node:child_process';
const connection=JSON.parse(fs.readFileSync('.aibro/connection.json','utf8'));
const headers={'Authorization':`Bearer ${connection.token}`,'Content-Type':'application/json'};
let checks=0;
async function call(command,ok=true){
  const r=await fetch(connection.url+'/command',{method:'POST',headers,body:JSON.stringify(command)});
  const v=await r.json();assert.equal(v.ok,ok,JSON.stringify(v));checks++;return ok?v.data:v.error;
}
const fixture=http.createServer((req,res)=>{
  res.setHeader('Content-Type','text/html; charset=utf-8');
  if(req.url==='/next'){res.end('<title>Next</title><h1>Destination reached</h1>');return;}
  res.end(`<!doctype html><title>Aibro fixture</title><style>body{font:18px sans-serif;padding:24px}button,input,select{margin:8px;padding:8px}#covered{position:relative}#cover{position:absolute;inset:0;background:#888;z-index:2}</style>
  <h1>Agent test page</h1><label>Name <input id=name></label><label>Secret <input type=password value=never-log-me></label>
  <button id=save onclick="document.querySelector('#result').textContent=document.querySelector('#name').value+':'+event.isTrusted">Save</button>
  <output id=result></output><button disabled>Disabled</button>
  <span id=covered><button>Covered</button><span id=cover></span></span>
  <label>Color<select id=color onchange="document.querySelector('#result').textContent=this.value"><option value=red>Red</option><option value=blue>Blue</option></select></label>
  <a href=/next>Next page</a><div id=shadow></div><script>document.querySelector('#shadow').attachShadow({mode:'open'}).innerHTML='<button>Shadow action</button>'</script>`);
});
await new Promise(resolve=>fixture.listen(0,'127.0.0.1',resolve));
let tab;
try{
  const unauthorized=await fetch(connection.url+'/command',{method:'POST',body:'{"op":"tabs"}'});assert.equal(unauthorized.status,401);checks++;
  const origin=await fetch(connection.url+'/command',{method:'POST',headers:{...headers,Origin:'https://evil.example'},body:'{"op":"tabs"}'});assert.equal(origin.status,401);checks++;
  const opened=await call({op:'open',url:`http://127.0.0.1:${fixture.address().port}/`});tab=opened.tab;
  let page=opened.page;
  const target=name=>{const el=page.elements.find(e=>e.name===name);assert.ok(el,`Missing ${name}`);return el.ref;};
  assert.equal(page.title,'Aibro fixture');assert.ok(page.elements.some(e=>e.name==='Shadow action'));assert.ok(!JSON.stringify(page).includes('never-log-me'));checks++;
  const stale=target('Save');page=await call({op:'fill',tab,target:target('Name'),text:'안녕 AI browser'});
  assert.equal(page.elements.find(e=>e.name==='Name').value,'안녕 AI browser');checks++;
  assert.match(await call({op:'click',tab,target:stale},false),/Stale reference/);
  page=await call({op:'click',tab,target:target('Save')});assert.ok(page.text.includes('안녕 AI browser:true'));checks++;
  assert.match(await call({op:'click',tab,target:target('Disabled')},false),/disabled/);
  assert.match(await call({op:'click',tab,target:target('Covered')},false),/covered/);
  page=await call({op:'select',tab,target:target('Color'),value:'blue'});assert.ok(page.text.includes('blue'));checks++;
  const png=await call({op:'screenshot',tab});assert.ok(Buffer.from(png.base64,'base64').subarray(0,8).equals(Buffer.from([137,80,78,71,13,10,26,10])));checks++;
  page=await call({op:'click',tab,target:target('Next page')});assert.equal(page.title,'Next');checks++;
  page=await call({op:'back',tab});assert.equal(page.title,'Aibro fixture');checks++;
  page=await call({op:'reload',tab});assert.equal(page.title,'Aibro fixture');checks++;
  await call({op:'scroll',tab,y:200});await call({op:'press',tab,key:'Tab'});
  await call({op:'navigate',tab,url:'javascript:alert(1)'},false);
  await call({op:'click',tab,target:'missing',extra:true},false);
  const process=spawn('target/bundle/aibro-agent.exe',['--data-dir',`${processRoot()}/.aibro`],{stdio:['pipe','pipe','pipe']});
  let output='';let errors='';process.stdout.on('data',d=>output+=d);process.stderr.on('data',d=>errors+=d);
  for(const request of [
    {id:1,method:'initialize',params:{protocolVersion:'2025-11-25',capabilities:{},clientInfo:{name:'smoke',version:'1'}}},
    {method:'notifications/initialized'}, {id:2,method:'tools/list'},
    {id:3,method:'tools/call',params:{name:'browser',arguments:{op:'tabs'}}},
    {id:4,method:'tools/call',params:{name:'browser',arguments:{op:'screenshot',tab}}}
  ])process.stdin.write(JSON.stringify({jsonrpc:'2.0',...request})+'\n');
  process.stdin.end();const exit=await new Promise(resolve=>process.on('exit',resolve));assert.equal(exit,0,errors);
  const messages=output.trim().split('\n').map(l=>JSON.parse(l));assert.equal(messages.length,4);assert.equal(messages[0].result.protocolVersion,'2025-11-25');
  assert.equal(messages[1].result.tools[0].name,'browser');assert.equal(messages[2].result.isError,false);assert.equal(messages[3].result.content[0].type,'image');checks++;
  await call({op:'close',tab});tab=undefined;
  console.log(`PASS: ${checks} checks (real CEF browser + HTTP + MCP)`);
}finally{
  if(tab)await call({op:'close',tab}).catch(()=>{});
  fixture.closeAllConnections();await new Promise(resolve=>fixture.close(resolve));
}
function processRoot(){return globalThis.process.cwd().replaceAll('\\','/');}
