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
            code = select.move(into, outof)
            assert code is not None
            decoded = next(iter(Decoder(BITNESS, code, ip=0)))
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
    assert len(wide) == 3 and len(narrow) == 2
    assert wide[0] == 0x66


def test_a_move_to_itself_is_no_instruction() -> None:
    assert select.move(Register.EAX, Register.EAX) == b""


def test_mixed_widths_are_refused_rather_than_guessed() -> None:
    """`mov ecx,ax` is not a move -- it is a zero or sign extension, and
    which one was meant is not something this can know."""
    assert select.move(Register.ECX, Register.AX) is None
    assert select.move(Register.AX, Register.ECX) is None


def test_registers_this_does_not_name_are_refused() -> None:
    """bp and the segment registers are where values live, not values."""
    assert select.move(Register.EBP, Register.EAX) is None
    assert select.move(Register.EAX, Register.ES) is None
    assert select.move(Register.ESP, Register.EAX) is None


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
        back = next(iter(Decoder(BITNESS, made, ip=op.at)), None)
        assert back is not None, f"{obj.stem} {op.at:#x}: emitted bytes do not decode"
        assert str(back) == str(op.node.insn.insn), f"{obj.stem} {op.at:#x}: {back} != {op.node.insn.insn}"


def test_the_covered_share_of_the_corpus_is_what_was_measured() -> None:
    """A canary on progress, not on correctness.

    28.8% of the corpus's operations, which is the register and immediate
    forms. The rest is memory operands, calls and branches -- an address and
    a target both have to survive layout, which is the other half of M1.
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
    assert (total, emitted) == (20245, 5839)


def test_a_wide_push_is_not_a_narrow_one() -> None:
    """`push 3` puts two bytes on the stack and `pushd 3` puts four.

    A caller that pops a dword after the narrow one reads two bytes of
    whatever was under it. The first version of push_imm always emitted
    PUSH_IMM16 and got this wrong at 24 sites.
    """
    narrow, wide = select.push_imm(3, 2), select.push_imm(3, 4)
    assert narrow is not None and wide is not None
    assert len(wide) == len(narrow) + 3  # the 0x66 prefix and two more bytes
    assert wide[0] == 0x66


def test_a_register_is_not_an_immediate() -> None:
    """iced's Register_ IS an int -- Register.EAX is the number 37 -- so a
    single push() taking either emitted `push 25h` where it meant `push eax`.
    Two functions, and this is what keeps them two."""
    assert select.push(Register.EAX) == bytes([0x66, 0x50])
    assert select.push_imm(int(Register.EAX)) != select.push(Register.EAX)


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
