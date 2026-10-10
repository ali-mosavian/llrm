"""The slow DOS tier is skipped unless it is asked for."""

import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).parent))

import conftest  # noqa: E402


def test_the_slow_tier_runs_only_when_named():
    """The demos and graphics sweeps (minutes) ran on every change with the quick probes."""
    assert not conftest.asked_for("", {})
    assert conftest.asked_for("slow_dos", {})
    assert conftest.asked_for("", {"LLRM_DOS_SLOW": "1"})
    assert not conftest.asked_for("", {"LLRM_DOS_SLOW": "0"})
