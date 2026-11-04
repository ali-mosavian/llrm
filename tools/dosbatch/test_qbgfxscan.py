"""A CGA PAINT scan under DOSBox, m32, with the upper half of ecx dirty: a row of one colour has no pixel of another, and B$FSCC counted
the bytes of the row in all of ecx, so a pointer left in ecx by the code before it ran the scan on past the row (color_masks crashed)."""

from __future__ import annotations

import sys
from pathlib import Path

import pytest

sys.path.insert(0, str(Path(__file__).parent))

import dosbatch  # noqa: E402
import qb32  # noqa: E402
import qbruntime  # noqa: E402

SOURCES = dosbatch.ROOT / "tests" / "qbgfx"
NEEDED = ("gfxdev", "gdfill", "gdline", "gdxfer", "gdscan", "gdcirc", "bios", "ports", "block")


@pytest.fixture(scope="module")
def found(tmp_path_factory):
    if not qbruntime.dosbatch.DOSBOX.exists():
        pytest.skip("DOSBox-X is unavailable")
    work = tmp_path_factory.mktemp("gfxscan")
    include = dosbatch.c_include(qb32.TARGET, work)
    obj, stub, exe = work / "s32.obj", work / "dirty.obj", work / "s32.exe"
    dosbatch._host(*dosbatch.c_compile(SOURCES / "scan_ecx.c", obj, qb32.TARGET, "-Os", include, [], ["-I", str(qbruntime.RUNTIME), "-I", str(qb32.PLATFORM), *qbruntime.device_includes(qb32.PLATFORM)]))
    dosbatch.assemble(SOURCES / "scan_ecx.asm", stub)
    runtime = qb32.build(work / "rt32")
    loaders = dosbatch.link_target(qb32.TARGET, obj, exe, work, objects_after=(stub, *(runtime[name] for name in NEEDED)))
    job = dosbatch.Job("S32", "exe", exe, files=loaders)
    results = qb32.run_jobs([job], work / "run")
    return results[job.stem].status, qbruntime.raw_output(work / "run", job.stem).decode().strip()


def test_a_row_of_one_colour_has_no_other_pixel_whatever_ecx_held(found):
    assert found == ("ok", "0")
