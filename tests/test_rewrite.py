"""
The rewriter over the committed objects. No emulator, no compilers.

These are the invariants that stand between a change to the emitter and a
program that links, runs, and quietly computes the wrong thing.
"""

from pathlib import Path
from dataclasses import replace

import pytest
from iced_x86 import Code

import corpus
from qbopt import omf
from helpers import hx
from qbopt.calls import Kind
from qbopt.lift import FIXUP
from qbopt.blocks import Ends
from qbopt.calls import sites
from qbopt.blocks import Block
from qbopt.calls import Operand
from qbopt.declen import decode
from qbopt.flags import live_in
from qbopt.module import Module
from qbopt.relocate import Edit
from qbopt.blocks import CodeMap
from qbopt.calls import CallSite
from qbopt.calls import MULTIPLY
from qbopt.rewrite import Region
from qbopt.rewrite import Planned
from qbopt.rewrite import Combined
from qbopt.rewrite import POP_ROOT
from qbopt import registers as regs
from qbopt.rewrite import PUSH_HI_LO
from qbopt.declen import run as decode_run
from qbopt.rewrite import dead_pairs_after
from qbopt.rewrite import tail_widened_calls
from qbopt.rewrite import drop_restore_repush_round_trips

pytestmark = pytest.mark.corpus

# The only opcodes calls.absorb() ever replaces a non-COMPARE call with, so
# a combined edit's own bytes must decode to at least one of them or the
# call's own arithmetic never actually happened.
#
# SHL and LEA are here because a multiply by a constant the 386 can do
# without multiplying does not emit an imul at all -- calls.SCALES and
# _without_multiplying(). They belong to this set for the same reason the
# imul forms do: they ARE the arithmetic, not something around it.
CALL_REPLACEMENT_OPCODES = {
    Code.IMUL_R32_RM32,
    Code.IMUL_R32_RM32_IMM8,
    Code.IMUL_R32_RM32_IMM32,
    Code.IMUL_RM32,
    Code.IDIV_RM32,
    Code.SHRD_RM32_R32_CL,
    Code.SHRD_RM32_R32_IMM8,
    Code.SHL_RM32_IMM8,
    Code.LEA_R32_M,
}


def _combined(obj: Path) -> Combined:
    found = corpus.loaded(obj)
    assert found is not None
    mapped = corpus.mapped(obj)
    assert not isinstance(mapped, str)
    blocks = corpus.partitioned(obj)
    live = live_in(blocks)
    reached = corpus.reached(obj)
    assert not isinstance(reached, str)
    reg_live = regs.analyse(blocks)
    return tail_widened_calls(found, mapped, blocks, live, reg_live, sites(found, reached, blocks), [])


def test_dead_pairs_after_finds_a_provably_dead_register() -> None:
    # mov dx,5 overwrites dx before anything reads it, in a block that then
    # returns -- pair 0's own restore would be pure waste. bx is untouched,
    # and a block that leaves keeps anything untouched conservatively live
    # (the same reasoning flags.py's own live_in applies to the flags: a
    # FUNCTION can hand its answer back in a register the caller expects).
    code = hx("BA 05 00 C3")  # mov dx,5 / ret
    insns, _ = decode_run(code, 0, len(code))
    block = Block(0, len(code), tuple(insns), Ends.RETURN, ())
    live = regs.analyse([block])
    assert dead_pairs_after([block], live, 0, 0) == frozenset({0})


def test_dead_pairs_after_keeps_a_register_something_reads() -> None:
    # add ax,dx reads dx before anything could overwrite it -- pair 0 stays
    # live. mov bx,7 fully overwrites bx before the ret -- pair 1 is dead.
    # One block, one call, both answers -- proves the two pairs are judged
    # independently, not by one shared verdict.
    code = hx("01 D0 BB 07 00 C3")  # add ax,dx / mov bx,7 / ret
    insns, _ = decode_run(code, 0, len(code))
    block = Block(0, len(code), tuple(insns), Ends.RETURN, ())
    live = regs.analyse([block])
    assert dead_pairs_after([block], live, 0, 0) == frozenset({1})


