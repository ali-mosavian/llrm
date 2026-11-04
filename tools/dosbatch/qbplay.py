"""Plays a DOS program under DOSBox-X through its debug socket, in lockstep: the guest runs only between commands, for as long as the
program takes to ask for a key or for a stated number of emulated milliseconds, so keys typed, looks and screenshots are at emulated
instants that do not depend on the host. The same script drives BCOM45's build and llrm's, so their screens can be compared at the
same points."""

from __future__ import annotations

import contextlib
import fcntl
import json
import os
import queue
import socket
import subprocess
import tempfile
import threading
import time
from pathlib import Path

import dosbatch

CONF = dosbatch.CONF


SLICE_MS = 100      # emulated ms a lockstep wait runs at a time
BOOT_SLICES = 100   # slices the prompt may take (10 emulated s)
FRAME_MS = 30       # emulated ms that always hold a frame (the screen is redrawn at 70 Hz)
BACKSTOP = 3600  # wall seconds: for an emulator that waits and burns no CPU


@contextlib.contextmanager
def port_claim():
    """One session at a time picks a free port and gets its emulator to hold it: the port is free between the pick and the emulator's
    bind, and a second session picking in that gap got the same one (sessions of other processes run at once in the gate)."""
    with open(Path(tempfile.gettempdir()) / "llrm-qbplay-port.lock", "w") as handle:
        fcntl.flock(handle, fcntl.LOCK_EX)
        yield


