"""The QB runtime's graphics against BCOM45: each tests/qbrt/gfx_*.bas runs to a SLEEP under DOSBox-X, once as
BCOM45 builds it and once with the llrm runtime, and the two screenshots must have no pixel that differs."""

from __future__ import annotations

import shutil
import sys
import time
from pathlib import Path

import pytest

sys.path.insert(0, str(Path(__file__).parent))

import dosbatch  # noqa: E402
import qbplay  # noqa: E402
import qbruntime  # noqa: E402

PROBES = sorted((dosbatch.ROOT / "tests" / "qbrt").glob("gfx_*.bas"))
SETTLE_SECONDS = 2
GRAPHICS_MODE = 0x10   # SCREEN 9: what the probes draw in


@pytest.fixture(scope="module")
def programs(tmp_path_factory):
    """Both builds of every probe, linked: (reference EXE, candidate EXE) by name."""
    if not dosbatch.QB45.is_dir():
        pytest.skip("QB45_DIR is unavailable")
    work = tmp_path_factory.mktemp("qbgfx")
    objects = {}
    for source in PROBES:
        pair = (work / f"{source.stem}.qb45.obj", work / f"{source.stem}.llrm.obj")
        for runtime, obj in zip(("qb45", "llrm"), pair):
            error = qbruntime.compile_basic(source, obj, runtime)
            assert error is None, error
        objects[source.stem] = (*pair, *qbruntime.linked_objects(source, work))
    archive, _ = qbruntime.build(work / "archive")
    exes: dict[str, tuple[Path, Path]] = {}
    for tag, runtime in (("ref", "bcom45"), ("cand", "llrmqb")):
        jobs = []
        for at, (name, (reference, candidate, *more)) in enumerate(objects.items()):
            stem = f"J{at:03d}"
            if tag == "ref":
                jobs.append(dosbatch.Job(stem, "obj", reference, objects=tuple(more), budget_ms=200))
            else:
                jobs.append(
                    dosbatch.Job(stem, "obj", candidate, runtime=runtime, runtime_file=archive, objects=tuple(more), budget_ms=200)
                )
        run = work / f"link_{tag}"
        dosbatch.run(jobs, run)
        for name, job in zip(objects, jobs):
            kept = work / f"{name}.{tag}.exe"
            shutil.copy(run / f"{job.stem.upper()}.EXE", kept)
            exes.setdefault(name, [None, None])[tag == "cand"] = kept
    return {name: tuple(pair) for name, pair in exes.items()}


def screenshot(exe: Path, work: Path) -> Path:
    work.mkdir(exist_ok=True)
    shutil.copy(exe, work / "P.EXE")
    session = qbplay.Session(work)
    try:
        session.send({"cmd": "dos_cmd", "command": "P.EXE"}, wait=False)
        session.wait_for_mode(GRAPHICS_MODE)
        time.sleep(SETTLE_SECONDS)
        session.shot(work / "shot.png")
    finally:
        session.close()
    return work / "shot.png"


@pytest.mark.parametrize("name", [path.stem for path in PROBES])
def test_graphics_probe_has_no_pixel_different_from_bcom45(programs, tmp_path, name: str):
    reference, candidate = programs[name]
    want = screenshot(reference, tmp_path / "ref")
    got = screenshot(candidate, tmp_path / "cand")
    assert qbplay.differing_pixels(want, got) == 0
