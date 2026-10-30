// The keys from the page to the worker: a ring of 64 integers in a SharedArrayBuffer. A key is a character code, or 256
// plus the scan code of an extended key (the arrows). The page writes, the worker reads; neither blocks the other.
export const SLOTS = 64;
export const NOTIFY = 0; // the count written
const READ = 1; // the count read; the keys follow

export const newRing = () => new SharedArrayBuffer(4 * (2 + SLOTS));
export const ctrlOf = (buffer) => new Int32Array(buffer);
export const RING = 2;

/** Queues `key`; a full ring drops it. */
export function push(ctrl, key) {
  const written = Atomics.load(ctrl, NOTIFY);
  if (written - Atomics.load(ctrl, READ) >= SLOTS) return false;
  ctrl[RING + (written % SLOTS)] = key;
  Atomics.store(ctrl, NOTIFY, written + 1);
  Atomics.notify(ctrl, NOTIFY);
  return true;
}

/** The oldest queued key, or undefined. */
export function pop(ctrl) {
  const read = Atomics.load(ctrl, READ);
  if (read === Atomics.load(ctrl, NOTIFY)) return undefined;
  const key = ctrl[RING + (read % SLOTS)];
  Atomics.store(ctrl, READ, read + 1);
  return key;
}

const EXTENDED = { ArrowUp: 72, ArrowLeft: 75, ArrowRight: 77, ArrowDown: 80, Home: 71, End: 79, PageUp: 73, PageDown: 81, Insert: 82, Delete: 83 };
const CONTROL = { Enter: 13, Backspace: 8, Tab: 9, Escape: 27 };

/** The key a keyboard event types, or null for one the program does not see. */
export function keyOf(event) {
  if (event.ctrlKey || event.metaKey || event.altKey) return null;
  if (event.key in CONTROL) return CONTROL[event.key];
  if (event.key in EXTENDED) return 256 + EXTENDED[event.key];
  if (event.key.length === 1) {
    const code = event.key.charCodeAt(0);
    return code < 128 ? code : null;
  }
  return null;
}