class Session:
    def limit(self, seconds: float):
        """A function that says whether the emulator has used `seconds` of CPU time since now. The wall clock is the host's
        load: waiting 30 s for a DOS shell to come up failed whenever the host was busy. The wall only backs this up."""
        cpu, wall = dosbatch._cpu_seconds(self.process.pid), time.monotonic()
        return lambda: dosbatch._cpu_seconds(self.process.pid) - cpu > seconds or time.monotonic() - wall > BACKSTOP

    def __init__(self, work: Path, conf: str = CONF):
        self.work = work
        self.now_us = 0
        with port_claim():
            probe = socket.socket()
            probe.bind(("127.0.0.1", 0))
            port = probe.getsockname()[1]
            probe.close()
            (work / "play.conf").write_text(conf + f"[autoexec]\nmount c {work}\nc:\n")
            # TZ is pinned: the fork turns "virtual time start" into the guest's time of day in the host's zone, and a program that seeds RND
            # from TIMER (NIBBLES) then plays another game on a UTC runner than on a CET dev box.
            env = {**os.environ, "TZ": "UTC", "SDL_VIDEODRIVER": "dummy", "DOSBOX_DEBUG_PORT": str(port)}
            self.process = subprocess.Popen(
                [str(dosbatch.DOSBOX), "-nolog", "-conf", str(work / "play.conf")],
                env=env,
                stdout=subprocess.DEVNULL,
                stderr=subprocess.DEVNULL,
            )
            late = self.limit(30)
            while True:
                try:
                    self.sock = socket.create_connection(("127.0.0.1", port))
                    break
                except OSError:
                    if late():
                        raise
                    time.sleep(0.2)
        self.pending = 0
        self.replies: queue.Queue = queue.Queue()
        self.events: list[dict] = []
        self.arrived = threading.Condition()
        threading.Thread(target=self.read, args=(self.sock.makefile("r"),), daemon=True).start()
        self.require_lockstep()
        self.await_frozen()
        # Boots in emulated slices to the prompt (autoexec's own keyboard reads are not the prompt's), then parks at its read.
        for _ in range(BOOT_SLICES):
            if "C:\\>" in self.screen():
                break
            self.resume(SLICE_MS)
        else:
            raise TimeoutError("the DOS prompt never came up")
        self.send({"cmd": "key_wait_arm", "stop": True})
        self.resume(SLICE_MS * BOOT_SLICES)

    def read(self, lines) -> None:
        """Replies go to the queue in the order their commands were sent; events are kept."""
        for line in lines:
            message = json.loads(line)
            if "status" in message:
                self.replies.put(message)
                continue
            with self.arrived:
                self.events.append(message)
                self.arrived.notify_all()
            if message.get("event") == "stopped":
                self.sock.sendall(b'{"cmd":"continue"}\n')

    def send(self, command: dict, wait: bool = True) -> dict | None:
        self.sock.sendall((json.dumps(command) + "\n").encode())
        if not wait:
            self.pending += 1
            return None
        while self.pending:
            self.reply()
            self.pending -= 1
        return self.reply()

    def reply(self) -> dict:
        """The next reply: the emulator gets 120 s of CPU time to give it, however long a busy host takes."""
        late = self.limit(120)
        while True:
            try:
                return self.replies.get(timeout=1)
            except queue.Empty:
                if late():
                    raise

    def require_lockstep(self) -> None:
        """Refuses an emulator that cannot park the guest (a fork before `key_wait_arm` took `stop` and `continue` took `ms`: `resume` would
        wait for ever) or that is not on virtual time."""
        status = self.send({"cmd": "virtual_time_status"})
        if not (status.get("lockstep") and status.get("on")):
            self.close()
            raise RuntimeError(f"{dosbatch.DOSBOX} cannot run in lockstep (status {status}): it needs the fork's `lockstep` and the conf's `virtual time`")

    def await_frozen(self) -> None:
        """Until the emulator is parked at its first instruction: a `continue` sent before it had frozen was lost, and a frozen emulator
        burns no CPU for a wait to count (gate 65: 40 minutes, #1570). The guest has not run, so how long this takes is no matter."""
        deadline = time.monotonic() + BACKSTOP
        while not self.send({"cmd": "status"}).get("socketFrozen"):
            if time.monotonic() > deadline or self.process.poll() is not None:
                raise TimeoutError("the emulator never parked at its first instruction")
            time.sleep(0.01)

    def resume(self, ms: int | None = None) -> dict:
        """Lockstep: runs the guest until it parks, at the next keyboard read that finds nothing (`key_wait`, once armed with
        `stop`) or after `ms` emulated milliseconds (`run_limit`), and returns that event. The guest is parked where the
        emulated clock says, whatever the host does, so what the client then reads, types or photographs is the same every run.
        `now_us` is the emulated clock at the park. The wait on the host only ends when the event comes or the emulator dies."""
        since = len(self.events)
        self.send({"cmd": "continue", **({"ms": ms} if ms else {})})
        with self.arrived:
            while True:
                for event in self.events[since:]:
                    if event.get("event") in ("key_wait", "run_limit"):
                        self.now_us = event["emulated_us"]
                        return event
                if self.process.poll() is not None:
                    raise RuntimeError("the emulator exited before the guest parked")
                self.arrived.wait(1)

    def run_until(self, done, ms: int, what: str) -> None:
        """Lockstep: runs the guest in SLICE_MS slices until `done()`, asked where it is parked, holds; raises after `ms` emulated ms."""
        waited = 0
        while not done():
            if waited >= ms:
                raise TimeoutError(f"{what} never came in {ms} emulated ms")
            self.resume(SLICE_MS)
            waited += SLICE_MS

    def launch(self, ms: int, program: str = "P.EXE") -> None:
        """Lockstep: types `program` at the DOS prompt, which starts it at an emulated instant that is the same every run."""
        self.keys_in_lockstep([*(c for c in program), "enter"], ms)

    def video_mode(self) -> int | None:
        return self.send({"cmd": "text_screen"}).get("video_mode")

    def run_to_key(self, ms: int) -> None:
        """Lockstep: runs the guest to its next keyboard read that finds nothing; raises if it takes more than `ms` emulated ms."""
        if self.resume(ms)["event"] != "key_wait":
            raise TimeoutError(f"the program read no key in {ms} emulated ms")

    def keys_in_lockstep(self, keys: list[str], ms: int, waiting: int = 1) -> None:
        """Each key goes in where the guest is parked and the guest runs to the read that wants the next, so the program takes every key
        at the emulated instant it asks for it. A program that flushes the keyboard before it asks (GORILLA's prompts) reads the key at the
        flush and throws it away: `waiting` reads in a row that find nothing, with the picture the same, are what a program that asks looks like."""
        for key in keys:
            self.send({"cmd": "key", "key": key, "stop": True})
            self.run_to_key(ms)
            picture, same, until = (self.screen_hash() if waiting > 1 else None), 1, self.now_us + ms * 1000
            while same < waiting:
                if self.now_us > until:
                    raise TimeoutError(f"the program did not wait for a key in {ms} emulated ms")
                self.send({"cmd": "key_wait_arm", "stop": True})
                self.run_to_key(ms)
                look = self.screen_hash()
                same, picture = (same + 1 if look == picture else 1), look

    def keys_timed(self, keys: list[str], gap_ms: int) -> None:
        """Each key goes in where the guest is parked and the guest runs `gap_ms` emulated ms, whether or not it has asked for a key."""
        for key in keys:
            self.send({"cmd": "key", "key": key})
            self.resume(gap_ms)

    def snap(self, path: Path) -> None:
        """Lockstep: a PNG of the screen as it is where the guest is parked: the fork draws it on the next frame, so the guest runs
        on for the frames it takes (a few emulated ms, the same every run)."""
        path.unlink(missing_ok=True)
        since = len(self.events)
        if self.send({"cmd": "screenshot", "path": str(path), "async": True}).get("status") != "ok":
            raise RuntimeError("screenshot refused")
        for _ in range(10):
            self.resume(FRAME_MS)
            for event in self.events[since:]:
                if event.get("event") == "screenshot":
                    if event.get("result") != "ok" or not path.exists():
                        raise RuntimeError(f"screenshot: {event}")
                    return
        raise RuntimeError("no frame was drawn")

    def screen_hash(self, length: int = 0x28000) -> str:
        """A hash of the adapter's video memory, every plane: the same two hashes are the same picture."""
        return str(self.send({"cmd": "mem_hash", "addr": 0, "len": length, "vram": True}).get("hash"))

    def screen(self) -> str:
        reply = self.send({"cmd": "text_screen"})
        text = reply.get("text") or ""
        return "\n".join(text) if isinstance(text, list) else text

    def close(self) -> None:
        self.process.kill()
        self.process.wait()


