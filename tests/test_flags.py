"""
The gate, and the measurement it rests on.

BC leaves the high half's flags; one 32-bit operation leaves the whole result's.
Where those differ, widening changes what a following jcc does -- silently, and
only on some values.
"""

from pathlib import Path
from dataclasses import replace

import pytest

from helpers import hx
from qbopt import module
from qbopt.flags import ALL
from qbopt.lift import lift
from qbopt.declen import run
from qbopt.flags import Flag
from qbopt.lift import Value
from qbopt.blocks import Ends
from qbopt.lift import needed
from qbopt.lift import refuse
from qbopt.blocks import Block
from qbopt.flags import effect
from qbopt.lift import regions
from qbopt.declen import decode
from qbopt.flags import live_in
from qbopt.lift import computes
from qbopt.blocks import code_map
from qbopt.flags import DIVERGENT
from qbopt.blocks import partition
from qbopt.flags import live_after
from qbopt.lift import emit_region
from qbopt.rewrite import flags_after

LOAD_AND_STORE = "A1 5E 00 8B 16 60 00  23 06 5A 00 23 16 5C 00  A3 62 00 89 16 64 00"
# long enough that the widened form plus the restore still fits, so the only
# thing that can refuse it is the flags
JUST_LOAD_STORE = "A1 5E 00 8B 16 60 00  A3 62 00 89 16 64 00A1 66 00 8B 16 68 00  A3 6A 00 89 16 6C 00"


def only_region(enc: str) -> tuple[list[Value], list[bool], list[int]]:
    code = hx(enc)
    values, _ = lift(code, 0, len(code))
    need = needed(values)
    return values, need, regions(values)[0]


def test_a_widened_operation_is_refused_when_a_flag_is_read() -> None:
    values, need, region = only_region(LOAD_AND_STORE)
    assert emit_region(values, need, region, Flag.ZF) is None


def test_the_same_region_is_taken_when_nothing_reads_a_flag() -> None:
    # Byte for byte the same input. A gate stuck shut fails here; a gate stuck
    # open fails above. There is no way to pass both without computing liveness.
    values, need, region = only_region(LOAD_AND_STORE)
    assert emit_region(values, need, region, Flag.NONE) is not None


@pytest.mark.parametrize("live", [Flag.CF, Flag.SF, Flag.OF, Flag.CF | Flag.SF | Flag.OF])
def test_the_flags_widening_never_changes_do_not_refuse(live: Flag) -> None:
    # CF, SF, OF and the computed value never differ between the two forms.
    values, need, region = only_region(LOAD_AND_STORE)
    assert emit_region(values, need, region, live) is not None


def test_a_region_that_computes_nothing_still_destroys_the_flags() -> None:
    # Only the divergence rule would let this through: the region is load and
    # store, so no flag's value changes. But putting the high half back is a
    # shr, which writes five of the six, and a jz after it goes the other way.
    values, need, region = only_region(JUST_LOAD_STORE)
    assert not any(values[i].op.startswith("alu") for i in region if need[i])
    plain = emit_region(values, need, region, Flag.NONE)
    guarded = emit_region(values, need, region, Flag.ZF)
    assert plain is not None and guarded is not None
    assert len(guarded) == len(plain), "both are padded to the bytes they replace"
    assert b"\x9c" in guarded and b"\x9d" in guarded, "the restore is wrapped in pushf/popf"
    assert b"\x9c" not in plain


def test_an_instruction_this_does_not_model_reads_everything() -> None:
    # Maximal uses, minimal definitions. Both directions push the gate toward
    # refusing, which is what stops a later simplification defaulting to "no
    # effect" and quietly opening the hole.
    unmodelled = decode(hx("D7"), 0)  # xlat
    assert unmodelled is not None
    assert effect(unmodelled) == effect(unmodelled).__class__(ALL, Flag.NONE)


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
    assert effect(insn).reads == reads


def test_a_call_destroys_the_flags_rather_than_reading_them() -> None:
    call = decode(hx("9A 00 00 00 00"), 0)
    assert call is not None
    assert effect(call) == effect(call).__class__(Flag.NONE, ALL)


def test_a_region_followed_by_a_call_is_always_safe() -> None:
    # The call writes every flag, so nothing before it can be read.
    values, need, region = only_region(LOAD_AND_STORE)
    assert refuse(values, need, region, Flag.NONE) is None


def test_liveness_across_a_real_module(fixtures: Path) -> None:
    found = module.load(fixtures / "jumptable.obj")
    assert found is not None
    mapped = code_map(found)
    assert not isinstance(mapped, str)
    blocks = partition(found, mapped)
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
    code = hx(enc)
    insns, _ = run(code, 0, len(code))
    shifted = [replace(insn, at=insn.at + at) for insn in insns]
    return Block(at, at + len(code), tuple(shifted), ends, succ)


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
    writes = a_block(0x00, "23 06 5A 00", Ends.FALLS_THROUGH, (0x10,))
    live = live_in([writes, ret])
    assert live[writes.at] == Flag.NONE


def test_the_gate_refuses_real_regions_not_just_invented_ones(fixtures: Path) -> None:
    # A gate that never fires is not a gate. Measured over the corpus: of 241
    # regions, 25 have a flag live afterwards and 22 of those compute something,
    # so widening them would change what the following jcc does. That is 9 per
    # cent of the work silently wrong, not a hypothetical.
    refused = considered = 0
    for path in sorted(fixtures.glob("*.obj")):
        found = module.load(path)
        assert found is not None
        mapped = code_map(found)
        if isinstance(mapped, str):
            continue
        blocks = partition(found, mapped)
        live = live_in(blocks)
        values, _ = lift(found.code, found.start, found.end, found.resolve)
        need = needed(values)
        for region in regions(values):
            considered += 1
            after = flags_after(blocks, live, values[region[0]].at, values[region[-1]].end)
            refused += computes(values, need, region) and bool(after & DIVERGENT)
    assert considered > 100
    assert refused >= 20, f"only {refused} of {considered} regions refused on flags"
