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
import qbplay  # noqa: E402
import qbruntime  # noqa: E402

pytestmark = pytest.mark.slow_dos
SOURCE = dosbatch.ROOT / "tests" / "qbrt" / "keys_trap.bas"
KEYS = ["f1", "f2", "a", "f1"]


def test_trapped_keys_run_their_handlers_and_are_not_read(tmp_path):
    if not dosbatch.QB45.is_dir():
        pytest.skip("QB45_DIR is unavailable")
    obj = tmp_path / "keys.llrm.obj"
    assert (error := qbruntime.compile_basic(SOURCE, obj, "llrm")) is None, error
    archive, _ = qbruntime.build(tmp_path / "archive")
    job = dosbatch.Job("LLRM", "obj", obj, runtime="llrmqb", runtime_file=archive, screen=True, budget_ms=30000)
    dosbatch.run([job], tmp_path / "build")
    work = tmp_path / "play"
    work.mkdir()
    shutil.copy(tmp_path / "build" / "LLRM.EXE", work / "P.EXE")
    session = qbplay.Session(work)
    try:
        session.send({"cmd": "dos_cmd", "command": "P.EXE"}, wait=False)
        session.wait_for("ready", 60)
        session.type(KEYS)
        session.wait_for("done", 60)
        screen = " ".join(session.screen().split())
    finally:
        session.close()
    assert "f1 = 2 f2 = 1 other = a" in screen, screen
