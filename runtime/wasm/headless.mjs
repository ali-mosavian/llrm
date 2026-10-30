// Runs a QB program built by build.sh on the headless device in node: `node headless.mjs PROGRAM.wasm [--keys TEXT@MS ...]
// [--dump-at MS=NAME ...] [--out DIR] [--virtual]`. The clock is real, as on DOS: a program that times its own loops
// (Gorillas' CalcDelay) ends. With `--virtual` it moves by `--step-us` on each poll of the program's clock or keyboard
// instead, so a run is the same every time: for a program whose timing loops were taken out. Keys and frame dumps are
// scheduled on the clock, in milliseconds since the start. Frames are written as PPM files.
import { readFileSync, writeFileSync, mkdirSync } from 'node:fs';

const args = process.argv.slice(2);
const wasmPath = args.shift();
const keys = [], dumps = [];
let out = '.', stepUs = 100, limitMs = 600000, wallMs = 0, virtual = false;
while (args.length) {
  const flag = args.shift();
  if (flag === '--keys') { const [text, at] = args.shift().split('@'); keys.push({ text: JSON.parse(`"${text}"`), at: Number(at) }); }
  else if (flag === '--dump-at') { const [at, name] = args.shift().split('='); dumps.push({ at: Number(at), name }); }
  else if (flag === '--out') out = args.shift();
  else if (flag === '--step-us') stepUs = Number(args.shift());
  else if (flag === '--limit-ms') limitMs = Number(args.shift());
  else if (flag === '--virtual') virtual = true;
  else if (flag === '--wall-ms') wallMs = Number(args.shift());
  else throw new Error(`unknown argument ${flag}`);
}
mkdirSync(out, { recursive: true });

let exports;
let now = 0; // virtual milliseconds
const started = Date.now();
const calls = {};
const text = [];
const mem = () => new Uint8Array(exports.memory.buffer);
const cstring = (at) => { const m = mem(); let end = at; while (m[end]) end++; return Buffer.from(m.subarray(at, end)).toString('latin1'); };
const lrint = (x) => { const r = Math.round(x); return Math.abs(x % 1) === 0.5 && r % 2 !== 0 ? r - 1 : r; };

class Exit extends Error { constructor(code) { super(`exit ${code}`); this.code = code; } }

function dump(name) {
  const w = exports.host_width(), h = exports.host_height();
  const rgb = new Uint8Array(768);
  const scratch = exports.__heap_base.value - 1024; // below the heap, inside the stack's reach: not touched while paused
  exports.host_colors(scratch);
  rgb.set(mem().subarray(scratch, scratch + 768));
  const pixels = mem().subarray(exports.host_pixels(), exports.host_pixels() + w * h);
  const body = Buffer.alloc(w * h * 3);
  for (let i = 0; i < w * h; i++) { body[i * 3] = rgb[pixels[i] * 3]; body[i * 3 + 1] = rgb[pixels[i] * 3 + 1]; body[i * 3 + 2] = rgb[pixels[i] * 3 + 2]; }
  writeFileSync(`${out}/${name}.ppm`, Buffer.concat([Buffer.from(`P6\n${w} ${h}\n255\n`), body]));
}

function due() {
  for (const k of keys) if (!k.done && now >= k.at) { k.done = true; const p = exports.__heap_base.value - 64; mem().set(Buffer.from(k.text + '\0', 'latin1'), p); exports.headless_type(p); }
  for (const d of dumps) if (!d.done && now >= d.at) { d.done = true; dump(d.name); }
  if (now > limitMs || (wallMs && Date.now() - started > wallMs)) throw new Exit(124);
}

function tick() {
  now = virtual ? now + stepUs / 1000 : Date.now() - started;
  due();
}

const imports = { env: {
  llrm_host_write: (fd, ptr, n) => { text.push(Buffer.from(mem().subarray(ptr, ptr + n)).toString('latin1')); },
  llrm_host_exit: (code) => { throw new Exit(code); },
  llrm_host_fail: (msg) => { throw new Error(cstring(msg)); },
  llrm_host_now: () => { calls.now = (calls.now || 0) + 1; tick(); return now; },
  llrm_host_poll: () => { calls.poll = (calls.poll || 0) + 1; tick(); },
  sin: Math.sin, cos: Math.cos, atan: Math.atan, lrint, lrintf: lrint,
} };

const { instance } = await WebAssembly.instantiate(readFileSync(wasmPath), imports);
exports = instance.exports;
exports.__wasm_call_ctors();
let code = 0;
try { code = exports.llrm_wasm_start(); } catch (error) { if (error instanceof Exit) code = error.code; else throw error; }
dump('final');
{
  // The text screen, the rows that are not blank, as runtime/qb/dev/headless/main.c prints it.
  const row = exports.__heap_base.value - 256;
  const lines = [];
  for (let r = 0; r < 25; r++) {
    exports.host_text_row(r, row);
    const text = cstring(row).replace(/ +$/, '');
    if (text) lines.push(text);
  }
  if (lines.length) text.push(lines.join('\n') + '\n');
}
if (process.env.HEADLESS_TRACE) console.error(JSON.stringify({ now, calls }));
process.stdout.write(text.join(''));
process.exitCode = code;
