"""STEP, LINE -(x, y) and the LINE style, against what BC makes of them: tests/qbrt/lastpoint_*.bas (one per gap: STEP, style, omitted first point) mark the last point after each
statement and writes a checksum of each screen row to L.TXT, and the llrm build's file must be BC's.

Before the fix the front end took STEP's operands as absolute and refused the style and the missing first point."""

from __future__ import annotations

import sys
from pathlib import Path

import pytest

sys.path.insert(0, str(Path(__file__).parent))

import dosbatch  # noqa: E402
import qbbuild  # noqa: E402

pytestmark = pytest.mark.slow_dos
SOURCES = sorted((dosbatch.ROOT / "tests" / "qbrt").glob("lastpoint_*.bas"))


@pytest.fixture(scope="module")
def objects(tmp_path_factory):
    return qbbuild.runtimes(tmp_path_factory.mktemp("qbgfxstep_rt"))


def run_text(job: dosbatch.Job, where: Path) -> str:
    result = dosbatch.run([job], where)[job.stem]
    assert result.status == "ok", result.detail
    return dosbatch.read_dos(where, "L.TXT")


@pytest.mark.parametrize("source", SOURCES, ids=lambda path: path.stem)
def test_graphics_statement_matches_bc(tmp_path, objects, source: Path):
    """BC's file is the oracle; ours, at m16 and at m32, must be the same."""
    if not dosbatch.QB45.is_dir():
        pytest.skip("QB45_DIR is unavailable")
    built = qbbuild.build(source, tmp_path / "build", objects)
    reference = tmp_path / "LASTPT.BAS"
    reference.write_bytes(source.read_text().replace("\n", "\r\n").encode())
    want = run_text(dosbatch.Job("BCREF", "bas", reference, screen=True, budget_ms=30000), tmp_path / "BCREF")
    assert want.count("\n") > 150, "BC's reference is missing"
    for target in qbbuild.TARGETS:
        one = built[target]
        got = run_text(dosbatch.Job("LLRM", "exe", one.exe, files=one.files, screen=True, budget_ms=30000), tmp_path / f"LLRM{target}")
        assert want == got, f"m{target}"
