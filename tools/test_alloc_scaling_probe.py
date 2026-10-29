"""tools/test_alloc_scaling.py skips when llrm-c has no old allocator (#6040)."""

import stat
import sys
from pathlib import Path

import pytest

sys.path.insert(0, str(Path(__file__).resolve().parent))
import test_alloc_scaling as scaling  # noqa: E402


def fake_llrm_c(directory: Path, profile: str) -> None:
    compiler = directory / "llrm-c"
    compiler.write_text(f"#!/bin/sh\necho '{profile}' >&2\n")
    compiler.chmod(compiler.stat().st_mode | stat.S_IXUSR)


def profile(*rows: str) -> str:
    return "\n".join(f"[instr]      1.0 Minstr own      1.0 Minstr total      1x {row}" for row in rows)


@pytest.mark.parametrize("rows, skips", [
    (("base regalloc",), True),
    (("regalloc base",), True),
    (("facts intervals", "facts slots", "intervals homes"), True),
    (scaling.OLD_ALLOCATOR_ROWS, False),
])
def test_an_llrm_c_profile_without_every_old_allocator_row_skips_the_scaling_tests(tmp_path, monkeypatch, rows, skips):
    """A c-legacy llrm-c prints `regalloc base` but not the old allocator's other rows: the tests died with KeyError: 'facts intervals' and refused the runner's batches (#6040 alone, 14:22, with the one-row probe)."""
    fake_llrm_c(tmp_path, profile(*rows))
    monkeypatch.setenv("LLRM_BIN", str(tmp_path))
    monkeypatch.setattr(scaling.llrmbin, "stale_binaries", lambda *a, **k: [])
    probe = scaling.old_allocator.__wrapped__
    if skips:
        with pytest.raises(pytest.skip.Exception):
            probe()
    else:
        probe()
