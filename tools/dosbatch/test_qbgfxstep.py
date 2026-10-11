"""STEP, LINE -(x, y) and the LINE style, against what BC makes of them: tests/qbrt/lastpoint_*.bas (one per gap: STEP, style, omitted first point) mark the last point after each
statement and writes a checksum of each screen row to L.TXT, and the llrm build's file must be BC's.

Before the fix the front end took STEP's operands as absolute and refused the style and the missing first point."""

from __future__ import annotations

import sys
from pathlib import Path

import pytest

sys.path.insert(0, str(Path(__file__).parent))

import dosbatch  # noqa: E402
import qbruntime  # noqa: E402

pytestmark = pytest.mark.slow_dos
SOURCES = sorted((dosbatch.ROOT / "tests" / "qbrt").glob("lastpoint_*.bas"))


@pytest.mark.parametrize("source", SOURCES, ids=lambda path: path.stem)
def test_graphics_statement_matches_bc(tmp_path, source: Path):
    if not dosbatch.QB45.is_dir():
        pytest.skip("QB45_DIR is unavailable")
    obj = tmp_path / "lp.llrm.obj"
    assert (error := qbruntime.compile_basic(source, obj, "llrm")) is None, error
    archive, _ = qbruntime.build(tmp_path / "archive")
    reference = tmp_path / "LASTPT.BAS"
    reference.write_bytes(source.read_text().replace("\n", "\r\n").encode())
    jobs = [
        dosbatch.Job("BCREF", "bas", reference, screen=True, budget_ms=30000),
        dosbatch.Job("LLRM", "obj", obj, runtime="llrmqb", runtime_file=archive, screen=True, budget_ms=30000),
    ]
    texts = []
    for job in jobs:
        where = tmp_path / job.stem
        result = dosbatch.run([job], where)[job.stem]
        assert result.status == "ok", result.detail
        texts.append(dosbatch.read_dos(where, "L.TXT"))
    assert texts[0].count("\n") > 150, "BC's reference is missing"
    assert texts[0] == texts[1]
