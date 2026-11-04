"""The box fills of the graphics screen under DOSBox, m16 and m32: every mode (SCREEN 13, 1, 2, 9, 7) and operation (set, and, or,
xor) leaves the pixels of a random box, a few dots and a line as the operation says and the others alone.  No BASIC statement fills with and, or or xor, and
the fills patch the opcode of their loop (gdfill.asm), so this is the one test of those patches."""

from __future__ import annotations

import subprocess
import sys
from pathlib import Path

import pytest

sys.path.insert(0, str(Path(__file__).parent))

import dosbatch  # noqa: E402
import qb32  # noqa: E402
import qbbuild  # noqa: E402
import qbruntime  # noqa: E402

SOURCE = dosbatch.ROOT / "tests" / "qbgfx" / "fill_ops.c"
NEEDED = ("gfxdev", "gdfill", "gdline", "gdxfer", "gdscan", "gdcirc", "bios", "ports", "block")


def compile_c(target: str, include: Path, platform: Path, obj: Path) -> None:
    dosbatch._host(*dosbatch.c_compile(SOURCE, obj, target, "-Os", include, [], ["-I", str(qbruntime.RUNTIME), "-I", str(platform), *qbruntime.device_includes(platform)]))


@pytest.fixture(scope="module")
def found(tmp_path_factory):
    if not qbruntime.dosbatch.DOSBOX.exists():
        pytest.skip("DOSBox-X is unavailable")
    work = tmp_path_factory.mktemp("gfxfill")
    jobs = []
    runtime16 = qbbuild.runtime16(work / "rt16")
    objects16 = tuple(runtime16[name] for name in NEEDED)
    obj16, exe16 = work / "f16.obj", work / "f16.exe"
    compile_c(dosbatch.REAL_MODE, dosbatch.c_include(dosbatch.REAL_MODE, work), qbruntime.platform_directory(), obj16)
    dosbatch.link_target(dosbatch.REAL_MODE, obj16, exe16, work, objects_after=objects16)
    jobs.append(dosbatch.Job("F16", "exe", exe16))
    runtime32 = qb32.build(work / "rt32")
    obj32, exe32 = work / "f32.obj", work / "f32.exe"
    compile_c(qb32.TARGET, dosbatch.c_include(qb32.TARGET, work), qb32.PLATFORM, obj32)
    loaders = dosbatch.link_target(qb32.TARGET, obj32, exe32, work, objects_after=tuple(runtime32[name] for name in NEEDED))
    jobs.append(dosbatch.Job("F32", "exe", exe32, files=loaders))
    results = qb32.run_jobs(jobs, work / "run")
    return {job.stem: (results[job.stem].status, qbruntime.raw_output(work / "run", job.stem).decode().strip()) for job in jobs}


def test_every_box_fill_leaves_what_its_operation_says(found):
    for stem, (status, printed) in found.items():
        assert (status, printed) == ("ok", "0"), stem
