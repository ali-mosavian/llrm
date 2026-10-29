"""REDIM PRESERVE (runtime/qb/array.c, the m32 ad.c) on the host: a grow keeps the old elements and zeroes the new, a
shrink truncates, a changed lower bound is Subscript out of range, and plain REDIM still clears."""

import shutil
import subprocess
from pathlib import Path

import pytest

ROOT = Path(__file__).resolve().parent.parent


@pytest.mark.skipif(shutil.which("gcc") is None, reason="needs a host C compiler")
def test_redim_preserve_keeps_what_fits_and_zeroes_the_rest(tmp_path):
    exe = tmp_path / "preserve_test"
    build = subprocess.run(
        ["gcc", "-O1", "-g", "-w", "-D__cdecl=", "-I", str(ROOT / "tests/hostalloc/shim"), "-I", str(ROOT / "runtime/qb"), "-I", str(ROOT / "runtime/qb/dos/m32"), str(ROOT / "tests/hostarray/preserve_test.c"), "-o", str(exe)],
        capture_output=True,
        text=True,
    )
    assert build.returncode == 0, build.stderr[-900:]
    done = subprocess.run([str(exe)], capture_output=True, text=True)
    assert done.returncode == 0 and done.stdout.strip() == "ok", done.stdout + done.stderr[-300:]
