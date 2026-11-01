// What a wasm32 QB module needs of its host, for the node harness (headless.mjs):
// the `env` imports, the clock tick, and the page of memory the host keeps for itself. The caller says what time it is and
// what to do on a poll, a key look, output, exit and a tone.
const DENIED = 5;
export const TICK_MS = 54.9254; // 18.2 a second: the timer interrupt that takes PLAY's notes off its queue

export class Exit extends Error {
  constructor(code) { super(`exit ${code}`); this.code = code; }
}

// The C math routines the wasm backend calls where the pin does (`RTLIB`): the double routine, and the float one
// rounded to a float.
const doubles = { sin: Math.sin, cos: Math.cos, atan: Math.atan, exp: Math.exp, log: Math.log, log2: Math.log2,
  exp2: (x) => 2 ** x, pow: Math.pow, fmod: (x, y) => x % y };
const libm = Object.fromEntries(Object.entries(doubles).flatMap(([name, f]) =>
  [[name, f], [`${name}f`, (...args) => Math.fround(f(...args))]]));
// C's lrint: ties to even.
export const lrint = (x) => { const r = Math.round(x); return Math.abs(x % 1) === 0.5 && r % 2 !== 0 ? r - 1 : r; };

/**
 * hooks: clock() the time in ms; poll(now) after each look at the clock or keyboard; keyPoll(ready, now) when the program
 * looks for a key; write(text) output; tone(hertz, now); fail(message) a named failure of the host.
 */
export async function instantiate(bytes, hooks) {
  const host = { exports: null, scratch: 0, now: 0 };
  const mem = () => new Uint8Array(host.exports.memory.buffer);
  host.mem = mem;
  host.cstring = (at) => { const m = mem(); let end = at; while (m[end]) end++; return Buffer_latin1(m.subarray(at, end)); };
  let nextTick = TICK_MS, inTick = false;

  // Runs the ticks that have come due. A host that stalls does not owe the program all it missed.
  host.tick = (now) => {
    if (inTick) return;
    inTick = true;
    for (let fired = 0; now >= nextTick && fired < 4; fired++) { nextTick += TICK_MS; host.exports.host_tick_fire(); }
    if (now >= nextTick) nextTick = now + TICK_MS;
    inTick = false;
  };
  const look = () => { host.now = hooks.clock(); hooks.poll?.(host.now); host.tick(host.now); return host.now; };

  const fs = hooks.fs;
  // `text` as bytes at `ptr`, cut at `max`; its length.
  const text_out = (ptr, max, text) => {
    const m = mem(), n = Math.min(text.length, max);
    for (let at = 0; at < n; at++) m[ptr + at] = text.charCodeAt(at) & 255;
    return n;
  };

  const imports = { env: {
    llrm_host_write: (fd, ptr, n) => hooks.write?.(Buffer_latin1(mem().subarray(ptr, ptr + n))),
    // Files, the command line and the environment: `hooks.fs` (nodefs.mjs), `hooks.args` and `hooks.env`. A host
    // without them answers as one that denies every file (5) and has no arguments.
    llrm_host_fs_open: (name, mode) => fs ? fs.open(host.cstring(name), mode) : -DENIED,
    llrm_host_fs_create: (name) => fs ? fs.create(host.cstring(name)) : -DENIED,
    llrm_host_fs_read: (fd, ptr, n) => fs ? fs.read(fd, mem().subarray(ptr, ptr + n)) : -DENIED,
    llrm_host_fs_write: (fd, ptr, n) => fs ? fs.write(fd, mem().subarray(ptr, ptr + n)) : -DENIED,
    llrm_host_fs_seek: (fd, offset, origin) => fs ? fs.seek(fd, offset, origin) : -DENIED,
    llrm_host_fs_close: (fd) => fs ? fs.close(fd) : -DENIED,
    llrm_host_fs_remove: (name) => fs ? fs.remove(host.cstring(name)) : -DENIED,
    llrm_host_fs_rename: (from, to) => fs ? fs.rename(host.cstring(from), host.cstring(to)) : -DENIED,
    llrm_host_fs_mkdir: (name) => fs ? fs.mkdir(host.cstring(name)) : -DENIED,
    llrm_host_fs_chdir: (name) => fs ? fs.chdir(host.cstring(name)) : -DENIED,
    llrm_host_fs_rmdir: (name) => fs ? fs.rmdir(host.cstring(name)) : -DENIED,
    llrm_host_fs_directory: (ptr, max) => text_out(ptr, max, fs ? fs.directory() : ''),
    llrm_host_fs_free: () => (fs ? fs.freeBytes() : 0),
    llrm_host_fs_find: (pattern, attrs, index, entry) =>
      fs ? fs.find(host.cstring(pattern), attrs, index, mem().subarray(entry, entry + 43)) : -DENIED,
    llrm_host_command_line: (ptr, max) => text_out(ptr, max, hooks.args ?? ''),
    llrm_host_environment: (index, ptr, max) => {
      const one = (hooks.env ?? [])[index];
      return one === undefined ? -1 : text_out(ptr, max, one);
    },
    llrm_host_exit: (code) => { throw new Exit(code); },
    llrm_host_fail: (msg) => { throw new Error(host.cstring(msg)); },
    llrm_host_now: look,
    llrm_host_key_poll: (ready) => hooks.keyPoll?.(ready, host.now),
    llrm_host_poll: () => { look(); },
    llrm_host_tone: (hertz) => hooks.tone?.(hertz, host.now),
    ...libm,
    lrint, lrintf: lrint,
  } };
  const { instance } = await WebAssembly.instantiate(bytes, imports);
  host.exports = instance.exports;
  // The host's own memory: a page after the program's. `__heap_base` is the top of the program's stack, whose outermost
  // frames anything written below it would overwrite.
  host.scratch = host.exports.memory.grow(1) * 65536;
  // The engine's call stack is the program's stack check: when it runs out the program ends as QB's does, with the
  // runtime's "Out of stack space" (the call exits the program).
  host.run = () => {
    try {
      host.exports.__wasm_call_ctors();
      return host.exports.llrm_wasm_start();
    } catch (error) {
      if (!(error instanceof RangeError)) throw error;
      return host.exports.llrm_wasm_stack_overflow();
    }
  };

  // The screen: its size, the palette (8 bits a channel, 256 entries) and the pixels as palette indices; copies.
  host.frame = () => {
    const w = host.exports.host_width(), h = host.exports.host_height();
    host.exports.host_colors(host.scratch);
    const colors = mem().slice(host.scratch, host.scratch + 768);
    const at = host.exports.host_pixels();
    return { w, h, colors, pixels: mem().slice(at, at + w * h) };
  };

  // A typed key, as `headless_type` takes it: a character, or the scan code of an extended key.
  host.type = (code) => {
    if (code >= 256) return host.exports.headless_type_extended(code & 255);
    if (code === 0) return;
    mem()[host.scratch] = code;
    mem()[host.scratch + 1] = 0;
    host.exports.headless_type(host.scratch);
  };
  return host;
}

// Bytes as a latin1 string, in node and in a browser.
function Buffer_latin1(bytes) {
  let text = '';
  for (let at = 0; at < bytes.length; at += 8192) text += String.fromCharCode.apply(null, bytes.subarray(at, at + 8192));
  return text;
}
