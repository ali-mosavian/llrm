"""
The gate, and the measurement it rests on.

BC leaves the high half's flags; one 32-bit operation leaves the whole result's.
Where those differ, widening changes what a following jcc does -- silently, and
only on some values.
"""

from pathlib import Path

import pytest

import corpus
from helpers import hx
from qbopt.flags import ALL
from qbopt.lift import lift
from qbopt.declen import run
from qbopt.flags import Flag
from qbopt.lift import FIXUP
from qbopt.lift import Value
from qbopt.blocks import Ends
from qbopt.lift import needed
from qbopt.lift import refuse
from qbopt.blocks import Block
from qbopt.lift import regions
from qbopt.declen import decode
from qbopt.flags import live_in
from qbopt.lift import computes
from qbopt.flags import DIVERGENT
from qbopt.flags import live_after
from qbopt.lift import emit_region
from qbopt.rewrite import flags_after

LOAD_AND_STORE = "A1 5E 00 8B 16 60 00  23 06 5A 00 23 16 5C 00  A3 62 00 89 16 64 00"
# long enough that the widened form plus the restore still fits, so the only
# thing that can refuse it is the flags
JUST_LOAD_STORE = "A1 5E 00 8B 16 60 00  A3 62 00 89 16 64 00A1 66 00 8B 16 68 00  A3 6A 00 89 16 6C 00"


def only_region(enc: str) -> tuple[list[Value], list[bool], list[int]]:
    code = hx(enc)
    values, _, _ = lift(code, 0, len(code))
    need = needed(values)
    return values, need, regions(values)[0]


def test_a_widened_operation_is_refused_when_a_flag_is_read() -> None:
    values, need, region = only_region(LOAD_AND_STORE)
    assert emit_region(values, need, region, Flag.ZF, frozenset()) is None


IMMEDIATE_LOAD_AND_STORE = "A1 5E 00 8B 16 60 00   05 00 00 83 D2 04   A3 62 00 89 16 64 00"


def test_an_immediate_alu_pair_is_refused_when_a_flag_is_read() -> None:
    # Op.ALUI sets flags the exact same way Op.ALUM does -- BC leaves the high
    # half's, one 32-bit op leaves the whole result's -- so it has to trip the
    # same DIVERGENT gate, not just the memory-operand form.
    values, need, region = only_region(IMMEDIATE_LOAD_AND_STORE)
    assert emit_region(values, need, region, Flag.ZF, frozenset()) is None


def test_an_immediate_alu_pair_is_taken_when_nothing_reads_a_flag() -> None:
    values, need, region = only_region(IMMEDIATE_LOAD_AND_STORE)
    assert emit_region(values, need, region, Flag.NONE, frozenset()) is not None


def test_the_same_region_is_taken_when_nothing_reads_a_flag() -> None:
    # Byte for byte the same input. A gate stuck shut fails here; a gate stuck
    # open fails above. There is no way to pass both without computing liveness.
    values, need, region = only_region(LOAD_AND_STORE)
    assert emit_region(values, need, region, Flag.NONE, frozenset()) is not None


@pytest.mark.parametrize("live", [Flag.CF, Flag.SF, Flag.OF, Flag.CF | Flag.SF | Flag.OF])
def test_the_flags_widening_never_changes_do_not_refuse(live: Flag) -> None:
    # CF, SF, OF and the computed value never differ between the two forms.
    values, need, region = only_region(LOAD_AND_STORE)
    assert emit_region(values, need, region, live, frozenset()) is not None


def test_putting_the_high_half_back_writes_no_flag() -> None:
    # It used to be mov edx,eax / shr edx,16, and the shr wrote five of the six,
    # so a region of pure load and store changed no flag's value and still
    # destroyed what followed it. Through the stack it writes nothing, which is
    # both shorter and why the gate needs only the divergence mask.
    #
    # Decoded rather than asserted by hand: if this sequence ever changes to one
    # that writes flags, this fails, and the destruction mask has to come back.
    for restore in FIXUP.values():
        insns, gave_up = run(restore, 0, len(restore))
        assert gave_up is None
        assert not any(insn.writes for insn in insns)


def test_a_region_that_computes_nothing_is_taken_whatever_is_live() -> None:
    values, need, region = only_region(JUST_LOAD_STORE)
    assert not computes(values, need, region)
    for live in (Flag.NONE, Flag.ZF, ALL):
        assert emit_region(values, need, region, live, frozenset()) is not None


