"""The QB runtime against BCOM45: every probe in tests/qbrt and every milestone program, raw bytes."""

from __future__ import annotations

import sys
from pathlib import Path

import pytest

sys.path.insert(0, str(Path(__file__).parent))

import qbruntime  # noqa: E402

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
        pair = (work / f"{name}.qb45.obj", work / f"{name}.llrm.obj")
        for runtime, obj in zip(("qb45", "llrm"), pair):
            error = qbruntime.compile_basic(source, obj, runtime)
            assert error is None, error
        objects[name] = pair
    archive, _ = qbruntime.build(work / "archive")
    return qbruntime.differential_batch(objects, archive, work / "differential")


# Milestone programs whose runtime entries are not ported yet.  Strict: a program that starts to pass
# must leave this set.
PENDING = {"huge"}


def case(name: str):
    return pytest.param(name, marks=pytest.mark.xfail(strict=True, reason="runtime entries not ported")) if name in PENDING else name


@pytest.mark.parametrize("name", [case(name) for name in sorted(SOURCES)])
def test_program_matches_bcom45_byte_for_byte(results, name: str):
    result = results[name]
    assert result.reference.status == "ok"
    assert result.candidate.status == "ok", result.candidate.detail
    assert result.difference == ""
    assert result.screen_difference == ""


def symbols(obj: Path, kind: int) -> set[str]:
    """The names an OMF object defines (PUBDEF, 0x90) or references (EXTDEF, 0x8C)."""
    data, at, found = obj.read_bytes(), 0, set()
    while at < len(data):
        record, size = data[at], data[at + 1] | data[at + 2] << 8
        body = data[at + 3 : at + 2 + size]
        at += 3 + size
        if record != kind:
            continue
        i = 0
        if kind == 0x90:  # the group and segment indexes, and a frame number where there is no segment
            i = 2 + (2 if body[1] == 0 else 0)
        while i < len(body):
            length = body[i]
            found.add(body[i + 1 : i + 1 + length].decode("latin-1"))
            i += 1 + length + (3 if kind == 0x90 else 1)
    return found


def test_every_runtime_entry_is_called_by_a_probe_under_llrm(tmp_path_factory):
    """A prototype that drifts from what llrm-qb passes breaks only the programs that call it, so an
    entry no probe calls would break silently."""
    if not qbruntime.dosbatch.QB45.is_dir():
        pytest.skip("QB45_DIR is unavailable")
    work = tmp_path_factory.mktemp("qbrt-entries")
    qbruntime.build(work / "archive")
    exported = set()
    for obj in (work / "archive").glob("*.obj"):
        exported |= {name for name in symbols(obj, 0x90) if name.startswith("B$")}
    called = set()
    for name, source in SOURCES.items():
        obj = work / f"{name}.obj"
        assert qbruntime.compile_basic(source, obj, "llrm") is None
        called |= symbols(obj, 0x8C)
    assert exported
    assert sorted(exported - called) == []
