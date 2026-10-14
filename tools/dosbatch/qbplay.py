"""Plays a DOS program under DOSBox-X through its debug socket: keys typed when the screen shows what a
script waits for, screenshots taken where it says.  The same script drives BCOM45's build and llrm's, so
their screens can be compared at the same points."""

from __future__ import annotations

import json
import os
import queue
import socket
import subprocess
import threading
import time
from pathlib import Path

import dosbatch

CONF = dosbatch.CONF


BACKSTOP = 3600  # wall seconds: for an emulator that waits and burns no CPU


class Session:
    def limit(self, seconds: float):
        """A function that says whether the emulator has used `seconds` of CPU time since now. The wall clock is the host's
        load: waiting 30 s for a DOS shell to come up failed whenever the host was busy. The wall only backs this up."""
        cpu, wall = dosbatch._cpu_seconds(self.process.pid), time.monotonic()
        return lambda: dosbatch._cpu_seconds(self.process.pid) - cpu > seconds or time.monotonic() - wall > BACKSTOP

    def __init__(self, work: Path, conf: str = CONF):
        self.work = work
        probe = socket.socket()
        probe.bind(("127.0.0.1", 0))
        port = probe.getsockname()[1]
        probe.close()
        (work / "play.conf").write_text(conf + f"[autoexec]\nmount c {work}\nc:\n")
        env = {**os.environ, "SDL_VIDEODRIVER": "dummy", "DOSBOX_DEBUG_PORT": str(port)}
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
        self.send({"cmd": "continue"})
        self.wait_for_shell()

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

    def wait_for_shell(self, seconds: float = 30) -> None:
        """Until the DOS shell takes commands: `wait_for_shell` can answer before the emulator has run far enough, so the
        status says."""
        late = self.limit(seconds)
        while not (status := self.send({"cmd": "status"})).get("shellReady"):
            if status.get("socketFrozen"):
                # The emulator freezes at launch and waits for `continue`; one sent before it had frozen was lost, and a frozen
                # emulator burns no CPU, so `late` never ended the wait (gate 65: 40 minutes, #1570).
                self.send({"cmd": "continue"})
            if late():
                raise TimeoutError("the DOS shell never came up")
            time.sleep(0.01)

    def wait_event(self, name: str, since: int, seconds: float) -> dict:
        """The first event called `name` among those after the first `since`; the wait ends the moment it arrives.  With an
        emulator behind the session `seconds` are its CPU time, else the wall clock's."""
        late = self.limit(seconds) if getattr(self, "process", None) else None
        deadline = time.monotonic() + seconds
        with self.arrived:
            while True:
                for event in self.events[since:]:
                    if event.get("event") == name:
                        return event
                if (late() if late else time.monotonic() >= deadline):
                    raise TimeoutError(f"no {name} event in {seconds} s")
                self.arrived.wait(0.2 if late else max(deadline - time.monotonic(), 0.001))

    def idle(self, seconds: float = 60) -> None:
        """Until the program reads the keyboard and finds nothing there: it has done what the keys, or its start, asked."""
        since = len(self.events)
        self.send({"cmd": "key_wait_arm"})
        self.wait_event("key_wait", since, seconds)

    def keys(self, keys: list[str], seconds: float = 60) -> None:
        """`keys` typed, and the program idle again: the next step needs no pause."""
        self.type(keys, pause=0)
        self.idle(seconds)  # armed after the keys: a read between two of them found nothing, and would end the wait early

    def settle(self, looks: int = 3, seconds: float = 60, stable_for: float = 0.0) -> None:
        """Until the screen is the same picture `looks` times running and for `stable_for` host seconds (by the hash of the
        adapter's memory): a program that computes for a while between two strokes looks done to a few quick looks."""
        late, same, last, since = self.limit(seconds), 0, None, time.monotonic()
        while not late():
            look = self.screen_hash()
            if look != last:
                same, since = 1, time.monotonic()
            else:
                same += 1
            last = look
            if same >= looks and time.monotonic() - since >= stable_for:
                return
        raise TimeoutError("the screen never settled")

    def screen_hash(self, length: int = 0x28000) -> str:
        """A hash of the adapter's video memory, every plane: the same two hashes are the same picture."""
        return str(self.send({"cmd": "mem_hash", "addr": 0, "len": length, "vram": True}).get("hash"))

    def screen(self) -> str:
        reply = self.send({"cmd": "text_screen"})
        text = reply.get("text") or ""
        return "\n".join(text) if isinstance(text, list) else text

    def wait_for(self, text: str, seconds: float = 60, poll: float = 0.25) -> None:
        late = self.limit(seconds)
        while text not in self.screen():
            if late():
                raise TimeoutError(f"{text!r} never appeared:\n{self.screen()}")
            time.sleep(poll)

    def wait_for_mode(self, mode: int, seconds: float = 60) -> None:
        """Until the screen is in the BIOS video mode `mode`."""
        late = self.limit(seconds)
        while self.send({"cmd": "text_screen"}).get("video_mode") != mode:
            if late():
                raise TimeoutError(f"the screen never went to mode {mode}")
            time.sleep(0.25)

    def type(self, keys: list[str], pause: float = 0.15) -> None:
        for key in keys:
            self.send({"cmd": "key", "key": key})
            time.sleep(pause)

    def shot(self, path: Path, settle: float = 0.5) -> None:
        """A PNG of the screen: the fork replies once the file is whole; `settle` is a pause after, for a caller that wants one."""
        path.unlink(missing_ok=True)
        reply = self.send({"cmd": "screenshot", "path": str(path), "timeout_ms": 5000})
        if reply.get("status") != "ok" or not path.exists():
            raise RuntimeError(f"screenshot: {reply}")
        time.sleep(settle)

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
