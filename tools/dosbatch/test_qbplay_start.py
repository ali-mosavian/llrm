"""Session start-up: the emulator freezes at launch until a `continue` reaches it after it has frozen (#1570)."""

from __future__ import annotations

import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).parent))

import qbplay  # noqa: E402


class FrozenAtLaunch:
    """An emulator that ignores the `continue` sent before it froze: it answers `socketFrozen` until it gets another."""

    def __init__(self, ignored: int):
        self.ignored, self.sent = ignored, []

    def send(self, command: dict) -> dict:
        self.sent.append(command["cmd"])
        if command["cmd"] == "continue":
            self.ignored -= 1
            return {"status": "ok"}
        frozen = self.ignored >= 0
        return {"status": "ok", "socketFrozen": frozen, "shellReady": not frozen}


def test_a_lost_continue_is_sent_again_while_the_emulator_is_frozen():
    """The gate's `test_graphics_probe` waited 40 minutes on an emulator frozen at F000:FFF0: its one `continue` was lost, and
    a frozen emulator burns none of the CPU time the wait was bounded by."""
    one = object.__new__(qbplay.Session)
    emulator = FrozenAtLaunch(ignored=1)
    one.send = emulator.send
    one.limit = lambda seconds: (lambda: len(emulator.sent) > 50)
    one.wait_for_shell()
    assert "continue" in emulator.sent