@pytest.mark.parametrize(
    ("enc", "reads"),
    [
        ("74 02", Flag.ZF),
        ("75 02", Flag.ZF),
        ("72 02", Flag.CF),
        ("77 02", Flag.CF | Flag.ZF),
        ("78 02", Flag.SF),
        ("7A 02", Flag.PF),
        ("7C 02", Flag.SF | Flag.OF),
        ("7F 02", Flag.ZF | Flag.SF | Flag.OF),
    ],
)
def test_a_conditional_reads_only_its_own_condition(enc: str, reads: Flag) -> None:
    insn = decode(hx(enc), 0)
    assert insn is not None
    assert Flag(insn.reads & ALL) == reads


def test_a_call_destroys_the_flags_rather_than_reading_them() -> None:
    # The callee is free to leave them however it likes, so nothing before a
    # call can have a flag read across it.
    call = decode(hx("9A 00 00 00 00"), 0)
    assert call is not None
    assert Flag(call.reads & ALL) == Flag.NONE


def test_a_region_followed_by_a_call_is_always_safe() -> None:
    # The call writes every flag, so nothing before it can be read.
    values, need, region = only_region(LOAD_AND_STORE)
    assert refuse(values, need, region, Flag.NONE) is None


def test_liveness_across_a_real_module(fixtures: Path) -> None:
    found = corpus.loaded(fixtures / "jumptable.obj")
    assert found is not None
    mapped = corpus.mapped(fixtures / "jumptable.obj")
    assert not isinstance(mapped, str)
    blocks = corpus.partitioned(fixtures / "jumptable.obj")
    live = live_in(blocks)

    for block in blocks:
        # nothing may be live that no successor reads and this block does not read
        assert live[block.at] & ~ALL == Flag.NONE
        if block.leaves:
            assert live_after(block, block.end, live) == ALL


def test_divergent_is_the_three_that_differ() -> None:
    assert DIVERGENT == Flag.ZF | Flag.PF | Flag.AF
    assert DIVERGENT & (Flag.CF | Flag.SF | Flag.OF) == Flag.NONE


def a_block(at: int, enc: str, ends: Ends, succ: tuple[int, ...]) -> Block:
    # decoded at the address it will live at, so nothing has to be shifted after
    code = bytes(at) + hx(enc)
    insns, _ = run(code, at, len(code))
    return Block(at, len(code), tuple(insns), ends, succ)


def test_a_block_whose_successors_are_unknown_keeps_every_flag_live() -> None:
    # A FUNCTION can return its answer in the flags, and B$CPI4 proves BC thinks
    # that way. Anything this cannot see the other side of has to assume the
    # worst, or a region before a ret is widened and the caller reads a flag
    # that changed.
    ret = a_block(0x10, "C3", Ends.RETURN, ())
    before = a_block(0x00, "90", Ends.FALLS_THROUGH, (0x10,))
    live = live_in([before, ret])
    assert live[ret.at] == ALL, "a ret leaves, so everything is live on entry to it"
    assert live[before.at] == ALL, "and that reaches back through the block before it"


def test_a_block_that_writes_a_flag_stops_it_being_live_earlier() -> None:
    # and [mem] writes all six, so nothing before it can have them read.
    ret = a_block(0x10, "C3", Ends.RETURN, ())
    computing = a_block(0x00, "23 06 5A 00", Ends.FALLS_THROUGH, (0x10,))
    live = live_in([computing, ret])
    assert live[computing.at] == Flag.NONE


def test_no_bc_output_in_the_corpus_reads_a_flag_a_widened_region_leaves(fixtures: Path) -> None:
    """The gate does not fire on real code, and that is worth knowing.

    Measured over the whole corpus, suite/flags.bas included -- which computes
    into a value whose high half is zero and branches on it immediately, in both
    the split and the fused form: no region is followed by a read of ZF, PF or
    AF. BC cannot use those flags. The ones its own code leaves describe the high
    half alone, which answers nothing about the long, so it always materialises
    the result and compares it explicitly.

    So the gate is insurance rather than a working part, and the accept/refuse
    pair above is what proves it works. If this ever stops being zero, BC has
    started doing something new and the gate is earning its place.
    """
    fired = considered = 0
    for path in sorted(fixtures.glob("*.obj")):
        found = corpus.loaded(path)
        assert found is not None
        mapped = corpus.mapped(path)
        if isinstance(mapped, str):
            continue
        blocks = corpus.partitioned(path)
        live = live_in(blocks)
        reached = corpus.reached(path)
        assert not isinstance(reached, str)
        values, _, _ = lift(found.code, found.start, found.end, found.resolve, reached)
        need = needed(values)
        for region in regions(values):
            considered += 1
            after = flags_after(blocks, live, values[region[0]].at, values[region[-1]].end)
            fired += computes(values, need, region) and bool(after & DIVERGENT)
    assert considered > 200
    assert fired == 0, f"{fired} of {considered} regions now read a flag widening changes"
