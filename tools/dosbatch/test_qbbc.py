"""Statements against what BC makes of them: each tests/qbrt/events_*.bas and bc_*.bas writes what happened to L.TXT, and the
llrm build's file must be BC's (BC compiles with /V, the line checks that event trapping needs)."""

from __future__ import annotations

import sys
from pathlib import Path

import pytest

sys.path.insert(0, str(Path(__file__).parent))

import dosbatch  # noqa: E402
import qbruntime  # noqa: E402

pytestmark = pytest.mark.slow_dos
SOURCES = sorted([*(dosbatch.ROOT / "tests" / "qbrt").glob("events_*.bas"), *(dosbatch.ROOT / "tests" / "qbrt").glob("bc_*.bas")])


@pytest.mark.parametrize("source", SOURCES, ids=lambda path: path.stem)
def test_program_matches_bc(tmp_path, source: Path):
    if not dosbatch.QB45.is_dir():
        pytest.skip("QB45_DIR is unavailable")
    obj = tmp_path / "lp.llrm.obj"
    assert (error := qbruntime.compile_basic(source, obj, "llrm")) is None, error
    archive, _ = qbruntime.build(tmp_path / "archive")
    reference = tmp_path / "REF.BAS"
    reference.write_bytes(source.read_text().replace("\n", "\r\n").encode())
    jobs = [
        dosbatch.Job("BCREF", "bas", reference, screen=True, budget_ms=60000, switches="/O /FPi /V" + (" /E /X" if "ON ERROR" in reference.read_text().upper() else "")),
        dosbatch.Job("LLRM", "obj", obj, runtime="llrmqb", runtime_file=archive, screen=True, budget_ms=60000),
    ]
    texts = []
    for job in jobs:
        where = tmp_path / job.stem
        result = dosbatch.run([job], where)[job.stem]
        assert result.status == "ok", result.detail
        texts.append(dosbatch.read_dos(where, "L.TXT"))
    assert texts[0].strip(), "BC's reference is missing"
    differ = [
        f"line {at}: BC {one!r}, llrm {two!r}"
        for at, (one, two) in enumerate(zip(texts[0].split("\n"), texts[1].split("\n")))
        if one != two
    ]
    assert texts[0] == texts[1], "\n".join(differ[:20]) or f"BC {texts[0]!r}, llrm {texts[1]!r}"