def png_pixels(path: Path) -> tuple[int, int, bytes]:
    """Width, height and the RGB bytes of a PNG DOSBox wrote (8 bits, no interlace)."""
    import struct
    import zlib

    data = path.read_bytes()
    at, chunks = 8, []
    while at < len(data):
        size, kind = struct.unpack(">I4s", data[at : at + 8])
        chunks.append((kind, data[at + 8 : at + 8 + size]))
        at += 12 + size
    width, height, depth, color, _, _, interlace = struct.unpack(">IIBBBBB", chunks[0][1])
    assert depth == 8 and color in (2, 6) and interlace == 0, (depth, color, interlace)
    step = 3 if color == 2 else 4
    raw = zlib.decompress(b"".join(body for kind, body in chunks if kind == b"IDAT"))
    stride, rows, previous = width * step, [], bytearray(width * step)
    for y in range(height):
        base = y * (stride + 1)
        method, line = raw[base], bytearray(raw[base + 1 : base + 1 + stride])
        for x in range(stride):
            left = line[x - step] if x >= step else 0
            up = previous[x]
            corner = previous[x - step] if x >= step else 0
            if method == 1:
                line[x] = (line[x] + left) & 255
            elif method == 2:
                line[x] = (line[x] + up) & 255
            elif method == 3:
                line[x] = (line[x] + (left + up) // 2) & 255
            elif method == 4:
                p = left + up - corner
                pa, pb, pc = abs(p - left), abs(p - up), abs(p - corner)
                line[x] = (line[x] + (left if pa <= pb and pa <= pc else up if pb <= pc else corner)) & 255
        rows.append(bytes(line))
        previous = line
    rgb = b"".join(b"".join(row[i : i + 3] for i in range(0, stride, step)) for row in rows)
    return width, height, rgb


def differing_pixels(left: Path, right: Path) -> int:
    """How many pixels of two screenshots differ (every one when their sizes do)."""
    a, b = png_pixels(left), png_pixels(right)
    if a[:2] != b[:2]:
        return a[0] * a[1]
    return sum(1 for i in range(0, len(a[2]), 3) if a[2][i : i + 3] != b[2][i : i + 3])


def reference(sources: dict[str, Path], work: Path) -> dict[str, Path]:
    """Each program as BCOM45 builds it: llrm-qb's object on the Microsoft runtime, linked by Microsoft's LINK.  The
    oracle a test compares against."""
    import shutil

    import qbruntime

    objs = {}
    for name, source in sources.items():
        obj = work / f"{name}.qb45.obj"
        error = qbruntime.compile_basic(source, obj, "qb45")
        assert error is None, error
        objs[name] = (obj, qbruntime.linked_pairs(source, work))
    jobs = [
        dosbatch.Job(f"J{at:03d}", "obj", obj, objects=qbruntime.extras(tuple(pairs), False), budget_ms=200)
        for at, (obj, pairs) in enumerate(objs.values())
    ]
    run = work / "link_ref"
    dosbatch.run(jobs, run)
    out = {}
    for name, job in zip(objs, jobs):
        out[name] = work / f"{name}.ref.exe"
        shutil.copy(run / f"{job.stem.upper()}.EXE", out[name])
    return out


def built(sources: dict[str, Path], work: Path) -> dict[str, tuple[Path, dict]]:
    """`reference` and ours at each target (qbbuild: the llrm runtime, jwlink): (reference exe, {target: Built}) by name."""
    import qbbuild

    refs = reference(sources, work)
    objects = qbbuild.runtimes(work)
    return {name: (refs[name], qbbuild.build(source, work / f"b_{name}", objects)) for name, source in sources.items()}
