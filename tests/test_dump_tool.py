"""The stage instrument must start before it can measure an object.

`tools/dump.py` used the name `regs` after importing the liveness module as
`liveness`; every invocation died while evaluating the function annotation,
before it read the object it was supposed to dump.
"""

import runpy
from pathlib import Path

import pytest


def test_dump_tool_module_loads(monkeypatch: pytest.MonkeyPatch) -> None:
    monkeypatch.setattr("sys.argv", ["dump.py", "--help"])
    try:
        runpy.run_path("tools/dump.py", run_name="__main__")
    except SystemExit as exit_:
        assert exit_.code == 0


def test_dump_tool_writes_live_stage_for_a_real_object(monkeypatch: pytest.MonkeyPatch, tmp_path: Path) -> None:
    """d_surf dumping reached live.txt then called the retired register-liveness API."""
    namespace = runpy.run_path("tools/dump.py")
    monkeypatch.setitem(namespace["dump_one"].__globals__, "OUT", tmp_path)
    output = namespace["dump_one"](Path("fixtures/omf/stride-v-plain.obj"))
    live = (output / "live.txt").read_text()
    assert "in={" in live and "out={" in live
