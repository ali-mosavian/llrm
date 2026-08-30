"""
Whether dx or bx is still wanted -- the two registers lift.py's own restore
idiom (FIXUP) puts back after a widened region, and the two historical bugs
this analysis has to avoid: ax/dx treated as one joint unit instead of two
independent registers, and ir.py's own rooting (which folds every
sub-register to its 32-bit parent and, correctly for ITS purpose, counts a
partial write as also a use) answering a different question than "is dx's
own value still wanted".
"""

from iced_x86 import Register

from helpers import hx
from qbopt.declen import run
from qbopt.blocks import Ends
from qbopt.blocks import Block
from qbopt.registers import reads
from qbopt.registers import writes
from qbopt.registers import analyse
from qbopt.registers import live_in
from qbopt.registers import live_after


def a_block(at: int, enc: str, ends: Ends, succ: tuple[int, ...]) -> Block:
    code = bytes(at) + hx(enc)
    insns, _ = run(code, at, len(code))
    return Block(at, len(code), tuple(insns), ends, succ)


def test_pop_dx_writes_dx_and_does_not_read_it() -> None:
    # calls.py's own restore idiom's exact middle instruction, and the
    # historical bug ir.ROOT-based reasoning would repeat: a plain `pop dx`
    # is not a read of the old dx, even though rooting it to edx and
    # counting a partial write as a use (right for ir.py's own purpose) would
    # say otherwise.
    block = a_block(0, "5A", Ends.FALLS_THROUGH, ())  # pop dx
    assert writes(block, Register.DX)
    assert not reads(block, Register.DX)


def test_push_dx_reads_dx() -> None:
    block = a_block(0, "52", Ends.FALLS_THROUGH, ())  # push dx
    assert reads(block, Register.DX)


def test_push_eax_does_not_touch_dx_at_all() -> None:
    # eax and edx are different physical registers -- the restore idiom's own
    # `push eax` (calls.py's restoring(), lift.FIXUP[0]) is not a use of the
    # OLD dx at all; it is what later makes dx's NEW value available to
    # `pop dx`, via the stack, not via the register file. Worth asserting on
    # its own: a plausible-sounding but wrong claim ("push eax reads dx
    # through the group") is exactly the kind of error this module's own
    # historical bugs came from.
    block = a_block(0, "66 50", Ends.FALLS_THROUGH, ())  # push eax
    assert not reads(block, Register.DX)
    assert not writes(block, Register.DX)


def test_a_call_is_conservative_both_ways() -> None:
    block = a_block(0, "9A 00 00 00 00", Ends.FALLS_THROUGH, ())  # call far
    assert reads(block, Register.DX)
    assert writes(block, Register.DX)


def test_a_conditional_write_is_not_a_kill() -> None:
    # cmovz dx,ax -- a write that may not fire leaves dx exactly as it was,
    # so treating it as a kill could call a still-live restore dead.
    block = a_block(0, "0F 44 D0", Ends.FALLS_THROUGH, ())  # cmovz dx,ax
    assert not writes(block, Register.DX)


def test_a_full_width_write_kills_the_prior_value() -> None:
    block = a_block(0, "BA 34 12", Ends.FALLS_THROUGH, ())  # mov dx,1234h
    assert writes(block, Register.DX)


def test_dh_alone_does_not_fully_overwrite_dx() -> None:
    block = a_block(0, "B6 12", Ends.FALLS_THROUGH, ())  # mov dh,12h
    assert not writes(block, Register.DX)


def test_live_after_stops_at_the_first_read() -> None:
    # mov dx,1 / add ax,dx / mov dx,2 -- dx is read at offset 3, before its
    # own later write at offset 5 could make that moot.
    code = hx("BA 01 00 01 D0 BA 02 00")
    insns, _ = run(code, 0, len(code))
    block = Block(0, len(code), tuple(insns), Ends.RETURN, ())
    assert live_after(block, 3, Register.DX, {0: False}) is True


def test_live_after_is_false_once_overwritten_first() -> None:
    # mov dx,2 / mov dx,1 -- the first mov's own dx is dead: overwritten
    # before anything reads it, in a block that leaves right after.
    code = hx("BA 02 00 BA 01 00")
    insns, _ = run(code, 0, len(code))
    block = Block(0, len(code), tuple(insns), Ends.RETURN, ())
    assert live_after(block, 3, Register.DX, {0: False}) is False


def test_live_in_reaches_back_through_an_unwritten_block() -> None:
    read_dx = a_block(0x10, "01 D0", Ends.RETURN, ())  # add ax,dx
    before = a_block(0x00, "90", Ends.FALLS_THROUGH, (0x10,))
    live = live_in([before, read_dx], Register.DX)
    assert live[read_dx.at] is True
    assert live[before.at] is True


def test_live_in_stops_at_a_write() -> None:
    read_dx = a_block(0x10, "01 D0", Ends.RETURN, ())  # add ax,dx
    writes_first = a_block(0x00, "BA 05 00", Ends.FALLS_THROUGH, (0x10,))  # mov dx,5
    live = live_in([writes_first, read_dx], Register.DX)
    assert live[writes_first.at] is False


def test_a_block_that_leaves_keeps_dx_live() -> None:
    # A FUNCTION can return its answer in a register the same way it can in
    # the flags -- anything this cannot see the other side of has to assume
    # the worst, or a restore right before a ret is dropped and the caller
    # reads a register that was never put back.
    ret = a_block(0x10, "C3", Ends.RETURN, ())
    live = live_in([ret], Register.DX)
    assert live[ret.at] is True


def test_analyse_covers_both_registers() -> None:
    ret = a_block(0x10, "C3", Ends.RETURN, ())
    found = analyse([ret])
    assert found.dx[ret.at] is True
    assert found.bx[ret.at] is True
