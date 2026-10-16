"""NIBBLES and GORILLA give the same pictures, random parts included, whatever the speed of the host (qb32demos.py plays them in lockstep)."""

from __future__ import annotations

import os
import shutil
import signal
import subprocess
import sys
import threading
import time
from pathlib import Path

import pytest

REQUIRES = ["qb45"]

sys.path.insert(0, str(Path(__file__).parent))

import dosbatch  # noqa: E402
import qb32demos  # noqa: E402
import qbbuild  # noqa: E402
import qbplay  # noqa: E402
import qbruntime  # noqa: E402

pytestmark = pytest.mark.slow_dos


def slow_emulator(monkeypatch, share: float) -> None:
    """The emulator gets `share` of every 40 ms of the host: stopped for the rest, as on a starved runner."""
    real = subprocess.Popen

    def popen(*args, **kwargs):
        process = real(*args, **kwargs)

        def starve():
            while process.poll() is None:
                time.sleep(0.04 * share)
                try:
                    os.kill(process.pid, signal.SIGSTOP)
                    time.sleep(0.04 * (1 - share))
                    os.kill(process.pid, signal.SIGCONT)
                except ProcessLookupError:
                    return

        threading.Thread(target=starve, daemon=True).start()
        return process

    monkeypatch.setattr(qbplay.subprocess, "Popen", popen)


@pytest.mark.parametrize("name", ["NIBBLES", "GORILLA"])
def test_a_demo_shows_the_same_pictures_on_a_host_a_tenth_as_fast(name: str, tmp_path, monkeypatch):
    """GORILLA's skyline, the cursor's blink and where the number lies came from the clock at the instant the harness typed a key, and the
    harness typed on the host's time: 20 plays gave 17 different prompt1 pictures, and a starved runner saw 127 looks and 54 pictures
    of a screen that never settled. Every checkpoint, whole, must be the same bytes on a host ten times slower."""
    found, reason = qbruntime.demo_sources()
    if not found or not dosbatch.QB45.is_dir():
        pytest.skip(reason or "QB45_DIR is unavailable")
    built = qbbuild.build(found[f"{name}.BAS"], tmp_path / "build", qbbuild.runtimes(tmp_path))["x86-m16"]
    played = []
    for share in (1.0, 0.1):
        with monkeypatch.context() as patch:
            if share < 1:
                slow_emulator(patch, share)
            play_dir = tmp_path / f"play{share}"
            play_dir.mkdir()
            for beside in built.files:
                shutil.copy(beside, play_dir / Path(beside).name)
            shots = qb32demos.play(built.exe, play_dir, qb32demos.script(name), name)
            played.append({checkpoint: path.read_bytes() for checkpoint, path in shots.items()})
    assert played[0].keys() == played[1].keys()
    assert [c for c in played[0] if played[0][c] != played[1][c]] == []
