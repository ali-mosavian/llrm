"""
extent.py needs no CodeView, so CodeView is the check on it: compile the same
source with /Zi and assert the reachability-derived procedure ranges equal
cvinfo.Procedure.offset/proc_length exactly.
"""

import tempfile
from pathlib import Path

import pytest
from configs import CONFIGS
from dosbox import read_dos
from dosbox import dosbox_bin
from cache import cached_launch
from cache import toolchain_identity

from qbopt import omf
from qbopt import cvinfo
from qbopt import module
from qbopt.extent import BodyKind
from qbopt.extent import Partition
from qbopt.extent import partition

pytestmark = [pytest.mark.e2e, pytest.mark.skipif(dosbox_bin() is None, reason="no dosbox-x")]

ROOT = Path(__file__).resolve().parents[1]
SUITE = ROOT / "suite"

COMPILERS = ["v-g3", "p-g2", "q-noO"]

CASES = [
    pytest.param(
        tag,
        marks=pytest.mark.skipif(not CONFIGS[tag].available, reason=f"no {tag} toolchain"),
    )
    for tag in COMPILERS
]


def compile_with_cv(
    tag: str, program: str, source_dir: Path, extra: str = ""
) -> tuple[module.Module, cvinfo.DebugInfo]:
    cfg = CONFIGS[tag]
    (root := ROOT / "build" / "extent").mkdir(parents=True, exist_ok=True)
    # unique per call -- see test_cvinfo.py's compile_with_debug for why a
    # shared (tag, program) workdir races under pytest-xdist.
    work = Path(tempfile.mkdtemp(dir=root, prefix=f"{tag}-{program}-"))
    dos_name = f"{program.upper()[:8]}.BAS"
    (work / dos_name).write_bytes((source_dir / f"{program}.bas").read_bytes())
    obj_name = f"{program.upper()[:8]}.OBJ"
    run = cached_launch(
        work,
        cfg.mount,
        [f"{cfg.bc} /Zi {extra} {cfg.switches} {dos_name}, {obj_name}; > BC.OUT"],
        identity=toolchain_identity(cfg),
        timeout=180,
        env={"LIB": r"V:\LIB"},
    )
    assert run.finished, "compile did not return"
    obj = work / obj_name
    assert obj.is_file(), read_dos(work, "BC.OUT")
    records = omf.read(obj)
    found = module.of(records)
    assert found is not None
    return found, cvinfo.parse(records)


def _procedure_ranges(found_partition: Partition | str) -> dict[str | None, tuple[int, int]]:
    assert isinstance(found_partition, Partition)
    out = {}
    for body in found_partition.bodies:
        if body.kind is BodyKind.PROCEDURE:
            (span,) = body.ranges
            out[body.name] = span
    return out


def bare(name: str) -> str:
    return name.upper().rstrip("&$%!#")


@pytest.mark.parametrize("tag", CASES)
def test_procedure_extents_match_codeview_exactly(tag: str) -> None:
    found, info = compile_with_cv(tag, "procs", SUITE)
    found_partition = partition(found)
    assert not isinstance(found_partition, str)
    assert found_partition.complete

    measured = {bare(name): span for name, span in _procedure_ranges(found_partition).items() if name}
    expected = {bare(p.name): (p.offset, p.offset + p.proc_length) for p in info.procedures}
    assert measured == expected


@pytest.mark.parametrize("tag", CASES)
def test_procedure_extent_survives_an_inline_table_and_a_resume_map(tag: str) -> None:
    # The untested combination a design review flagged: an ON GOTO table and
    # an ON ERROR GOTO/RESUME map, both structurally similar to the frame a
    # SUB's own bytes sit inside, in the same module as a real SUB.
    found, info = compile_with_cv(tag, "errsub", SUITE / "cvonly", extra="/X")
    found_partition = partition(found)
    assert not isinstance(found_partition, str)
    assert found_partition.complete

    measured = {bare(name): span for name, span in _procedure_ranges(found_partition).items() if name}
    expected = {bare(p.name): (p.offset, p.offset + p.proc_length) for p in info.procedures}
    assert measured == expected
