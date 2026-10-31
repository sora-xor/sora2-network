import fs from 'node:fs';
const dir=process.argv[2] ?? 'outputs';
let checked=0;const transcript=[];
for(const file of fs.readdirSync(dir).filter(x=>x.endsWith('.old.wasm')).sort()) {
 const name=file.replace(/-\d+\.old\.wasm$/,'');
 function execute(path){
  let instance;
  try{instance=new WebAssembly.Instance(new WebAssembly.Module(fs.readFileSync(path)),{env:{host:x=>x+1}})}catch(e){return ['instantiate',e.constructor.name]}
  const fn=instance.exports.run ?? instance.exports.h;
  if(!fn)return ['no callable export'];
  const args=name==='locals'?[[4,2.5],[4,2.5]]:name==='float-comparison'?[[2.5,3.0],[2.5,3.0]]:[[0],[1],[2],[9],[0]];
  return args.map(args=>{try{return ['return',String(fn(...args))]}catch(e){return ['trap',e.constructor.name]}});
 }
 const old=execute(`${dir}/${file}`);const modern=execute(`${dir}/${file.replace('.old.','.new.')}`);
 if(JSON.stringify(old)!==JSON.stringify(modern))throw Error(`${file}: ${JSON.stringify({old,modern})}`);
 transcript.push([file,modern]);checked++;
}
fs.writeFileSync('execution-transcript.json',JSON.stringify(transcript));
console.log(`PASS ${checked} old/new WebAssembly instances and their repeated-call/trap outcomes`);