def _round_trip_scaffold(pair: int, a_fixup: tuple = (), b_fixup: tuple = ()) -> tuple[Module, CodeMap, list[Planned]]:
    # a's own replacement ends in a restore; the object's own untouched bytes
    # right after a's span are push <hi>, push <lo> for the same pair; b's own
    # replacement starts with pop <root> for that pair -- docs/residue.md's B,
    # built directly rather than through a real widened region and a real
    # absorbed call, since the fold only cares about the final edits' own bytes.
    lo_a, hi_a = 0, 6
    gap_lo, gap_hi = hi_a, hi_a + 2
    lo_b, hi_b = gap_hi, gap_hi + 5

    code = bytearray(hi_b)
    code[gap_lo:gap_hi] = PUSH_HI_LO[pair]
    found = Module(records=[], seg=1, name="T", code=bytes(code), start=0, end=len(code), chunks=((0, len(code)),))
    mapped = CodeMap(starts=frozenset(), leaders=frozenset())

    a_edit = Edit(lo_a, hi_a, b"\x90" + FIXUP[pair], a_fixup)
    b_edit = Edit(lo_b, hi_b, POP_ROOT[pair] + b"\x90\x90\x90", b_fixup)
    planned = [
        Planned(Region(0, 1, lo_a, hi_a, "", a_edit.data.hex(), True, None), a_edit),
        Planned(Region(1, 1, lo_b, hi_b, "", b_edit.data.hex(), True, None), b_edit),
    ]
    return found, mapped, planned


@pytest.mark.parametrize("pair", (0, 1))
def test_drop_restore_repush_round_trips_folds_the_three_pieces(pair: int) -> None:
    found, mapped, planned = _round_trip_scaffold(pair)
    result = drop_restore_repush_round_trips(found, mapped, planned)

    assert len(result) == 3
    assert [one.region.taken for one in result] == [False, False, True]
    assert result[0].region.reason == "folded into a restore/re-push round-trip removal"
    assert result[0].edit is None and result[1].edit is None

    merged = result[2]
    assert merged.edit is not None
    assert (merged.edit.lo, merged.edit.hi) == (0, 13)
    assert merged.edit.data == b"\x90\x90\x90\x90", "the restore, the push pair, and the pop are all gone"


def test_drop_restore_repush_round_trips_carries_both_edits_own_fixups() -> None:
    a_fixup = ((0, "A"),)  # offset into a.edit.data's own untouched prefix byte
    b_fixup = ((3, "B"),)  # offset into b.edit.data, past its own 2-byte pop
    found, mapped, planned = _round_trip_scaffold(0, a_fixup, b_fixup)
    merged = drop_restore_repush_round_trips(found, mapped, planned)[2]
    assert merged.edit is not None
    # a's own fixup offset is untouched; b's shifts left by pop's own 2 bytes,
    # then right by what's left of a's own prefix (1 byte: the 0x90 before its
    # restore)
    assert merged.edit.fixups == ((0, "A"), (2, "B"))


def test_drop_restore_repush_round_trips_leaves_a_mismatched_gap_alone() -> None:
    found, mapped, planned = _round_trip_scaffold(0)
    # pair 1's push bytes where pair 0's restore expects its own
    code = bytearray(found.code)
    code[6:8] = PUSH_HI_LO[1]
    found = replace(found, code=bytes(code))
    assert drop_restore_repush_round_trips(found, mapped, planned) == planned


def test_drop_restore_repush_round_trips_leaves_a_non_pop_second_edit_alone() -> None:
    found, mapped, planned = _round_trip_scaffold(0)
    original = planned[1].edit
    assert original is not None
    b_edit = replace(original, data=b"\x90\x90\x90\x90\x90")  # no leading pop at all
    planned = [planned[0], Planned(planned[1].region, b_edit)]
    assert drop_restore_repush_round_trips(found, mapped, planned) == planned


def test_drop_restore_repush_round_trips_refuses_when_something_targets_the_gap() -> None:
    found, mapped, planned = _round_trip_scaffold(0)
    # a branch lands on the second push byte -- exactly what anchored_inside()
    # already refuses for an ordinary region, reused here unchanged.
    mapped = CodeMap(starts=frozenset(), leaders=frozenset({7}))
    assert drop_restore_repush_round_trips(found, mapped, planned) == planned


