"""Session start-up: the first `continue` is sent after the emulator has frozen at launch (#1570)."""

from __future__ import annotations

import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).parent))

import qbplay  # noqa: E402


class FrozenAtLaunch:
    """An emulator that answers `socketFrozen` only after a few polls: a `continue` sent before that was lost."""

    def __init__(self, polls: int):
        self.polls, self.sent = polls, []

    def send(self, command: dict) -> dict:
        self.sent.append(command["cmd"])
        self.polls -= 1
        return {"status": "ok", "socketFrozen": self.polls < 0}


def test_the_first_continue_waits_until_the_emulator_has_frozen():
    """The gate's `test_graphics_probe` waited 40 minutes on an emulator frozen at F000:FFF0: its one `continue` was sent before it had
    frozen and was lost, and a frozen emulator burns none of the CPU time the wait was bounded by."""
    one = object.__new__(qbplay.Session)
    emulator = FrozenAtLaunch(polls=3)
    one.send = emulator.send
    one.process = type("Running", (), {"poll": lambda self: None})()
    one.await_frozen()
    assert emulator.sent == ["status"] * 4 and "continue" not in emulator.sent


def test_two_sessions_never_pick_a_port_at_once(tmp_path):
    """The port was free between the pick and the emulator's bind: two sessions starting together could hold one and drive each other's emulator."""
    import threading
    import time

    inside, worst = [], []

    def session():
        with qbplay.port_claim():
            inside.append(1)
            worst.append(len(inside))
            time.sleep(0.05)
            inside.pop()

    threads = [threading.Thread(target=session) for _ in range(6)]
    [t.start() for t in threads]
    [t.join() for t in threads]
    assert max(worst) == 1
