"""
Code motion, proved the only way it can be: build a program, move its code,
link it and run it.

A nop inserted at an instruction boundary changes nothing about what the program
computes and everything about where it lives. Every offset after it moves, so
every fixup, public symbol, line number, segment length and branch displacement
has to move with it. If any one of them is left behind the program still links
-- and then prints the wrong answer, or does not come back.
"""

from pathlib import Path

import e2e
import pytest
from configs import CONFIGS
from dosbox import dosbox_bin

from qbopt.objectfile import omf
from qbopt.objectfile import module
from qbopt.objectfile.relocate import Edit
from qbopt.objectfile.relocate import Shift
from qbopt.objectfile.relocate import relocate
from qbopt.frontend.blocks import instructions

pytestmark = [pytest.mark.e2e, pytest.mark.skipif(dosbox_bin() is None, reason="no dosbox-x")]

ROOT = Path(__file__).resolve().parents[1]

# one per compiler; the switch axes are covered by the differential itself
COMPILERS = ["v-g3", "p-g2", "q-O"]

# jumps included: its ON GOTO table is data inside the code segment, and moving
# code past it works only because reachability knows the bytes after a B$OGTA
# call are a table rather than instructions.
# fpemu included: a nop ahead of it moves all 27 of its int 34h..3Bh sites, the
# ones the FP emulator patches. It needs the entry to be right -- while that was
# searched for, the search picked one byte into a 66-prefixed store and the nop
# landed inside an instruction.
MOVABLE = ["arith", "procs", "jumps", "fpemu"]

CASES = [
    pytest.param(
        tag,
        program,
        marks=pytest.mark.skipif(not CONFIGS[tag].available, reason=f"no {tag} toolchain"),
    )
    for tag in COMPILERS
    for program in MOVABLE
]


def insert_a_nop(data: bytes) -> bytes:
    records = omf.parse(data)
    found = omf.code_segment(records)
    assert found is not None
    seg, _name, size = found
    image = omf.segment_image(records, seg, size)

    parsed = module.of(records)
    assert parsed is not None
    reached = instructions(parsed)
    assert not isinstance(reached, str), reached
    at = next(insn.at for insn in reached if insn.at >= 0x40)

    moved = relocate(records, seg, image, Shift.of([Edit(at, at, b"\x90")]))
    assert not isinstance(moved, str), moved
    assert b"".join(r.emit() for r in moved) != data, "the object must actually have changed"
    return b"".join(record.emit() for record in moved)


@pytest.mark.parametrize(("tag", "program"), CASES)
def test_a_program_survives_having_its_code_moved(tag: str, program: str) -> None:
    # its own workdir, not e2e.run()'s default BUILD/tag -- that one belongs
    # to test_e2e.py's own all-programs run under the same tag, and a
    # pytest-xdist worker running this case concurrently with that one would
    # otherwise race on the same directory
    work = ROOT / "build" / "motion" / tag / program
    result = e2e.run(tag, program, transform=insert_a_nop, work=work)
    failed = [v for v in result.verdicts if not v.ok]
    assert not failed, "; ".join(f"{v.program} {v.status}: {v.detail}" for v in failed)