def test_a_dry_run_writes_the_input_back_unchanged(obj: Path) -> None:
    data = obj.read_bytes()
    out, _ = corpus.rewritten(obj, dry_run=True)
    assert out == data


def test_a_dry_run_takes_no_region(obj: Path) -> None:
    _, found = corpus.rewritten(obj, dry_run=True)
    assert [r for r in found if r.taken] == []


def test_a_real_pass_rewrites_the_code_and_keeps_the_records_readable(obj: Path) -> None:
    data = obj.read_bytes()
    out, found = corpus.rewritten(obj, dry_run=False)
    if not any(region.taken for region in found):
        assert out == b"".join(record.emit() for record in omf.parse(data))
        return
    before, after = omf.code_segment(omf.parse(data)), omf.code_segment(omf.parse(out))
    assert before is not None and after is not None
    # The segment may grow. Widening never makes it longer, but absorbing a
    # call can: a divide is eighteen bytes against fifteen under /G3, and what
    # it buys is a far call and the routine behind it.
    assert after[2] <= before[2] * 2, "and never by more than the code it replaces"
    before_names, after_names = omf.externals(omf.parse(data)), omf.externals(omf.parse(out))
    assert len(after_names) == len(before_names), "an absorbed call may drop its fixup but never its EXTDEF slot"
    live = {f.index for f in omf.fixups(omf.parse(out)) if f.target == "external"}
    assert all(after_names[i] == before_names[i] for i in live), (
        "a still-referenced external must keep the name its fixups were resolved against"
    )
    # an orphaned entry may be left with its own name -- the routine's own
    # .LIB satisfies it regardless of whether anything here still calls it --
    # or renamed to one something in the object still actually does call, but
    # never to a third, unrelated, unresolvable name
    for index in range(1, len(after_names)):
        if index in live:
            continue
        assert after_names[index] in (before_names[index], *(after_names[i] for i in live))


def test_rewriting_the_output_widens_or_absorbs_nothing_new(obj: Path) -> None:
    """A second pass finds no widening and no call left to absorb.

    It may find a load to delete, and that is not a failure of idempotence
    but the point: absorbing a call removes a barrier, and a store and
    reload the call used to sit between becomes visible only afterwards.
    procs-q-O ends up with `mov [bp-12h],eax` immediately followed by
    `mov eax,[bp-12h]`, which the first pass could not see because the call
    was still there when it looked.

    So the invariant is narrowed rather than dropped: everything that
    rewrites bytes in place must still reach a fixed point in one pass, and
    only deletion -- which is what a later pass creates work for -- may
    appear on the second. Running the pass to a fixed point would collect
    those too, and is its own piece of work.
    """
    out, _ = corpus.rewritten(obj, dry_run=False)
    _, again = corpus.rewritten(out, dry_run=False)
    left = [r for r in again if r.taken and r.after != ""]
    assert left == [], f"a second pass rewrote {len(left)} regions in place"


def test_a_region_is_either_taken_or_says_why_not(obj: Path) -> None:
    _, found = corpus.rewritten(obj, dry_run=False)
    for region in found:
        assert region.taken != bool(region.reason), "taken and refused are exclusive, and one holds"


def test_regions_are_taken_and_come_out_smaller(fixtures: Path) -> None:
    # A pass that refused everything would satisfy every invariant here.
    #
    # Measured: 195 of 229 regions are taken and shrink by a fifth. What is left
    # is 16 that cross a LEDATA boundary, 14 single pairs that still grow, and 4
    # that swallow a line number.
    taken = total = before = after = 0
    for path in sorted(fixtures.glob("*.obj")):
        _, found = corpus.rewritten(path, dry_run=False)
        total += len(found)
        for region in found:
            if region.taken:
                taken += 1
                before += region.end - region.at
                after += len(region.after or "") // 2
    assert total > 200
    assert taken > total * 3 // 4, f"only {taken} of {total} regions taken"
    assert after < before * 85 // 100, "a taken region is meaningfully smaller"


