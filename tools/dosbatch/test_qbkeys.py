"""ON KEY(n) GOSUB: tests/qbrt/keys_trap.bas runs with F1, F2, a, F1 typed into it.

BC is not the reference here: its traps hook the keyboard interrupt, which the keys DOSBox injects do not go through
(its build reads them as ordinary keys), so the expected screen is what QB's documentation says: the two function keys
are taken by their handlers, the other key is read by INKEY$."""

from __future__ import annotations

import shutil
import sys
from pathlib import Path

import pytest

sys.path.insert(0, str(Path(__file__).parent))

import dosbatch  # noqa: E402
import qb32demos  # noqa: E402
import qbbuild  # noqa: E402
import qbplay  # noqa: E402

pytestmark = pytest.mark.slow_dos
SOURCE = dosbatch.ROOT / "tests" / "qbrt" / "keys_trap.bas"
KEYS = ["f1", "f2", "a", "f1"]
PARK_MS = 5   # emulated ms the program runs before a key goes in, and after


def test_trapped_keys_run_their_handlers_and_are_not_read(tmp_path):
    built = qbbuild.build(SOURCE, tmp_path / "build", qbbuild.runtimes(tmp_path))
    for target in qbbuild.TARGETS:
        work = tmp_path / f"play{target}"
        work.mkdir()
        shutil.copy(built[target].exe, work / "P.EXE")
        for beside in built[target].files:
            shutil.copy(beside, work / Path(beside).name)
        session = qbplay.Session(work)
        try:
            session.launch(qb32demos.LONGEST_MS)
            session.run_until(lambda: "ready" in session.screen(), qb32demos.LONGEST_MS, "'ready'")
            # One key at a time, each into the phase of the program that takes it (the program waits for its handler before the next):
            # a timed park is long enough for the program to have moved on.
            session.resume(PARK_MS)
            for key in KEYS:
                session.send({"cmd": "key", "key": key})
                session.resume(PARK_MS)
            session.run_until(lambda: "done" in session.screen(), qb32demos.LONGEST_MS, "'done'")
            screen = " ".join(session.screen().split())
        finally:
            session.close()
        assert "f1 = 2 f2 = 1 other = a" in screen, f"{target}: {screen}"
