"""
qbopt/select.py: the first instructions this pass writes rather than moves.

Everything before it rearranged bytes BC had already emitted. So the tests
are about refusal as much as output -- an emitter that guesses at a form it
does not know produces a working program that computes something else.
"""

from pathlib import Path
from collections.abc import Iterator

import pytest
from iced_x86 import Code
from iced_x86 import Decoder
from iced_x86 import Register

import corpus
from qbopt import select
from qbopt.declen import BITNESS

ROOTS = (Register.EAX, Register.ECX, Register.EDX, Register.EBX, Register.ESI, Register.EDI)
HALVES = (Register.AX, Register.CX, Register.DX, Register.BX, Register.SI, Register.DI)


def test_a_move_decodes_back_to_the_move_that_was_asked_for() -> None:
    """Round-tripped through the decoder rather than compared to a literal:
    a hand-written expectation can be wrong in the same direction twice."""
    for into in ROOTS:
        for outof in ROOTS:
            if into is outof:
                continue
            built = select.move(into, outof)
            assert built is not None
            decoded = next(iter(Decoder(BITNESS, built.code, ip=0)))
            assert decoded.code == Code.MOV_R32_RM32
            assert decoded.op0_register == into
            assert decoded.op1_register == outof


def test_a_sixteen_bit_move_is_shorter_than_a_thirty_two_bit_one() -> None:
    """This is 16-bit code, so a 32-bit operand carries the 0x66 prefix.

    Worth a test because it is the fact that makes this emitter's output
    able to be LONGER than what it replaces -- three bytes against the two
    pushes it stands in for -- and a caller that assumed otherwise would
    silently overrun.
    """
    wide = select.move(Register.ECX, Register.EAX)
    narrow = select.move(Register.CX, Register.AX)
    assert wide is not None and narrow is not None
    assert len(wide.code) == 3 and len(narrow.code) == 2
    assert wide.code[0] == 0x66


def test_a_move_to_itself_is_no_instruction() -> None:
    made = select.move(Register.EAX, Register.EAX)
    assert made is not None and made.code == b""


def test_mixed_widths_are_refused_rather_than_guessed() -> None:
    """`mov ecx,ax` is not a move -- it is a zero or sign extension, and
    which one was meant is not something this can know."""
    assert select.move(Register.ECX, Register.AX) is None
    assert select.move(Register.AX, Register.ECX) is None


def test_registers_this_does_not_name_are_refused() -> None:
    """A segment register is not one this names.

    bp and sp are, and have to be: a procedure opens `push bp / mov bp,sp`
    and closes by popping it back, so a selector that could not say them
    could not emit a prologue. They are still not values -- mir.PHYSICAL
    keeps them out -- which is a different question from whether an
    instruction can name them.
    """
    assert select.move(Register.EAX, Register.ES) is None
    assert select.move(Register.ES, Register.EAX) is None
    assert select.move(Register.EBP, Register.ESP) is not None


# --- M1: selection over the corpus -------------------------------------------

FIXTURES = sorted(Path("fixtures/omf").glob("*.obj"))


def selected(obj: Path) -> Iterator[tuple]:
    """Every op in the object the selector will emit, with what it emitted."""
    from qbopt import ir
    from qbopt import mir
    from qbopt import blocks as split
    from qbopt.rewrite import code_map

    found = corpus.loaded(obj)
    if found is None:
        return
    mapped = code_map(found)
    if isinstance(mapped, str):
        return
    for _, body in mir.bodies(found, split.partition(found, mapped)):
        for block in body.blocks:
            for op in block.ops:
                what = getattr(op.node, "semantics", None)
                if what is None or what.op is ir.Operation.BARRIER:
                    continue
                made = select.emit(what, at=op.at)
                if made is not None:
                    yield op, made