def test_a_call_whose_tail_widens_is_folded_into_one_combined_edit(operator_obj: Path) -> None:
    # docs/residue.md's G/H, corpus-wide: every OPERATOR_OBJECTS fixture has
    # at least one call this pass absorbs whose bytes right after it still
    # widen -- lift.tail() seeded with the call's own result, per
    # ir.RESTORE_EFFECTS[0], rather than the wall an unrecognised restore
    # idiom would otherwise be.
    assert _combined(operator_obj).planned


def test_a_combined_edit_never_drops_the_calls_own_arithmetic(operator_obj: Path) -> None:
    # needed()'s ordinary rule marks a region's own last value per pair
    # needed and back-propagates from there -- exactly right for a region a
    # LOAD starts, but a region an Op.CALL seeds could, in principle, have
    # that call's own value overwritten by a fresh, unrelated LOAD before
    # anything downstream reads it, which would leave it looking unneeded by
    # that rule alone even though its bytes still replace the deleted
    # pushes/call outright. tail_widened_calls() forces it needed regardless
    # -- this checks the combined edit's own bytes actually still contain
    # the call's real replacement instruction, not just proves the flag.
    for one in _combined(operator_obj).planned:
        assert one.edit is not None
        decoded, _ = decode_run(one.edit.data, 0, len(one.edit.data))
        assert any(insn.code in CALL_REPLACEMENT_OPCODES for insn in decoded)


def test_a_call_whose_result_is_immediately_overwritten_still_keeps_its_own_bytes() -> None:
    # The edge case none of the real fixtures happen to contain, built by
    # hand: a fresh, unrelated LOAD right after the call, in the same pair,
    # discarding the call's own value before anything reads it.
    # needed()'s ordinary "last value per pair" rule would then mark the
    # LOAD needed and not the call -- tail_widened_calls()'s own forced
    # need[0] is what stops that from silently deleting the call's bytes
    # while the edit still removes the original call from the object.
    call_bytes = bytes([0x9A, 0, 0, 0, 0])  # a far call, 16-bit: opcode+offset16+seg16
    load_bytes = bytes.fromhex("A1 10 00 8B 16 12 00".replace(" ", ""))
    code = call_bytes + load_bytes
    call_insn = decode(code, 0)
    assert call_insn is not None
    following, _ = decode_run(code, call_insn.end, len(code))

    found = Module(records=[], seg=1, name="T", code=code, start=0, end=len(code), chunks=((0, len(code)),))
    mapped = CodeMap(starts=frozenset({call_insn.at, call_insn.end, *(i.at for i in following)}), leaders=frozenset())
    # a self-loop successor keeps the block from "leaving" (which would make
    # every flag conservatively live and refuse the site for an unrelated
    # reason) without needing a second, real block
    block = Block(at=0, end=len(code), insns=(call_insn, *following), ends=Ends.FALLS_THROUGH, succ=(0,))
    live = live_in([block])

    operand = Operand(Kind.CONSTANT, value=5, length=2)
    site = CallSite(at=0, end=call_insn.end, start=0, name=MULTIPLY, pushed=(operand, operand))
    reg_live = regs.analyse([block])
    combined = tail_widened_calls(found, mapped, [block], live, reg_live, [site], [])

    assert combined.planned, "the fresh load still widens against the call's own seeded value"
    edit = combined.planned[0].edit
    assert edit is not None
    decoded, _ = decode_run(edit.data, 0, len(edit.data))
    assert any(insn.code in CALL_REPLACEMENT_OPCODES for insn in decoded), (
        "the multiply's own instruction is missing -- need[0] was not forced"
    )


def test_a_combined_edit_never_restores_only_to_immediately_rewiden(operator_obj: Path) -> None:
    # calls.absorb(..., restore=False) drops the trailing high-half restore
    # precisely because the widened tail is about to consume eax directly --
    # putting it back only to re-derive it a few bytes later is the round
    # trip docs/residue.md calls G and H. A restore may still appear, but
    # only as the combined edit's own trailing FIXUP[pair], never buried in
    # the middle of it.
    for one in _combined(operator_obj).planned:
        assert one.edit is not None
        data = one.edit.data
        trailing = data[-4:]
        middle = data[:-4] if trailing in FIXUP.values() else data
        assert FIXUP[0] not in middle
        assert FIXUP[1] not in middle
