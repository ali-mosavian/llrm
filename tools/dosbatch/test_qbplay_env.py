"""The emulator's environment in a played session."""
import subprocess
import sys
from pathlib import Path

import pytest

sys.path.insert(0, str(Path(__file__).parent))

import qbplay  # noqa: E402


def test_the_emulator_runs_in_utc_whatever_the_host_zone_is(monkeypatch, tmp_path):
    """Run 7: NIBBLES m16 died 38 pixels off BCOM45's on a UTC runner and matched on a CET dev box, because the guest's time of day (so
    RANDOMIZE TIMER's seed, so the game) followed the host's zone."""
    seen = {}

    class Stop(Exception):
        pass

    def popen(command, env=None, **kwargs):
        seen.update(env)
        raise Stop

    monkeypatch.setenv("TZ", "Europe/Berlin")
    monkeypatch.setattr(subprocess, "Popen", popen)
    with pytest.raises(Stop):
        qbplay.Session(tmp_path)
    assert seen["TZ"] == "UTC"
