// The host's endings against a module's `catch_all`: a function `try`s a call to an import, which ends the program the way
// `llrm_host_exit` does. Prints "escaped" when the ending got through the `catch_all`, "swallowed" when it did not.
// usage: node probe_end.mjs [throw]   (`throw` ends by throwing: the old way)
import { Exit, ender } from './host.mjs';
// (module (import "env" "f" (func)) (func (export "g") try call 0 catch_all end))
const module = Uint8Array.of(0, 97, 115, 109, 1, 0, 0, 0, 1, 4, 1, 96, 0, 0, 2, 9, 1, 3, 101, 110, 118, 1, 102, 0, 0, 3, 2, 1, 0,
  7, 5, 1, 1, 103, 0, 1, 10, 10, 1, 8, 0, 6, 64, 16, 0, 25, 11, 11);
const ending = ender();
const end = process.argv[2] === "throw" ? (error) => { throw error; } : ending.end;
const { instance } = await WebAssembly.instantiate(module, { env: { f: () => end(new Exit(3)) } });
let how = 'swallowed';
try { instance.exports.g(); } catch { how = ending.ended ? 'escaped' : 'escaped-unrecorded'; }
console.log(how);
