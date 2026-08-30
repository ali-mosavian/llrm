"""
The rewriter over the committed objects. No emulator, no compilers.

These are the invariants that stand between a change to the emitter and a
program that links, runs, and quietly computes the wrong thing.
"""

from pathlib import Path

import pytest
from iced_x86 import Code

from qbopt import omf
from qbopt import module
from qbopt.calls import Kind
from qbopt.lift import FIXUP
from qbopt.blocks import Ends
from qbopt.calls import sites
from qbopt.blocks import Block
from qbopt.calls import Operand
from qbopt.declen import decode
from qbopt.flags import live_in
from qbopt.module import Module
from qbopt.blocks import CodeMap
from qbopt.calls import CallSite
from qbopt.calls import MULTIPLY
from qbopt.blocks import code_map
from qbopt.rewrite import rewrite
from qbopt.blocks import partition
from qbopt.rewrite import Combined
from qbopt.blocks import instructions
from qbopt.declen import run as decode_run
from qbopt.rewrite import tail_widened_calls

pytestmark = pytest.mark.corpus

# The only opcodes calls.absorb() ever replaces a non-COMPARE call with --
# tail_widened_calls() only ever folds one of these four names in, so a
# combined edit's own bytes must decode to at least one of them, or the
# call's own arithmetic never actually happened.
CALL_REPLACEMENT_OPCODES = {
    Code.IMUL_R32_RM32,
    Code.IMUL_R32_RM32_IMM8,
    Code.IMUL_R32_RM32_IMM32,
    Code.IMUL_RM32,
    Code.IDIV_RM32,
    Code.SHRD_RM32_R32_CL,
    Code.SHRD_RM32_R32_IMM8,
}


def _combined(obj: Path) -> Combined:
    found = module.of(omf.parse(obj.read_bytes()))
    assert found is not None
    mapped = code_map(found)
    assert not isinstance(mapped, str)
    blocks = partition(found, mapped)
    live = live_in(blocks)
    reached = instructions(found)
    assert not isinstance(reached, str)
    return tail_widened_calls(found, mapped, blocks, live, sites(found, reached, blocks), [])


def test_a_dry_run_writes_the_input_back_unchanged(obj: Path) -> None:
    data = obj.read_bytes()
    out, _ = rewrite(data, dry_run=True)
    assert out == data


def test_a_dry_run_takes_no_region(obj: Path) -> None:
    _, found = rewrite(obj.read_bytes(), dry_run=True)
    assert [r for r in found if r.taken] == []


def test_a_real_pass_rewrites_the_code_and_keeps_the_records_readable(obj: Path) -> None:
    data = obj.read_bytes()
    out, found = rewrite(data, dry_run=False)
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


def test_rewriting_the_output_finds_nothing_new(obj: Path) -> None:
    out, _ = rewrite(obj.read_bytes(), dry_run=False)
    _, again = rewrite(out, dry_run=False)
    assert [r for r in again if r.taken] == []


def test_a_region_is_either_taken_or_says_why_not(obj: Path) -> None:
    _, found = rewrite(obj.read_bytes(), dry_run=False)
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
        _, found = rewrite(path.read_bytes(), dry_run=False)
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
    combined = tail_widened_calls(found, mapped, [block], live, [site], [])

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
