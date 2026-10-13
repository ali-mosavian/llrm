"""The span fills of the QB runtime's graphics (runtime/qb/dos/gfxdev.c) on the host: each fill, for every operation, leaves the video
memory the pixel plot would."""

import shutil
import subprocess
from pathlib import Path

import pytest

ROOT = Path(__file__).resolve().parent.parent


@pytest.mark.skipif(shutil.which("gcc") is None, reason="needs a host C compiler")
def test_span_fills_leave_the_video_memory_the_pixel_plot_does(tmp_path):
    exe = tmp_path / "fill_test"
    build = subprocess.run(
        ["gcc", "-O1", "-g", "-w", "-I", str(ROOT / "tests/hostgfx/shim"), "-I", str(ROOT / "runtime/qb/dos"), str(ROOT / "tests/hostgfx/fill_test.c"), "-o", str(exe)],
        capture_output=True,
        text=True,
    )
    assert build.returncode == 0, build.stderr[-800:]
    done = subprocess.run([str(exe)], capture_output=True, text=True)
    assert done.returncode == 0 and done.stdout.strip() == "ok", done.stderr[-300:]
