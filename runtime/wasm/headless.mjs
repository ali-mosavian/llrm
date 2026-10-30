// Runs a QB program built by build.sh on the headless device in node: `node headless.mjs PROGRAM.wasm [--keys TEXT@MS ...]
// [--dump-at MS=NAME ...] [--out DIR] [--virtual]`. The clock is real, as on DOS: a program that times its own loops
// (Gorillas' CalcDelay) ends. With `--virtual` it moves by `--step-us` on each poll of the program's clock or keyboard
// instead, so a run is the same every time: for a program whose timing loops were taken out. Keys and frame dumps are
// scheduled on the clock, in milliseconds since the start. Frames are written as PPM files.
import { readFileSync, writeFileSync, mkdirSync } from 'node:fs';

const args = process.argv.slice(2);
const wasmPath = args.shift();
const keys = [], dumps = [];
let script = '', gap = 700, onWait = false, settle = 300;
let out = '.', stepUs = 100, limitMs = 600000, wallMs = 0, virtual = false;
while (args.length) {
  const flag = args.shift();
  if (flag === '--keys') { const [text, at] = args.shift().split('@'); keys.push({ text: JSON.parse(`"${text}"`), at: Number(at) }); }
  else if (flag === '--script') script = args.shift();
  else if (flag === '--gap') gap = Number(args.shift());
  else if (flag === '--dump-at') { const [at, name] = args.shift().split('='); dumps.push({ at: Number(at), name }); }
  else if (flag === '--out') out = args.shift();
  else if (flag === '--step-us') stepUs = Number(args.shift());
  else if (flag === '--limit-ms') limitMs = Number(args.shift());
  else if (flag === '--virtual') virtual = true;
  else if (flag === '--dump-after') { const [spec, name] = args.shift().split('='); const [n, dwell] = spec.split('+'); dumps.push({ afterKeys: Number(n), dwell: Number(dwell || 0), name }); }
  else if (flag === '--on-wait') onWait = true;
  else if (flag === '--settle-ms') settle = Number(args.shift());
  else if (flag === '--wall-ms') wallMs = Number(args.shift());
  else throw new Error(`unknown argument ${flag}`);
}
// QB_HEADLESS_KEYS as the native driver reads it: a key every `gap` ms from the start (\r, \n, \t and \\ are escapes).
for (let at = 0, n = 1; at < script.length; at++, n++) {
  let c = script[at];
  if (c === '\\' && at + 1 < script.length) { at++; c = 'rn'.includes(script[at]) ? '\r' : script[at] === 't' ? '\t' : script[at]; }
  keys.push({ text: c, at: onWait ? 0 : n * gap });
}
mkdirSync(out, { recursive: true });

let exports, scratch;
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
  exports.host_colors(scratch);
  rgb.set(mem().subarray(scratch, scratch + 768));
  const pixels = mem().subarray(exports.host_pixels(), exports.host_pixels() + w * h);
  const body = Buffer.alloc(w * h * 3);
  for (let i = 0; i < w * h; i++) { body[i * 3] = rgb[pixels[i] * 3]; body[i * 3 + 1] = rgb[pixels[i] * 3 + 1]; body[i * 3 + 2] = rgb[pixels[i] * 3 + 2]; }
  writeFileSync(`${out}/${name}.ppm`, Buffer.concat([Buffer.from(`P6\n${w} ${h}\n255\n`), body]));
}

// With --on-wait a key is typed when the program looks for one, none is waiting and `settle` ms have passed since the last:
// a run does not depend on how long anything took (a key typed at the start would be lost to a program that clears the
// keyboard when it starts).
let lastKey = -Infinity;
const keyTimes = [];
function typeKey(k) {
  k.done = true;
  lastKey = now;
  keyTimes.push(now);
  const p = scratch;
  mem().set(Buffer.from(k.text + '\0', 'latin1'), p);
  exports.headless_type(p);
}

function keyPoll(ready) {
  if (!onWait || ready || now < lastKey + settle) return;
  // a frame wanted after this many keys is taken before the next is typed
  if (dumps.some((d) => !d.done && d.afterKeys === keyTimes.length)) return;
  const next = keys.find((k) => !k.done);
  if (next) typeKey(next);
}

function due() {
  if (!onWait) for (const k of keys) if (!k.done && now >= k.at) typeKey(k);
  for (const d of dumps) {
    if (d.done) continue;
    const reached = d.afterKeys === undefined ? now >= d.at : keyTimes.length >= d.afterKeys && now >= keyTimes[d.afterKeys - 1] + d.dwell;
    if (reached) { d.done = true; dump(d.name); }
  }
  if (now > limitMs || (wallMs && Date.now() - started > wallMs)) throw new Exit(124);
}

// The clock tick (18.2 a second) calls music_tick as the timer interrupt does: a PLAY waits for it to take notes off its queue.
const TICK_MS = 54.9254;
let nextTick = TICK_MS, inTick = false;

function tick() {
  now = virtual ? now + stepUs / 1000 : Date.now() - started;
  due();
  if (inTick) return;
  inTick = true;
  for (let fired = 0; now >= nextTick && fired < 4; fired++) { nextTick += TICK_MS; exports.host_tick_fire(); }
  if (now >= nextTick) nextTick = now + TICK_MS;
  inTick = false;
}

const imports = { env: {
  llrm_host_write: (fd, ptr, n) => { text.push(Buffer.from(mem().subarray(ptr, ptr + n)).toString('latin1')); },
  llrm_host_exit: (code) => { throw new Exit(code); },
  llrm_host_fail: (msg) => { throw new Error(cstring(msg)); },
  llrm_host_now: () => { calls.now = (calls.now || 0) + 1; tick(); return now; },
  llrm_host_key_poll: (ready) => keyPoll(ready),
  llrm_host_poll: () => { calls.poll = (calls.poll || 0) + 1; tick(); },
  sin: Math.sin, cos: Math.cos, atan: Math.atan, lrint, lrintf: lrint,
} };

const { instance } = await WebAssembly.instantiate(readFileSync(wasmPath), imports);
exports = instance.exports;
// The harness's own memory: a page after the program's. `__heap_base` is the top of the program's stack, whose
// outermost frames anything written below it would overwrite.
scratch = exports.memory.grow(1) * 65536;
exports.__wasm_call_ctors();
let code = 0;
try { code = exports.llrm_wasm_start(); } catch (error) { if (error instanceof Exit) code = error.code; else throw error; }
dump('final');
{
  // The text screen, the rows that are not blank, as runtime/qb/dev/headless/main.c prints it.
  const row = scratch;
  const lines = [];
  for (let r = 0; r < 25; r++) {
    exports.host_text_row(r, row);
    const text = cstring(row).replace(/ +$/, '');
    if (text) lines.push(text);
  }
  if (lines.length) text.push(lines.join('\n') + '\n');
}
if (process.env.HEADLESS_TRACE) console.error(JSON.stringify({ now, calls, keyTimes }));
process.stdout.write(text.join(''));
process.exitCode = code;
