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


class Session:
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
        deadline = time.monotonic() + 30
        while True:
            try:
                self.sock = socket.create_connection(("127.0.0.1", port))
                break
            except OSError:
                if time.monotonic() > deadline:
                    raise
                time.sleep(0.2)
        self.pending = 0
        self.replies: queue.Queue = queue.Queue()
        self.events: list[dict] = []
        threading.Thread(target=self.read, args=(self.sock.makefile("r"),), daemon=True).start()
        self.send({"cmd": "continue"})
        time.sleep(1)
        self.send({"cmd": "wait_for_shell", "timeoutMs": 5000})

    def read(self, lines) -> None:
        """Replies go to the queue in the order their commands were sent; events are kept."""
        for line in lines:
            message = json.loads(line)
            if "status" in message:
                self.replies.put(message)
                continue
            self.events.append(message)
            if message.get("event") == "stopped":
                self.sock.sendall(b'{"cmd":"continue"}\n')

    def send(self, command: dict, wait: bool = True) -> dict | None:
        self.sock.sendall((json.dumps(command) + "\n").encode())
        if not wait:
            self.pending += 1
            return None
        while self.pending:
            self.replies.get(timeout=120)
            self.pending -= 1
        return self.replies.get(timeout=120)

    def screen(self) -> str:
        reply = self.send({"cmd": "text_screen"})
        text = reply.get("text") or ""
        return "\n".join(text) if isinstance(text, list) else text

    def wait_for(self, text: str, seconds: float = 60, poll: float = 0.25) -> None:
        deadline = time.monotonic() + seconds
        while text not in self.screen():
            if time.monotonic() > deadline:
                raise TimeoutError(f"{text!r} never appeared:\n{self.screen()}")
            time.sleep(poll)

    def wait_for_mode(self, mode: int, seconds: float = 60) -> None:
        """Until the screen is in the BIOS video mode `mode`."""
        deadline = time.monotonic() + seconds
        while self.send({"cmd": "text_screen"}).get("video_mode") != mode:
            if time.monotonic() > deadline:
                raise TimeoutError(f"the screen never went to mode {mode}")
            time.sleep(0.25)

    def type(self, keys: list[str], pause: float = 0.15) -> None:
        for key in keys:
            self.send({"cmd": "key", "key": key})
            time.sleep(pause)

    def shot(self, path: Path, settle: float = 0.5) -> None:
        """A PNG of the screen: it is written a frame after the reply, so this returns once the file is there and has
        stopped growing (at most 2 s); `settle` is a pause after, for a caller that wants one."""
        path.unlink(missing_ok=True)
        self.send({"cmd": "screenshot", "path": str(path)})
        deadline, size = time.monotonic() + 2, -1
        while time.monotonic() < deadline:
            now = path.stat().st_size if path.exists() else 0
            if now and now == size:
                break
            size = now
            time.sleep(0.01)
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


def built_pairs(sources: dict[str, Path], work: Path) -> dict[str, tuple[Path, Path]]:
    """Each program as two linked EXEs, BCOM45's and the llrm runtime's: (reference, candidate) by name."""
    import shutil

    import qbruntime

    objects = {}
    for name, source in sources.items():
        pair = (work / f"{name}.qb45.obj", work / f"{name}.llrm.obj")
        for runtime, obj in zip(("qb45", "llrm"), pair):
            error = qbruntime.compile_basic(source, obj, runtime)
            assert error is None, error
        objects[name] = (*pair, *qbruntime.linked_pairs(source, work))
    archive, _ = qbruntime.build(work / "archive")
    exes: dict[str, list[Path | None]] = {name: [None, None] for name in objects}
    for tag, runtime in (("ref", "bcom45"), ("cand", "llrmqb")):
        jobs = []
        for at, (reference, candidate, *pairs) in enumerate(objects.values()):
            stem = f"J{at:03d}"
            if tag == "ref":
                jobs.append(dosbatch.Job(stem, "obj", reference, objects=qbruntime.extras(tuple(pairs), False), budget_ms=200))
            else:
                jobs.append(
                    dosbatch.Job(
                        stem,
                        "obj",
                        candidate,
                        runtime=runtime,
                        runtime_file=archive,
                        objects=qbruntime.extras(tuple(pairs), True),
                        budget_ms=200,
                    )
                )
        run = work / f"link_{tag}"
        dosbatch.run(jobs, run)
        for name, job in zip(objects, jobs):
            kept = work / f"{name}.{tag}.exe"
            shutil.copy(run / f"{job.stem.upper()}.EXE", kept)
            exes[name][tag == "cand"] = kept
    return {name: (pair[0], pair[1]) for name, pair in exes.items()}
