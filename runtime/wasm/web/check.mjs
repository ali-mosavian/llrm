// Runs a module through the page's worker as the page does, in a node worker thread: `node check.mjs PROGRAM.wasm
// [--keys TEXT] [--after MS]`. The keys go into the ring `--after` ms (default 500) after the start, as a person types. Prints
// one JSON line: frames received, the last frame's size and the sum of its pixels, the tones, the exit code and the text.
import { readFileSync } from 'node:fs';
import { Worker } from 'node:worker_threads';
import { ctrlOf, newRing, push } from './input.mjs';

const args = process.argv.slice(2);
const path = args.shift();
let keys = '', after = 500;
while (args.length) {
  const flag = args.shift();
  if (flag === '--keys') keys = args.shift().replaceAll('\\r', '\r');
  else if (flag === '--after') after = Number(args.shift());
  else throw new Error(`unknown argument ${flag}`);
}

const ring = newRing();
const ctrl = ctrlOf(ring);
const worker = new Worker(new URL('./worker.mjs', import.meta.url));
const seen = { frames: 0, tones: [], size: null, lit: 0 };
const timer = setTimeout(() => { console.error('check: no exit in 60 s'); process.exit(124); }, 60000);
setTimeout(() => { for (const c of keys) push(ctrl, c.charCodeAt(0)); }, after);
worker.on('message', (m) => {
  if (m.type === 'frame') {
    seen.frames++;
    if (m.final) { seen.size = [m.w, m.h]; seen.lit = m.pixels.reduce((n, p) => n + (p !== 0), 0); }
  } else if (m.type === 'tone') seen.tones.push(m.hertz);
  else if (m.type === 'exit') {
    clearTimeout(timer);
    console.log(JSON.stringify({ ...seen, code: m.code, text: m.text }));
    process.exit(0);
  } else if (m.type === 'error') { console.error(m.message); process.exit(1); }
});
worker.postMessage({ bytes: readFileSync(path), input: ring });
