import {spawn, spawnSync} from 'node:child_process';
import {createInterface} from 'node:readline';
import {resolve} from 'node:path';
import {performance} from 'node:perf_hooks';
const executable=resolve('target/bundle/aibro-agent.exe');
const root=resolve('.aibro');
const server=spawn(executable,['--data-dir',root],{stdio:['pipe','pipe','pipe']});
const pending=new Map();let sequence=0;
createInterface({input:server.stdout}).on('line',line=>{
  const response=JSON.parse(line);const handler=pending.get(response.id);
  if(handler){pending.delete(response.id);handler(response)}
});
server.stderr.on('data',()=>{});
function rpc(method,params){
  return new Promise((resolve,reject)=>{
    const id=++sequence;
    const timer=setTimeout(()=>{pending.delete(id);reject(Error('MCP timed out'))},90000);
    pending.set(id,value=>{clearTimeout(timer);resolve(value)});
    server.stdin.write(JSON.stringify({jsonrpc:'2.0',id,method,params})+'\n');
  });
}
function cli(command){
  const result=spawnSync(executable,['--data-dir',root,'--call',JSON.stringify(command)],{encoding:'utf8',maxBuffer:32*1024*1024,timeout:90000});
  if(result.status!==0)throw Error('CLI command failed');
  const response=JSON.parse(result.stdout);if(!response.ok)throw Error('Browser command failed');return response.data;
}
async function mcp(command){
  const response=await rpc('tools/call',{name:'browser',arguments:command});
  if(response.error||response.result?.isError)throw Error('MCP browser command failed');
  return JSON.parse(response.result.content[0].text);
}
function stats(values){
  const sorted=[...values].sort((a,b)=>a-b);
  return {median_ms:+sorted[Math.floor(sorted.length/2)].toFixed(2),p95_ms:+sorted[Math.min(sorted.length-1,Math.ceil(sorted.length*.95)-1)].toFixed(2)};
}
try {
  await rpc('initialize',{protocolVersion:'2025-11-25',capabilities:{},clientInfo:{name:'aibro-benchmark',version:'1'}});
  server.stdin.write(JSON.stringify({jsonrpc:'2.0',method:'notifications/initialized'})+'\n');
  const tabs=cli({op:'tabs'});if(!tabs.length)throw Error('No open tab');
  const results=[];
  for(const [command,count] of [[{op:'tabs'},30],[{op:'snapshot',tab:tabs[0].tab},10]]) {
    cli(command);await mcp(command);
    const times={cli:[],mcp:[]};
    for(let i=0;i<count;i++) {
      for(const mode of i%2?['mcp','cli']:['cli','mcp']) {
        const start=performance.now();if(mode==='cli')cli(command);else await mcp(command);
        times[mode].push(performance.now()-start);
      }
    }
    results.push({op:command.op,samples_per_mode:count,cli_per_process:stats(times.cli),mcp_persistent:stats(times.mcp)});
  }
  console.log(JSON.stringify({note:'Local timings; excludes model reasoning and MCP startup. Both modes call the same browser HTTP API. No page contents are recorded.',results},null,2));
}finally{server.stdin.end();server.kill();}
