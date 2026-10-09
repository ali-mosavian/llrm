"""The QB runtime against BCOM45: every probe in tests/qbrt and every milestone program, raw bytes."""

from __future__ import annotations

import sys
from pathlib import Path

import pytest

sys.path.insert(0, str(Path(__file__).parent))

import qbruntime  # noqa: E402
import run_tests  # noqa: E402

PROBES = sorted((qbruntime.dosbatch.ROOT / "tests" / "qbrt").glob("*.bas"))
SOURCES = {path.stem: path for path in [*PROBES, *qbruntime.milestone_sources()]}


@pytest.fixture(scope="module")
def results(tmp_path_factory):
    """Each program linked and run once against BCOM45 and once against the candidate."""
    if not qbruntime.dosbatch.QB45.is_dir():
        pytest.skip("QB45_DIR is unavailable")
    work = tmp_path_factory.mktemp("qbrt")
    objects = {}
    for name, source in SOURCES.items():
        obj = work / f"{name}.obj"
        error = run_tests.compile_one(run_tests.Program(source, ["-O2"], None, "qb45"), obj)
        assert error is None, error
        objects[name] = obj
    archive, _ = qbruntime.build(work / "archive")
    return qbruntime.differential_batch(objects, archive, work / "differential")


@pytest.mark.parametrize("name", sorted(SOURCES))
def test_program_matches_bcom45_byte_for_byte(results, name: str):
    result = results[name]
    assert result.reference.status == "ok"
    assert result.candidate.status == "ok", result.candidate.detail
    assert result.difference == ""
    assert result.screen_difference == ""
