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
    def __init__(self, work: Path):
        self.work = work
        probe = socket.socket()
        probe.bind(("127.0.0.1", 0))
        port = probe.getsockname()[1]
        probe.close()
        (work / "play.conf").write_text(CONF + f"[autoexec]\nmount c {work}\nc:\n")
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
        text = reply.get("screen") or reply.get("text") or reply.get("rows") or ""
        return "\n".join(text) if isinstance(text, list) else text

    def wait_for(self, text: str, seconds: float = 60) -> None:
        deadline = time.monotonic() + seconds
        while text not in self.screen():
            if time.monotonic() > deadline:
                raise TimeoutError(f"{text!r} never appeared:\n{self.screen()}")
            time.sleep(0.25)

    def type(self, keys: list[str]) -> None:
        for key in keys:
            self.send({"cmd": "key", "key": key})
            time.sleep(0.15)

    def shot(self, path: Path) -> None:
        self.send({"cmd": "screenshot", "path": str(path)})
        time.sleep(0.5)

    def close(self) -> None:
        self.process.kill()
        self.process.wait()
