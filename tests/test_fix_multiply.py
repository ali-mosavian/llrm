"""
fixMul& end to end: declared, never defined, absorbed before LINK ever sees it.

There is no base build for this one. `declare function fixMul& (...)` names a
routine that exists nowhere -- not in the runtime library, not in this module --
so the unmodified object cannot link at all, and e2e.py's three-way
differential has nothing to compare against. What is checked instead: the
rewritten object drops the call site entirely, keeps its EXTDEF the way an
absorbed call always does, links, and runs to the golden.
"""

from pathlib import Path

import pytest
from dosbox import launch
from configs import CONFIGS
from dosbox import read_dos
from dosbox import dosbox_bin

from qbopt import omf
from qbopt import module
from qbopt.calls import sites
from qbopt.rewrite import rewrite
from qbopt.calls import FIX_MULTIPLY
from qbopt.blocks import instructions

pytestmark = [pytest.mark.e2e, pytest.mark.skipif(dosbox_bin() is None, reason="no dosbox-x")]

ROOT = Path(__file__).resolve().parents[1]
SUITE = ROOT / "suite"

# one per compiler; the switch axes are covered by the differential elsewhere
COMPILERS = ["v-g3", "p-g2", "q-O"]

CASES = [
    pytest.param(
        tag,
        marks=pytest.mark.skipif(not CONFIGS[tag].available, reason=f"no {tag} toolchain"),
    )
    for tag in COMPILERS
]


def lines(text: str) -> list[str]:
    return [ln.rstrip() for ln in text.replace("\r\n", "\n").split("\n") if ln.strip()]


@pytest.mark.parametrize("tag", CASES)
def test_fixmul_absorbs_and_links_where_bc_alone_cannot(tag: str) -> None:
    cfg = CONFIGS[tag]
    work = ROOT / "build" / "fixmul" / tag
    work.mkdir(parents=True, exist_ok=True)
    (work / "FIXMUL.BAS").write_bytes((SUITE / "fixmul.bas").read_bytes())

    compiled = launch(
        work,
        cfg.mount,
        [f"{cfg.bc} {cfg.switches} FIXMUL.BAS, FIXMUL.OBJ; > BC.OUT"],
        timeout=180,
        env={"LIB": r"V:\LIB"},
    )
    assert compiled.finished, "compile did not return"
    obj = work / "FIXMUL.OBJ"
    assert obj.is_file(), read_dos(work, "BC.OUT")

    data = obj.read_bytes()
    parsed = module.of(omf.parse(data))
    assert parsed is not None
    reached = instructions(parsed)
    assert not isinstance(reached, str)
    found = [s for s in sites(parsed, reached) if s.name == FIX_MULTIPLY]
    assert len(found) == 7, "one fixMul& call per case in the program"

    fixmul_bytes = {parsed.code[s.start : s.end].hex() for s in found}
    out, regions = rewrite(data, dry_run=False)
    fixmul_regions = [r for r in regions if r.before in fixmul_bytes]
    assert len(fixmul_regions) == 7
    assert all(r.taken for r in fixmul_regions), [r.reason for r in fixmul_regions if not r.taken]
    live = {f.index for f in omf.fixups(omf.parse(out)) if f.target == "external"}
    assert FIX_MULTIPLY not in [n for i, n in enumerate(omf.externals(omf.parse(out))) if i in live], (
        "every reference to fixMul& must be gone -- LINK never resolves it, so one left is a linker failure"
    )
    (work / "FIXMULQ.OBJ").write_bytes(out)

    ran = launch(
        work,
        cfg.mount,
        [
            f"{cfg.link} FIXMULQ.OBJ, FIXMUL.EXE,, {cfg.runtime}; > LINK.OUT",
            "FIXMUL.EXE > RUN.TXT",
        ],
        timeout=180,
        env={"LIB": r"V:\LIB"},
    )
    assert ran.finished, "link/run did not return"
    link_out = read_dos(work, "LINK.OUT").lower()
    assert "unresolved external" not in link_out, "the EXTDEF must not be referenced by anything left in the object"

    got = lines(read_dos(work, "RUN.TXT"))
    want = lines((SUITE / "golden" / "fixmul.txt").read_text())
    assert got == want
