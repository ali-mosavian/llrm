// Runs a QB program built by build.sh on the headless device in node: `node headless.mjs PROGRAM.wasm [--keys TEXT@MS ...]
// [--dump-at MS=NAME ...] [--arg WORD ...] [--env NAME=VALUE ...] [--out DIR] [--virtual]`. The clock is real, as on DOS: a program that times its own loops
// (Gorillas' CalcDelay) ends. With `--virtual` it moves by `--step-us` on each poll of the program's clock or keyboard
// instead, so a run is the same every time: for a program whose timing loops were taken out. Keys and frame dumps are
// scheduled on the clock, in milliseconds since the start. Frames are written as PPM files.
import { readFileSync, writeFileSync, mkdirSync } from 'node:fs';
import { Exit, instantiate } from './host.mjs';
import { nodeFiles } from './nodefs.mjs';

const args = process.argv.slice(2);
const wasmPath = args.shift();
const keys = [], dumps = [];
let script = '', gap = 700, onWait = false, settle = 300;
const programArgs = [], environment = [];
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
  else if (flag === '--arg') programArgs.push(args.shift());
  else if (flag === '--env') environment.push(args.shift());
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

let now = 0; // milliseconds: virtual with --virtual
const started = Date.now();
const calls = {};
const text = [];
const tones = [];
let host;

function dump(name) {
  const { w, h, colors: rgb, pixels } = host.frame();
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
  for (const c of k.text) host.type(c.charCodeAt(0));
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

host = await instantiate(readFileSync(wasmPath), {
  clock: () => (now = virtual ? now + stepUs / 1000 : Date.now() - started),
  poll: () => { calls.poll = (calls.poll || 0) + 1; due(); },
  keyPoll,
  fs: nodeFiles(),
  args: programArgs.join(' '),
  env: environment,
  write: (chunk) => text.push(chunk),
  tone: (hertz, at) => tones.push({ at, hertz }),
});
let code = 0;
try { code = host.run(); } catch (error) { if (error instanceof Exit) code = error.code; else throw error; }
dump('final');
if (tones.length) writeFileSync(`${out}/tones.json`, JSON.stringify(tones));
{
  // The text screen, the rows that are not blank, as runtime/qb/dev/headless/main.c prints it.
  const lines = [];
  for (let r = 0; r < 25; r++) {
    host.exports.host_text_row(r, host.scratch);
    const row = host.cstring(host.scratch).replace(/ +$/, '');
    if (row) lines.push(row);
  }
  if (lines.length) text.push(lines.join('\n') + '\n');
}
if (process.env.HEADLESS_TRACE) console.error(JSON.stringify({ now, calls, keyTimes }));
process.stdout.write(text.join(''));
process.exitCode = code;
