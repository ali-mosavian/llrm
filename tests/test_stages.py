"""The dump rule 4 sends you to, and what it was not saying.

`_operand` printed a cell's address and nothing else, so a based cell --
one that names the value that computed its address, and after allocation
the register that value was placed in -- rendered identically to one BC
addressed itself. Two stage files could differ in the only field that
mattered and diff clean.
"""

import sys
from pathlib import Path

from iced_x86 import Register

sys.path.insert(0, str(Path(__file__).resolve().parent.parent / "tools"))

import stages

from qbopt import ir
from qbopt.module import Addr
from qbopt.module import Space


def test_a_based_cell_renders_the_value_and_the_register_it_was_placed_in() -> None:
    """`[es:bx+0x0]` said nothing about which value reached it."""
    where = Addr(Space.LITERAL, 0x2, base=Register.SI)
    placed = ir.Mem(where, 2, Register.BX, 2, 1, base=ir.Held(17, 2))
    said = stages._operand(placed)
    assert "v17" in said, f"the cell does not say which value reached it: {said}"
    assert "BX" in said, f"the cell does not say where that value was placed: {said}"


def test_a_cell_nothing_based_renders_as_it_did() -> None:
    """Every other line in every stage file stays byte-identical."""
    where = Addr(Space.LITERAL, 0x2, base=Register.SI)
    assert stages._operand(ir.Mem(where, 2)) == f"[{where}]"


def test_a_based_cell_nothing_placed_says_so() -> None:
    """Before allocation there is no register, and the dump has to show
    that rather than print a register that is not there."""
    where = Addr(Space.LITERAL, 0x2, base=Register.SI)
    unplaced = ir.Mem(where, 2, Register.NONE, 2, 1, base=ir.Held(17, 2))
    said = stages._operand(unplaced)
    assert "v17" in said and "BX" not in said, said


def test_the_stage_dump_lowers_through_the_same_contracts_the_emitter_uses() -> None:
    """Rule 4 sends you here, so the tool has to run.

    `lower.lowered` and `mir.bodies` both take the per-site contract map
    now, and this called them with the old signatures -- so every dump
    raised `TypeError: lowered() missing 3 required positional arguments`
    and the one instrument the project reaches for first was unusable.
    """
    from pathlib import Path

    import stages as tool

    found, bodies, contracts = tool._bodies(Path("fixtures/omf/bools-q-O.obj").read_bytes())
    assert bodies, "the fixture raised nothing"

    made = []

    class Capture:
        def __call__(self, number: int, kind: str, name: str):
            from contextlib import nullcontext

            made.append((number, kind, name))
            return nullcontext()

    # The one map the raise was given, handed on to the lowering: built
    # twice it can be built differently, which is the whole reason
    # wholeseg.py threads a single object rather than an equal one.
    assert contracts, "the tool built no contract map"
    from qbopt import lower

    seen = []
    was = lower.lowered

    def watching(name, body, calls, absorbed, given):
        seen.append(given)
        return was(name, body, calls, absorbed, given)

    lower.lowered = watching
    try:
        tool._machine(bodies, found, Capture(), contracts)
    finally:
        lower.lowered = was
    assert made, "the machine view produced no stage"
    assert seen, "the machine view lowered nothing"
    assert all(one is contracts for one in seen), "the lowering was given a different map"


def test_the_machine_view_is_the_pass_series_own_bodies() -> None:
    """s20 sits next to s13 and must be the same program.

    The machine views were built by rewriting the object and raising the
    result, so the body lowered at s20 was not the body the passes above
    produced -- a second raise of already-emitted bytes. Diffing s13
    against s20 then compares two programs, which is how a miscompile in
    the first pass reads as an extra phi in the second.
    """
    from pathlib import Path

    import stages as tool

    reads, given = [], []
    was_bodies, was_machine = tool._bodies, tool._machine

    def watching(data):
        got = was_bodies(data)
        reads.append(got)
        return got

    def receiving(bodies, found, view, contracts):
        given.append((found, contracts, bodies))
        return was_machine(bodies, found, view, contracts)

    tool._bodies, tool._machine = watching, receiving
    try:
        tool.main([str(Path("fixtures/omf/lngmix-p-g2.obj")), "--quiet", "--asm"])
    finally:
        tool._bodies, tool._machine = was_bodies, was_machine

    assert len(reads) == 1, f"the object was raised {len(reads)} times, not once"
    assert len(given) == 1, f"the machine view ran {len(given)} times"
    found, bodies, contracts = reads[0]
    theirs, mine, ours = given[0]
    assert theirs is found, "the machine view was given another module"
    assert mine is contracts, "the machine view was given another contract map"
    # The bodies are the pass series' own, which are not the raise's: every
    # pass returns new ones. What must hold is that they came from `main`'s
    # own loop over `found`, not from a second read.
    assert len(ours) == len(bodies), f"{len(ours)} bodies lowered, {len(bodies)} raised"
    assert [name for name, _one in ours] == [name for name, _one in bodies]
