"""
fixMul& end to end: declared, never defined, absorbed before LINK ever sees it.

There is no base build for this one. `declare function fixMul& (...)` names a
routine that exists nowhere -- not in the runtime library, not in this module --
so the unmodified object cannot link at all, and e2e.py's three-way
differential has nothing to compare against. What is checked instead: the
freshly emitted object drops every live reference to the call, links, and runs
to the golden.
"""

import shutil
from pathlib import Path

import pytest
from configs import CONFIGS
from dosbox import read_dos
from dosbox import dosbox_bin
from cache import cached_launch
from cache import toolchain_identity

from qbopt.objectfile import omf
from qbopt.rewrite import rewrite
from qbopt.objectfile import module
from qbopt.legacy.calls import sites
from qbopt.frontend.blocks import code_map
from qbopt.frontend.blocks import partition
from qbopt.legacy.calls import FIX_MULTIPLY
from qbopt.frontend.blocks import instructions

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
    shutil.rmtree(work, ignore_errors=True)
    work.mkdir(parents=True)
    (work / "FIXMUL.BAS").write_bytes((SUITE / "fixmul.bas").read_bytes())

    compiled = cached_launch(
        work,
        cfg.mount,
        [f"{cfg.bc} {cfg.switches} FIXMUL.BAS, FIXMUL.OBJ; > BC.OUT"],
        identity=toolchain_identity(cfg),
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
    mapped = code_map(parsed)
    assert not isinstance(mapped, str)
    blocks = partition(parsed, mapped)
    found = [s for s in sites(parsed, reached, blocks) if s.name == FIX_MULTIPLY]
    assert len(found) == 7, "one fixMul& call per case in the program"

    out, _ = rewrite(data, dry_run=False)
    live = {f.index for f in omf.fixups(omf.parse(out)) if f.target == "external"}
    assert FIX_MULTIPLY not in [n for i, n in enumerate(omf.externals(omf.parse(out))) if i in live], (
        "every reference to fixMul& must be gone -- LINK never resolves it, so one left is a linker failure"
    )
    (work / "FIXMULQ.OBJ").write_bytes(out)

    ran = cached_launch(
        work,
        cfg.mount,
        [
            f"{cfg.link} FIXMULQ.OBJ, FIXMUL.EXE,, {cfg.runtime}; > LINK.OUT",
            "FIXMUL.EXE > RUN.TXT",
        ],
        identity=toolchain_identity(cfg),
        timeout=180,
        env={"LIB": r"V:\LIB"},
    )
    assert ran.finished, "link/run did not return"
    link_out = read_dos(work, "LINK.OUT").lower()
    assert "unresolved external" not in link_out, "the EXTDEF must not be referenced by anything left in the object"

    got = lines(read_dos(work, "RUN.TXT"))
    want = lines((SUITE / "golden" / "fixmul.txt").read_text())
    assert len(got) == len(want)
    for one, other in zip(got, want, strict=True):
        # a DOUBLE's own PRINT precision is a real, measured compiler
        # difference -- QuickBASIC 4.5 shows sixteen significant digits for
        # 65535/65536 where VBDOS and PDS round to fifteen -- so an "F" line
        # compares as the value it means, not as the exact string
        if one.split("=", 1)[0].strip().startswith("F"):
            assert float(one.split("=", 1)[1]) == pytest.approx(float(other.split("=", 1)[1]))
        else:
            assert one == other
