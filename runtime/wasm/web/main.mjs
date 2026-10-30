// The page: a canvas for the frames the worker sends, the keyboard into the ring, and the speaker.
import { keyOf, newRing, ctrlOf, push } from './input.mjs';

const status = document.getElementById('status');
const canvas = document.getElementById('screen');
const context = canvas.getContext('2d');
const wasm = new URLSearchParams(location.search).get('wasm') ?? 'program.wasm';

if (!crossOriginIsolated) {
  status.textContent = 'Needs cross-origin isolation (SharedArrayBuffer): serve it with tools/serve-wasm.py.';
  throw new Error('not cross-origin isolated');
}

// The speaker: a square wave at the tone's pitch, started by the first key or click (browsers want a gesture).
let audio, oscillator, gain, pitch = 0;
function sound(hertz) {
  pitch = hertz;
  if (!oscillator) return;
  gain.gain.value = hertz ? 0.06 : 0;
  if (hertz) oscillator.frequency.value = hertz;
}
function wake() {
  if (audio) return;
  audio = new AudioContext();
  oscillator = audio.createOscillator();
  gain = audio.createGain();
  oscillator.type = 'square';
  oscillator.connect(gain).connect(audio.destination);
  oscillator.start();
  sound(pitch);
}

// The newest frame is drawn on the next animation frame; the ones in between are skipped.
let latest;
function draw() {
  if (latest) {
    const { w, h, pixels, colors } = latest;
    latest = null;
    if (canvas.width !== w || canvas.height !== h) { canvas.width = w; canvas.height = h; }
    const image = context.createImageData(w, h);
    const rgba = new Uint32Array(image.data.buffer);
    const palette = new Uint32Array(256);
    for (let at = 0; at < 256; at++) palette[at] = 0xff000000 | (colors[at * 3 + 2] << 16) | (colors[at * 3 + 1] << 8) | colors[at * 3];
    for (let at = 0; at < w * h; at++) rgba[at] = palette[pixels[at]];
    context.putImageData(image, 0, 0);
  }
  requestAnimationFrame(draw);
}
requestAnimationFrame(draw);

const ring = newRing();
const ctrl = ctrlOf(ring);
const worker = new Worker(new URL('./worker.mjs', import.meta.url), { type: 'module' });
worker.onmessage = ({ data }) => {
  if (data.type === 'frame') latest = data;
  else if (data.type === 'tone') sound(data.hertz);
  else if (data.type === 'exit') { sound(0); status.textContent = `The program ended (${data.code}).`; }
  else if (data.type === 'error') { status.textContent = 'The program stopped.'; console.error(data.message); }
};
worker.postMessage({ url: new URL(wasm, location.href).href, input: ring });

addEventListener('keydown', (event) => {
  wake();
  const key = keyOf(event);
  if (key === null) return;
  event.preventDefault();
  push(ctrl, key);
});
addEventListener('pointerdown', wake);
status.textContent = 'Click the page, then type.';
