// node run.mjs PROG.wasm : imports of env other than report are missing runtime; prints the report() values, one per line.
import fs from 'node:fs';
const mod = new WebAssembly.Module(fs.readFileSync(process.argv[2]));
const missing = WebAssembly.Module.imports(mod).filter(i => i.kind === 'function' && i.name !== 'report').map(i => i.name);
if (missing.length) { console.log('MISSING ' + missing.join(' ')); process.exit(3); }
const out = [];
const inst = new WebAssembly.Instance(mod, { env: { report: v => { out.push(String(v | 0)); } } });
const e = inst.exports;
if (e.__wasm_call_ctors) e.__wasm_call_ctors();
const f = e.main || e.__main_void;
let rc;
try { rc = f.length >= 2 ? f(0, 0) : f(); } catch (x) { console.log(out.join('\n')); console.log('TRAP ' + x.message); process.exit(4); }
process.stdout.write(out.map(s => s + '\n').join(''));