@pytest.mark.parametrize("obj", FIXTURES, ids=lambda p: p.stem)
def test_everything_selected_decodes_to_what_was_asked_for(obj: Path) -> None:
    """M1's gate, and the only one that means anything for a selector.

    Not "the same bytes" -- selection is allowed to choose an encoding BC did
    not. The claim is that what comes back is the same instruction: same
    mnemonic, same operands, same widths. Measured over the corpus: 5,839
    emitted, none different.
    """
    for op, made in selected(obj):
        back = next(iter(Decoder(BITNESS, made.code, ip=op.at)), None)
        assert back is not None, f"{obj.stem} {op.at:#x}: emitted bytes do not decode"
        want = op.node.insn.insn
        # A branch may come back in a different form -- BC writes `e9 0b 00`
        # where `eb 0c` reaches, and choosing between them is layout's call,
        # not selection's. Same mnemonic and same target is the claim.
        same = str(back) == str(want) or (back.mnemonic == want.mnemonic and back.near_branch16 == want.near_branch16)
        assert same, f"{obj.stem} {op.at:#x}: {back} != {want}"


def test_the_covered_share_of_the_corpus_is_what_was_measured() -> None:
    """A canary on progress, not on correctness.

    99.7% of the corpus's operations. What is left is about two hundred
    whose address is in a space operand_of() refuses, whose register is not
    one this names, or -- 16 of them -- an `escape`, which is a far jump
    this has no target for.
    """
    from qbopt import ir
    from qbopt import mir
    from qbopt import blocks as split
    from qbopt.rewrite import code_map

    total = emitted = 0
    for obj in FIXTURES:
        found = corpus.loaded(obj)
        if found is None:
            continue
        mapped = code_map(found)
        if isinstance(mapped, str):
            continue
        for _, body in mir.bodies(found, split.partition(found, mapped)):
            for block in body.blocks:
                for op in block.ops:
                    total += 1
                    what = getattr(op.node, "semantics", None)
                    if what is None or what.op is ir.Operation.BARRIER:
                        continue
                    if select.emit(what, at=op.at) is not None:
                        emitted += 1
    assert (total, emitted) == (20245, 20185)


def test_a_wide_push_is_not_a_narrow_one() -> None:
    """`push 3` puts two bytes on the stack and `pushd 3` puts four.

    A caller that pops a dword after the narrow one reads two bytes of
    whatever was under it. The first version of push_imm always emitted
    PUSH_IMM16 and got this wrong at 24 sites.
    """
    narrow, wide = select.push_imm(3, 2), select.push_imm(3, 4)
    assert narrow is not None and wide is not None
    # Asked of what reaches the stack rather than of the encoding's length:
    # PUSHD_IMM8 pushes four bytes from a one-byte immediate, so the wide
    # push is not always the longer instruction.
    assert next(iter(Decoder(BITNESS, narrow.code, ip=0))).stack_pointer_increment == -2
    assert next(iter(Decoder(BITNESS, wide.code, ip=0))).stack_pointer_increment == -4


def test_a_register_is_not_an_immediate() -> None:
    """iced's Register_ IS an int -- Register.EAX is the number 37 -- so a
    single push() taking either emitted `push 25h` where it meant `push eax`.
    Two functions, and this is what keeps them two."""
    one = select.push(Register.EAX)
    assert one is not None and one.code == bytes([0x66, 0x50])
    other = select.push_imm(int(Register.EAX))
    assert other is not None and other.code != one.code


def test_a_mixed_width_operation_is_refused() -> None:
    """`add ax,ecx` is not an instruction; guessing which width was meant is
    exactly what this module does not do."""
    assert select.arith("add", Register.AX, Register.ECX) is None
    assert select.arith("add", Register.EAX, Register.CX) is None


def test_an_operation_not_in_the_table_is_refused() -> None:
    for name in ("rol", "shl", "imul", "xchg", ""):
        assert select.arith(name, Register.AX, Register.CX) is None
    for name in ("bswap", "shr", ""):
        assert select.unary(name, Register.AX) is None


