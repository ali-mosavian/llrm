// The page's worker: runs a wasm32 QB module on a thread of its own, since the program spins on its timing loops, and
// hands the page what it shows and sounds. Keys come in through a ring the page fills (input.mjs). Runs as a browser
// module worker and, for web/check.mjs, as a node worker thread.
import { Exit, instantiate } from '../host.mjs';
import { NOTIFY, ctrlOf, pop } from './input.mjs';

const FRAME_MS = 33;
const inBrowser = typeof WorkerGlobalScope !== 'undefined';
const port = inBrowser ? self : (await import('node:worker_threads')).parentPort;
const once = (handle) => (inBrowser ? (self.onmessage = (event) => handle(event.data)) : port.once('message', handle));

once(async ({ url, bytes, input }) => {
  const ctrl = ctrlOf(input);
  const output = [];
  let host, lastFrame = -Infinity;
  const t0 = performance.now();
  const send = (final) => {
    const frame = host.frame();
    port.postMessage({ type: 'frame', final, ...frame }, [frame.pixels.buffer, frame.colors.buffer]);
  };
  try {
    bytes ??= await (await fetch(url)).arrayBuffer();
    host = await instantiate(bytes, {
      clock: () => performance.now() - t0,
      poll: (now) => { if (now - lastFrame >= FRAME_MS) { lastFrame = now; send(false); } },
      keyPoll: (ready) => {
        if (ready) return;
        const key = pop(ctrl);
        if (key !== undefined) host.type(key);
        else Atomics.wait(ctrl, NOTIFY, Atomics.load(ctrl, NOTIFY), 1); // idle until a key or a millisecond
      },
      write: (text) => output.push(text),
      tone: (hertz) => port.postMessage({ type: 'tone', hertz }),
    });
    let code = 0;
    try { code = host.run(); } catch (error) { if (!(error instanceof Exit)) throw error; code = error.code; }
    send(true);
    port.postMessage({ type: 'exit', code, text: output.join('') });
  } catch (error) {
    port.postMessage({ type: 'error', message: String(error?.stack ?? error) });
  }
});
