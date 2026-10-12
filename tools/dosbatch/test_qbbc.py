"""Statements against what BC makes of them: each tests/qbrt/events_*.bas and bc_*.bas writes what happened to L.TXT, and the
llrm build's file must be BC's (BC compiles with /V, the line checks that event trapping needs)."""

from __future__ import annotations

import sys
from pathlib import Path

import pytest

sys.path.insert(0, str(Path(__file__).parent))

import dosbatch  # noqa: E402
import qb32  # noqa: E402
import qbbuild  # noqa: E402

# Waits cost no wall clock time: the clock is the guest's instruction count.
CONF = qb32.virtual_conf(2000)

pytestmark = pytest.mark.slow_dos
# (program, target) that differ from BC for a bug that is open: expected to differ, and a pass fails the test so the entry goes.
KNOWN = {("bc_sound", qbbuild.FLAT): "#1482: the flat runtime ends the program at the second error under ON ERROR, with L.TXT unwritten"}
SOURCES = sorted([*(dosbatch.ROOT / "tests" / "qbrt").glob("events_*.bas"), *(dosbatch.ROOT / "tests" / "qbrt").glob("bc_*.bas")])


@pytest.fixture(scope="module")
def objects(tmp_path_factory):
    return qbbuild.runtimes(tmp_path_factory.mktemp("qbbc_rt"))


def run_text(job: dosbatch.Job, where: Path) -> str:
    result = dosbatch.run([job], where, conf=CONF)[job.stem]
    assert result.status == "ok", result.detail
    return dosbatch.read_dos(where, "L.TXT")


@pytest.mark.parametrize("source", SOURCES, ids=lambda path: path.stem)
def test_program_matches_bc(tmp_path, objects, source: Path):
    """BC and BCOM45 are the oracle; the llrm build is ours, at m16 and at m32, and both must write BC's file."""
    if not dosbatch.QB45.is_dir():
        pytest.skip("QB45_DIR is unavailable")
    built = qbbuild.build(source, tmp_path / "build", objects)
    reference = tmp_path / "REF.BAS"
    reference.write_bytes(source.read_text().replace("\n", "\r\n").encode())
    switches = "/O /FPi /V" + (" /E /X" if "ON ERROR" in reference.read_text().upper() else "")
    want = run_text(dosbatch.Job("BCREF", "bas", reference, screen=True, budget_ms=60000, switches=switches), tmp_path / "BCREF")
    assert want.strip(), "BC's reference is missing"
    for target in qbbuild.TARGETS:
        one = built[target]
        got = run_text(dosbatch.Job("LLRM", "exe", one.exe, files=one.files, screen=True, budget_ms=60000), tmp_path / f"LLRM{target}")
        differ = [f"line {at}: BC {a!r}, llrm {b!r}" for at, (a, b) in enumerate(zip(want.split("\n"), got.split("\n"))) if a != b]
        if reason := KNOWN.get((source.stem, target)):
            assert want != got, f"m{target} now matches BC: remove it from KNOWN ({reason})"
            continue
        assert want == got, f"m{target}: " + ("\n".join(differ[:20]) or f"BC {want!r}, llrm {got!r}")