def test_a_relocated_address_is_emitted_as_zero_and_says_where() -> None:
    """LINK adds what is in the code to the fixup's target.

    So a relocated displacement has to go out as zero -- anything else is
    added to the real address -- and the caller has to be told where it
    landed so the fixup moves with it. Both halves, or the field reads a
    bare zero at run time.
    """
    from qbopt import ir
    from qbopt.module import Addr
    from qbopt.module import Space

    cell = ir.Mem(Addr(Space.SEGMENT, 0x1234, 5), 2)
    made = select.move_from(Register.AX, cell)
    assert made is not None
    assert made.displacement_at is not None
    at = made.displacement_at
    assert made.code[at : at + 2] == b"\x00\x00", "a relocated displacement is not a number"


def test_a_frame_slot_keeps_its_displacement() -> None:
    """bp-relative is the other way round: the number really is in the code
    and no fixup names it, so there is nothing to move."""
    from qbopt import ir
    from qbopt.module import Addr
    from qbopt.module import Space

    cell = ir.Mem(Addr(Space.FRAME, -0x18), 2)
    made = select.move_from(Register.AX, cell)
    assert made is not None
    assert str(next(iter(Decoder(BITNESS, made.code, ip=0)))) == "mov ax,[bp-18h]"
    # It has a displacement and the bytes are the number, where a relocated
    # one is emitted as zero. Whether a fixup names it is the module's to
    # say; Emitted only reports where the field is.
    at = made.displacement_at
    assert at is not None
    # One byte, not two: -0x18 fits a signed byte and BC writes it that way
    # too. A relocated displacement is always two, because a fixup patches
    # a word -- which is the other half of what this pair of tests says.
    assert made.code[at:] == (-0x18 & 0xFF).to_bytes(1, "little")


def test_the_spaces_that_cannot_be_encoded_are_refused() -> None:
    """A FAR address needs a segment override this does not model, a GROUP
    one is refused everywhere in this project, and a STACK one is mir.py's
    name for a push slot rather than anything an instruction encodes.

    Space.LITERAL is not among them: a displacement no fixup claims is a
    real address in the code, which encodes like any other and needs
    nothing moved with it.
    """
    from qbopt import ir
    from qbopt.module import Addr
    from qbopt.module import Space

    for space in (Space.FAR, Space.GROUP, Space.STACK):
        assert select.operand_of(ir.Mem(Addr(space, 4), 2)) is None, space
    assert select.operand_of(ir.Mem(None, 2)) is None
    literal = select.operand_of(ir.Mem(Addr(Space.LITERAL, 4), 2))
    assert literal is not None and literal[1] is False, "a literal address relocates nothing"


def test_a_cell_of_the_wrong_width_is_refused() -> None:
    """`mov ax,[dword x]` is not an instruction."""
    from qbopt import ir
    from qbopt.module import Addr
    from qbopt.module import Space

    cell = ir.Mem(Addr(Space.FRAME, -4), 4)
    assert select.move_from(Register.AX, cell) is None
    assert select.move_from(Register.EAX, cell) is not None


def test_a_near_call_and_a_far_call_are_told_apart_by_the_target() -> None:
    """BC emits a near call for a procedure in the same module and a far one
    for everything in the runtime. `ir.Semantics.target` is the difference: a
    near call names an address, a far one does not have it to name -- its
    four bytes are zero and a fixup fills them.

    Treating every call as far emitted `call far ptr 0:0` where the input
    said `call 0032h`, at every intra-module call site in the corpus.
    """
    near = select.call_near(0x32, at=0x4E)
    far = select.call_far(at=0x4E)
    assert near is not None and far is not None
    assert near.displacement_at is None
    assert far.displacement_at == 1
    assert far.code == bytes([0x9A, 0, 0, 0, 0])
    assert str(next(iter(Decoder(BITNESS, near.code, ip=0x4E)))) == "call 0032h"


def test_a_store_of_an_immediate_takes_the_cell_s_width() -> None:
    """`mov word ptr [x],0` writes two bytes and `mov dword ptr [x],0` four.
    The immediate says nothing about which was meant."""
    from qbopt import ir
    from qbopt.module import Addr
    from qbopt.module import Space

    for width, text in ((2, "word"), (4, "dword")):
        made = select.store_imm(ir.Mem(Addr(Space.FRAME, -4), width), 0)
        assert made is not None
        assert text in str(next(iter(Decoder(BITNESS, made.code, ip=0))))
