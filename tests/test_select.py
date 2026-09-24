"""
qbopt/backend/select.py: the first instructions this pass writes rather than moves.

Everything before it rearranged bytes BC had already emitted. So the tests
are about refusal as much as output -- an emitter that guesses at a form it
does not know produces a working program that computes something else.
"""

from pathlib import Path
from collections.abc import Callable
from collections.abc import Iterator

import pytest
from iced_x86 import Code
from iced_x86 import Decoder
from iced_x86 import Register
from iced_x86 import Register_

import corpus
from qbopt.model import ir
from qbopt.backend import select


@pytest.mark.parametrize("register,width", [(Register.AL, 1), (Register.AX, 2), (Register.EAX, 4)])
@pytest.mark.parametrize("memory_first", [False, True])
def test_pl_move_exchange_with_frame_memory(register, width, memory_first):
    """PL_MOVE refused at 2f4b: XCHG AX,[BP-1Ch] lacked a memory encoding."""
    from qbopt.objectfile.module import Addr
    from qbopt.objectfile.module import Space

    cell = ir.Mem(Addr(Space.FRAME, -28), width)
    held = ir.Reg(register, width)
    dests = (cell, held) if memory_first else (held, cell)
    what = ir.Semantics(ir.Operation.EXCHANGE, "xchg", dests, tuple(reversed(dests)))
    emitted = select.emit(what)
    assert emitted is not None
    instruction = next(iter(Decoder(16, emitted.code)))
    assert instruction.code == getattr(Code, f"XCHG_RM{width * 8}_R{width * 8}")
    assert instruction.memory_base == Register.BP
    assert instruction.memory_displacement & 0xFFFF == 0xFFE4
    assert instruction.op1_register == register


@pytest.mark.parametrize("source", [Register.CL, Register.AH, Register.BL, Register.DH])
def test_byte_copy_for_nbody_timer(source):
    """NBODY's PIT writer refused at 0444: allocation required MOV AL,CL before OUT."""
    what = ir.Semantics(ir.Operation.MOVE, "mov", (ir.Reg(Register.AL, 1),), (ir.Reg(source, 1),))
    emitted = select.emit(what)
    assert emitted is not None
    instruction = next(iter(Decoder(16, emitted.code)))
    assert instruction.code == Code.MOV_R8_RM8
    assert instruction.op0_register == Register.AL
    assert instruction.op1_register == source


def test_signed_word_extension_uses_explicit_operands():
    """ADDRM's signed whole store needs MOVSX, not a width-mismatched MOV or CWD's fixed pair."""
    from qbopt.backend import target

    what = ir.Semantics(ir.Operation.EXTEND, "movsx", (ir.Reg(Register.EBX, 4),), (ir.Reg(Register.SI, 2),))
    emitted = select.emit(what)
    assert emitted is not None
    instruction = next(iter(Decoder(16, emitted.code)))
    assert instruction.code == Code.MOVSX_R32_RM16
    assert instruction.op0_register == Register.EBX and instruction.op1_register == Register.SI
    assert not target.requirements(what)


def test_load_accepts_unsigned_dword_bit_pattern():
    """CHAIN refused a folded 0xbffffff9 because iced's i32 constructor requires a signed integer."""
    from iced_x86 import Register

    emitted = select.load(Register.ESI, 0xBFFFFFF9)
    assert emitted is not None
    assert emitted.code == bytes.fromhex("66bef9ffffbf")


@pytest.mark.parametrize("value", [0x80000000, 0xEDCBA987, 0xFFFFFFFF])
def test_push_accepts_unsigned_dword_bit_patterns(value):
    """NOTS refused constant arguments with the sign bit set at the encoder's signed-i32 boundary."""
    emitted = select.push_imm(value, 4)
    assert emitted is not None
    instruction = next(iter(Decoder(16, emitted.code)))
    assert instruction.stack_pointer_increment == -4
    assert instruction.immediate(0) & 0xFFFFFFFF == value


from qbopt.frontend.declen import BITNESS


def test_store_accepts_unsigned_dword_bit_pattern():
    """CHAIN refused its whole 0xc1747c23 initializer at 0x48 instead of emitting it."""
    from qbopt.objectfile.module import Addr
    from qbopt.objectfile.module import Space

    cell = ir.Mem(Addr(Space.FRAME, -4), 4)
    emitted = select.store_imm(cell, 0xC1747C23)
    assert emitted is not None
    assert emitted.code == bytes.fromhex("66c746fc237c74c1")


ROOTS = (Register.EAX, Register.ECX, Register.EDX, Register.EBX, Register.ESI, Register.EDI)
HALVES = (Register.AX, Register.CX, Register.DX, Register.BX, Register.SI, Register.DI)


def _made(what: select.Emitted | None) -> select.Emitted:
    """The emitter returns None where it cannot encode, so a test that reads
    `.code` has to say which it expected."""
    assert what is not None
    return what


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
    from qbopt.model import ir
    from qbopt.model import mir
    from qbopt.frontend import blocks as split
    from qbopt.frontend.blocks import code_map

    found = corpus.loaded(obj)
    if found is None:
        return
    mapped = code_map(found)
    if isinstance(mapped, str):
        return
    raised = mir.bodies(found, split.partition(found, mapped))
    for _, body in raised:
        for block in body.blocks:
            for op in block.ops:
                node = raised.source.nodes.get(op.id)
                what = getattr(node, "semantics", None)
                if what is None or what.op is ir.Operation.BARRIER:
                    continue
                made = select.emit(what, at=op.at)
                if made is not None:
                    yield op, node, made


@pytest.mark.parametrize("obj", FIXTURES, ids=lambda p: p.stem)
def test_everything_selected_decodes_to_what_was_asked_for(obj: Path) -> None:
    """M1's gate, and the only one that means anything for a selector.

    Not "the same bytes" -- selection is allowed to choose an encoding BC did
    not. The claim is that what comes back is the same instruction: same
    mnemonic, same operands, same widths. Measured over the corpus: 5,839
    emitted, none different.
    """
    for op, node, made in selected(obj):
        back = next(iter(Decoder(BITNESS, made.code, ip=op.at)), None)
        assert back is not None, f"{obj.stem} {op.at:#x}: emitted bytes do not decode"
        want = node.insn.insn
        # A branch may come back in a different form -- BC writes `e9 0b 00`
        # where `eb 0c` reaches, and choosing between them is layout's call,
        # not selection's. Same mnemonic and same target is the claim.
        same = str(back) == str(want) or (back.mnemonic == want.mnemonic and back.near_branch16 == want.near_branch16)
        assert same, f"{obj.stem} {op.at:#x}: {back} != {want}"


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
    from qbopt.model import ir
    from qbopt.objectfile.module import Addr
    from qbopt.objectfile.module import Space

    cell = ir.Mem(Addr(Space.SEGMENT, 0x1234, 5), 2)
    made = select.move_from(Register.AX, cell)
    assert made is not None
    assert made.displacement_at is not None
    at = made.displacement_at
    assert made.code[at : at + 2] == b"\x00\x00", "a relocated displacement is not a number"


def test_a_frame_slot_keeps_its_displacement() -> None:
    """bp-relative is the other way round: the number really is in the code
    and no fixup names it, so there is nothing to move."""
    from qbopt.model import ir
    from qbopt.objectfile.module import Addr
    from qbopt.objectfile.module import Space

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
    from qbopt.model import ir
    from qbopt.objectfile.module import Addr
    from qbopt.objectfile.module import Space

    for space in (Space.FAR, Space.GROUP, Space.STACK):
        assert select.operand_of(ir.Mem(Addr(space, 4), 2)) is None, space
    assert select.operand_of(ir.Mem(None, 2)) is None
    literal = select.operand_of(ir.Mem(Addr(Space.LITERAL, 4), 2))
    assert literal is not None and literal[1] is False, "a literal address relocates nothing"


def test_a_cell_of_the_wrong_width_is_refused() -> None:
    """`mov ax,[dword x]` is not an instruction."""
    from qbopt.model import ir
    from qbopt.objectfile.module import Addr
    from qbopt.objectfile.module import Space

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
    from qbopt.model import ir
    from qbopt.objectfile.module import Addr
    from qbopt.objectfile.module import Space

    for width, text in ((2, "word"), (4, "dword")):
        made = select.store_imm(ir.Mem(Addr(Space.FRAME, -4), width), 0)
        assert made is not None
        assert text in str(next(iter(Decoder(BITNESS, made.code, ip=0))))


@pytest.mark.parametrize(
    ("value", "want"),
    [(0, "6a00"), (1, "6a01"), (0x31, "6a31"), (-1, "6aff"), (127, "6a7f"), (-128, "6a80")],
)
def test_a_narrow_push_of_a_small_literal_takes_the_byte_form(value: int, want: str) -> None:
    """`push 0` is `6a 00`, not `68 00 00`.

    PUSHW_IMM8 pushes two bytes from a one-byte sign-extended immediate,
    which is what BC emits and what this used to say did not exist -- the
    comment read "there is no PUSH_IMM8 that pushes two, which is why only
    the wide one shrinks". iced has had PUSHW_IMM8 all along.

    A byte a site, and qb-qrender has 513 of them. Nothing in fixtures/omf
    made it visible: the suite's pushes are addresses and long literals, so
    the corpus came out three bytes *shorter* whole-segment while a real
    program came out 2,776 longer.
    """
    made = select.push_imm(value, 2)
    assert made is not None
    assert made.code.hex() == want


@pytest.mark.parametrize("value", [128, -129, 1000, -1000, 0x7FFF, -0x8000])
def test_a_narrow_push_that_does_not_fit_a_byte_stays_wide(value: int) -> None:
    """The sign-extension has to be a fact, not a hope: 128 sign-extends to
    -128, so anything outside the signed byte range keeps PUSH_IMM16."""
    made = select.push_imm(value, 2)
    assert made is not None
    assert made.code[0] == 0x68, f"{value} took the byte form and does not fit one"
    assert len(made.code) == 3


@pytest.mark.parametrize(
    ("made", "want"),
    [
        (lambda: select.push_imm(0xFFFF, 2), "6aff"),
        (lambda: select.push_imm(0xFF80, 2), "6a80"),
        (lambda: select.arith_imm("add", Register.BX, 0xFFFF), "83c3ff"),
    ],
)
def test_a_word_immediate_written_unsigned_still_fits_a_byte(
    made: Callable[[], select.Emitted | None], want: str
) -> None:
    """`push 65535` was `68 ff ff` where jwasm writes `6a ff`: the C path hands
    a word's -1 over unsigned, and only a dword was read back as signed."""
    assert _made(made()).code.hex() == want


def test_a_literal_address_through_a_register_uses_the_byte_displacement() -> None:
    """`add bx,[si+0Ah]` is `03 5c 0a`, not `03 9c 0a 00`.

    Space.LITERAL hardcoded a two-byte displacement. That is right for a
    bare direct address and a byte too many everywhere BC reaches one
    through a register, which is how it writes a field of a record: 1,247
    `add` sites and 785 `mov` sites in qb-qrender, a byte each.
    """
    from qbopt.objectfile.module import Addr
    from qbopt.objectfile.module import Space

    cell = ir.Mem(addr=Addr(Space.LITERAL, 0x0A, 0, base=Register.SI), width=2, through=Register.SI, offset=0x0A)
    made = select.arith_mem("add", Register.BX, cell)
    assert made is not None
    assert made.code.hex() == "035c0a"


def test_a_bare_literal_address_keeps_two_bytes_however_small_it_is() -> None:
    """Without a base there is nowhere for a byte displacement to go.

    16-bit mod=00 r/m=110 is the direct-address form and it carries a word;
    the mod=01 encoding that would hold a byte means `[bp+disp8]`, which is
    a different address entirely. So the size is the encoding's, not the
    value's.
    """
    from qbopt.objectfile.module import Addr
    from qbopt.objectfile.module import Space

    for value in (0, 1, 0x10, 0x7F):
        cell = ir.Mem(addr=Addr(Space.LITERAL, value, 0), width=2)
        made = select.arith_mem("add", Register.BX, cell)
        assert made is not None, value
        assert len(made.code) == 4, f"{value:#x} came back {made.code.hex()}"
        assert made.code[1] & 0xC7 == 0x06, f"{value:#x} is not the direct-address form"


def test_a_relocated_push_keeps_the_wide_immediate() -> None:
    """`push offset X` arrives here as Imm(value=0) and must stay `68 00 00`.

    BC writes the address as zero and lets LINK fill it in, so the operand
    is indistinguishable from a real `push 0` -- every `push 0` in
    fixtures/omf is one of these. Shrinking it to `6a 00` leaves the
    module's two-byte relocation pointing at a one-byte field, which
    test_every_relocation_points_at_a_field_the_module_really_has caught
    across 100 objects the moment the byte form was wired.
    """
    assert _made(select.push_imm(0, 2, relocated=True)).code.hex() == "680000"
    assert _made(select.push_imm(0, 2, relocated=False)).code.hex() == "6a00"
    # and the wide push shrinks on the same rule
    assert _made(select.push_imm(3, 4, relocated=True)).code.hex() == "666803000000"
    assert _made(select.push_imm(3, 4, relocated=False)).code.hex() == "666a03"


@pytest.mark.parametrize(
    ("name", "reg", "want"),
    [("shl", Register.AX, "d1e0"), ("shl", Register.BX, "d1e3"), ("sar", Register.AX, "d1f8")],
)
def test_a_shift_by_one_takes_its_own_opcode(name: str, reg: Register_, want: str) -> None:
    """`shl ax,1` is `d1 e0`, a byte shorter than `c1 e0 01`.

    The by-1 shape was already in the table and never fired: iced models the
    implicit 1 as a real operand, so create_reg builds `shl ax,???` and the
    assembler refuses it. Built with the count, it encodes. 101 sites in
    qb-qrender, all of them strength reduction's own doubling.
    """
    made = select.shift(name, reg, 1)
    assert made is not None
    assert made.code.hex() == want


def test_a_shift_by_more_than_one_keeps_the_immediate_form() -> None:
    made = select.shift("shl", Register.AX, 3)
    assert made is not None
    assert made.code.hex() == "c1e003"


@pytest.mark.parametrize(("count", "hex_bytes"), [(1, "d166de"), (3, "c166de03"), (None, "d366de")])
def test_spilled_shift_is_encodable(count, hex_bytes) -> None:
    """Nbody refused a hoisted index spilled to [bp-22h] because memory SHL was missing."""
    from qbopt.objectfile.module import Addr
    from qbopt.objectfile.module import Space

    cell = ir.Mem(Addr(Space.FRAME, -0x22), 2)
    source = ir.Reg(Register.CL, 1) if count is None else ir.Imm(count, 1)
    made = select.emit(ir.Semantics(ir.Operation.BINARY, "shl", (cell,), (cell, source)))
    assert made is not None
    assert made.code.hex() == hex_bytes


@pytest.mark.parametrize(
    ("name", "value", "want"),
    [("add", 0x1286, "058612"), ("cmp", 0x1234, "3d3412"), ("sub", 0x4000, "2d0040")],
)
def test_an_accumulator_immediate_takes_the_short_opcode(name: str, value: int, want: str) -> None:
    """`add ax,1286h` is `05 86 12`, not `81 c0 86 12`.

    The accumulator has its own opcode for every arithmetic immediate, one
    byte shorter because it needs no modrm. Only for ax, and only where a
    byte immediate does not already fit -- `add ax,3` stays `83 c0 03`,
    which is shorter still. 592 sites in qb-qrender.
    """
    made = select.arith_imm(name, Register.AX, value)
    assert made is not None
    assert made.code.hex() == want


def test_a_byte_immediate_still_beats_the_accumulator_form() -> None:
    """`83 c0 03` is three bytes and `05 03 00` is three too, but the byte
    form is the one that also works on bx -- so the order stays: byte
    immediate, then accumulator, then the full word."""
    assert _made(select.arith_imm("add", Register.AX, 3)).code.hex() == "83c003"
    assert _made(select.arith_imm("add", Register.BX, 3)).code.hex() == "83c303"
    assert _made(select.arith_imm("add", Register.BX, 0x1286)).code.hex() == "81c38612"


@pytest.mark.parametrize(("value", "want"), [(0x32, "837ee232"), (0, "837ee200"), (-1, "837ee2ff")])
def test_a_compare_of_memory_against_a_small_literal_takes_the_byte_form(value: int, want: str) -> None:
    """`cmp word ptr [bp-1Eh],32h` is `83 7e e2 32`, not `81 7e e2 32 00`.

    arith_into_imm has tried the byte form since it was written; compare's
    own memory arm hardcoded the word one, so every `cmp` against a frame
    slot paid a byte. 209 sites in qb-qrender, and the fix is to stop having
    two of these.
    """
    from qbopt.objectfile.module import Addr
    from qbopt.objectfile.module import Space

    cell = ir.Mem(addr=Addr(Space.FRAME, -0x1E, 0), width=2)
    made = select.compare(cell, value)
    assert made is not None
    assert made.code.hex() == want


@pytest.mark.parametrize(("value", "want"), [(0, "807efc00"), (200, "807efcc8"), (-1, "807efcff")])
def test_a_compare_of_a_byte_cell_against_a_literal(value: int, want: str) -> None:
    """`cmp byte ptr [bp-4],0` came back None: only word and long cells were
    admitted, so sieve's flag test stayed `movzx dx,[m]; or dx,dx`."""
    from qbopt.objectfile.module import Addr
    from qbopt.objectfile.module import Space

    made = select.compare(ir.Mem(addr=Addr(Space.FRAME, -4, 0), width=1), value)
    assert made is not None
    assert made.code.hex() == want


@pytest.mark.parametrize(
    ("name", "dest", "source", "want"),
    [
        ("movzx", Register.BX, 1, "0fb65efc"),
        ("movsx", Register.AX, 1, "0fbe46fc"),
        ("movzx", Register.EBX, 2, "660fb75efc"),
        ("movzx", Register.EBX, Register.BX, "660fb7db"),
    ],
)
def test_an_extension_encodes_from_a_byte_or_a_cell(name: str, dest, source, want: str) -> None:
    """Only `movzx r32,r16` encoded, so liveness read `movzx bx,byte ptr [bp-4]` as
    touching every lane and the flags before sieve's flag test looked live."""
    from qbopt.backend import target
    from qbopt.objectfile.module import Addr
    from qbopt.objectfile.module import Space

    if source in (1, 2):
        operand = ir.Mem(Addr(Space.FRAME, -4, 0), source, Register.BP, 0, 2)
    else:
        operand = ir.Reg(source, 2)
    made = select.emit(ir.Semantics(ir.Operation.EXTEND, name, (ir.Reg(dest, target.WIDTHS[dest]),), (operand,)))
    assert made is not None
    assert made.code.hex() == want


def test_a_compare_of_memory_against_a_large_literal_stays_wide() -> None:
    from qbopt.objectfile.module import Addr
    from qbopt.objectfile.module import Space

    cell = ir.Mem(addr=Addr(Space.FRAME, -0x1E, 0), width=2)
    made = select.compare(cell, 0x1234)
    assert made is not None
    assert made.code.hex() == "817ee23412"


@pytest.mark.parametrize("name", ["add", "sub", "cmp", "and", "or", "xor"])
def test_a_relocated_arithmetic_immediate_keeps_its_width(name: str) -> None:
    """`add ax,offset X` arrives as `add ax,0` and must stay four bytes.

    BC writes it `81 c0 00 00` with a fixup on the two-byte immediate, and
    the linker fills the address in. Shrinking it to the sign-extended byte
    form leaves that relocation naming a one-byte field: the linker patches
    two bytes anyway, over the immediate and whatever follows it.

    This is what made a generated program read 0 for an array element. The
    instruction stream disassembled identically -- both sides show
    `add ax,0` before linking -- so nothing that compared the emitted code
    could see it. Only the linked image showed `add ax,0DCh` against
    `add ax,0FFDCh`.

    push_imm has had the same guard since the corpus caught it there; this
    is the other place an immediate can shrink.
    """
    wide = select.arith_imm(name, Register.AX, 0, relocated=True)
    assert wide is not None
    assert len(wide.code) == 3, f"{name} ax,0 relocated came back {wide.code.hex()}"
    assert wide.immediate_at is not None
    # the accumulator form is fine: its immediate is still a full word
    assert wide.code[0] != 0x83
    narrow = select.arith_imm(name, Register.AX, 0, relocated=False)
    assert narrow is not None and narrow.code[0] == 0x83


def test_a_relocated_immediate_against_memory_keeps_its_width() -> None:
    """The same for `add word ptr [bp-4],offset X`."""
    from qbopt.objectfile.module import Addr
    from qbopt.objectfile.module import Space

    cell = ir.Mem(addr=Addr(Space.FRAME, -4, 0), width=2)
    wide = select.arith_into_imm("add", cell, 0, relocated=True)
    assert wide is not None
    assert wide.code[0] == 0x81, f"came back {wide.code.hex()}"
    narrow = select.arith_into_imm("add", cell, 0, relocated=False)
    assert narrow is not None and narrow.code[0] == 0x83


@pytest.mark.parametrize("obj", FIXTURES, ids=lambda p: p.stem)
def test_a_relocated_field_never_changes_width(obj: Path) -> None:
    """The invariant the `relocated` flag exists to hold, over the corpus.

    A fixup names a field by its address and patches a fixed number of
    bytes. If the selector picks an encoding whose field is narrower -- the
    sign-extended byte immediate for `add ax,offset X`, which arrives as
    `add ax,0` -- the linker writes two bytes into a one-byte field and over
    whatever follows it.

    Nothing that compares the emitted code can see this: before linking,
    both forms disassemble as `add ax,0`. It took reducing a generated
    program to 31 lines and diffing the two linked images, where one said
    `add ax,0DCh` and the other `add ax,0FFDCh`.
    """
    from qbopt.model import lir
    from qbopt.model import mir
    from qbopt.backend import asm
    from qbopt.backend import lower
    from qbopt.objectfile import omf
    from qbopt.frontend import declen
    from qbopt.frontend import blocks as split
    from qbopt.frontend.blocks import code_map

    found = corpus.loaded(obj)
    assert found is not None
    mapped = code_map(found)
    if isinstance(mapped, str):
        return
    fields = frozenset(one.offset for one in omf.fixups(omf.parse(obj.read_bytes())) if one.seg == found.seg)
    raised = mir.bodies(found, split.partition(found, mapped))
    source = raised.source
    for _name, body in raised:
        for block in body.blocks:
            for op in block.ops:
                node = source.nodes.get(op.id)
                if not op.source_backed or node is None:
                    continue
                owned = tuple(span for identity in op.absorbed for span in source.occurrences[identity])
                selected = lir.Insn(
                    op.at,
                    owned[0],
                    lower.current(op, node=node),
                    tuple(value.id for value in op.defines),
                    tuple(value.id for value in op.uses),
                    op=op,
                    node=node,
                    symbol=op.symbol,
                    spread=owned,
                )
                what = asm._semantics(selected)
                field = asm._field_in(found, selected, fields, source)
                if what is None or field is None:
                    continue
                # Not a folded runtime call. Its address is its first push,
                # so the bytes there are not the instruction its field
                # belongs to -- and the fields it does have are the
                # operands' own, reused rather than re-encoded.
                if op.id is not None and op.id in source.absorbed:
                    continue
                was = declen.decode(found.code, op.at)
                if was is None:
                    continue
                if found.code[op.at : op.at + 1] in (b"\x9a", b"\xea"):
                    continue  # the far forms relocate a pointer, and have no narrower one
                if was.disp_at == field:
                    wanted = was.disp_len
                elif was.imm_at == field:
                    wanted = was.imm_len
                else:
                    continue  # a far call's target, which has no narrower form
                made = select.emit(what, at=op.at, relocated=True)
                if made is None:
                    continue
                now = declen.decode(made.code, 0)
                assert now is not None
                got = now.disp_len if made.displacement_at is not None else now.imm_len
                assert got == wanted, (
                    f"{obj.stem} {op.at:#x}: {was.insn} has a {wanted}-byte relocated field, "
                    f"emitted {made.code.hex()} has {got}"
                )


@pytest.mark.parametrize(
    ("hexs", "want"),
    [
        ("6685c0", "test eax,eax"),
        ("85c0", "test ax,ax"),
        # a compare against zero is deliberately improved into the test form,
        # which is a byte shorter and asks the same question -- the point here
        # is that the answer is never a *different* comparison
        ("6683f800", "test eax,eax"),
        ("663bc0", "cmp eax,eax"),
    ],
)
def test_a_compare_keeps_the_mnemonic_it_was_given(hexs: str, want: str) -> None:
    """`test` and `cmp` are both Operation.COMPARE and are not the same test.

    `cmp a,b` sets the flags from a-b; `test a,b` sets them from a AND b.
    For `test eax,eax` that is the value's own sign and zero, and re-emitting
    it as `cmp eax,eax` sets ZF unconditionally -- a branch on it always
    goes the same way.

    select's COMPARE arm hardcoded "cmp" in all four of its shapes while
    ir.Semantics carried name='test' all along. Nothing caught it because BC
    emits no `test` at all: 0 across the corpus and all of qb-qrender. The
    first thing to produce one would have been a peephole turning
    `cmp reg,0` into the shorter `test reg,reg`, which is where this was
    found.
    """
    from qbopt.frontend import declen
    from qbopt.objectfile.module import Addr
    from qbopt.objectfile.module import Space

    insn = declen.decode(bytes.fromhex(hexs), 0)
    assert insn is not None
    what = ir.instruction_semantics(insn, lambda *_a, **_k: Addr(Space.LITERAL, 0, 0))
    made = select.emit(what, at=0)
    assert made is not None, f"{want} came back unencodable"
    back = declen.decode(made.code, 0)
    assert back is not None
    assert str(back.insn) == want, f"asked for {want}, emitted {made.code.hex()} = {back.insn}"
    # the invariant underneath: a `test` never comes back as a `cmp`
    if str(insn.insn).startswith("test "):
        assert str(back.insn).startswith("test ")


@pytest.mark.parametrize(
    ("reg", "want"),
    [(Register.EAX, "6685c0"), (Register.ECX, "6685c9"), (Register.AX, "85c0"), (Register.BX, "85db")],
)
def test_a_comparison_against_zero_takes_the_test_form(reg: Register_, want: str) -> None:
    """`cmp reg,0` is a byte longer than `test reg,reg` and asks the same thing.

    Both set SF, ZF and PF from the value and clear CF and OF, so every
    conditional jump reads the same answer -- and `test` needs no immediate
    at all. `cmp eax,0` is `66 83 f8 00` against `66 85 c0`.

    BC never writes `cmp reg,0`: every one of these is something absorption
    emits, so this is a peephole on this pass's own output. 54 bytes over
    qb-qrender, 8 over the corpus.

    `or reg,reg` would save the same byte and was the other candidate. It
    writes the register -- the same value, but MIR records a definition --
    and downstream liveness would have to track that artificial definition.
    `test` writes nothing.
    """
    made = select.compare(ir.Reg(register=reg, width=4 if reg in (Register.EAX, Register.ECX) else 2), 0)
    assert made is not None
    assert made.code.hex() == want


def test_a_comparison_against_zero_stays_a_compare_when_relocated() -> None:
    """A relocated immediate is an address, not the number zero.

    `cmp ax,offset X` arrives as `cmp ax,0` exactly as `add ax,offset X`
    arrives as `add ax,0`, and turning it into `test ax,ax` would ask about
    ax rather than about the address -- and leave the fixup naming a field
    that no longer exists.
    """
    made = select.compare(ir.Reg(register=Register.AX, width=2), 0, relocated=True)
    assert made is not None
    assert made.code[0] != 0x85, f"a relocated compare became a test: {made.code.hex()}"


def test_a_remap_reaches_inside_a_memory_operand() -> None:
    """A cell is reached by a register as much as an accumulator is held in one.

    _remapped was applied to register operands and a memory operand was
    passed through as it stood, so an allocation that moved a value out of
    the register a cell is reached by came out half-renamed: segld's array
    base was rewritten `mov di,0` while `[si+0Ah]` behind it kept reading
    si. Every host test passed and the program printed 0 for 1050.

    Both halves matter -- `through` says which register reaches the cell and
    Addr.base is the one that gets encoded, so remapping only the first
    changed nothing at all and did it silently.
    """
    from qbopt.model import ir
    from qbopt.objectfile.module import Addr
    from qbopt.objectfile.module import Space

    where = {Register.SI: Register.DI, Register.ESI: Register.EDI}
    cell = ir.Mem(Addr(Space.LITERAL, 0x0A, base=Register.SI), 2, through=Register.SI, offset=0x0A, disp_width=1)
    what = ir.Semantics(
        ir.Operation.BINARY,
        "add",
        dests=(ir.Reg(register=Register.BX, width=2),),
        sources=(ir.Reg(register=Register.BX, width=2), cell),
    )
    plain = _made(select.emit(what, at=0))
    moved = _made(select.emit(what, at=0, where=where))
    assert plain.code != moved.code, "the remap never reached the operand"

    from iced_x86 import Decoder
    from iced_x86 import Formatter
    from iced_x86 import FormatterSyntax

    shown = Formatter(FormatterSyntax.NASM).format(next(iter(Decoder(16, moved.code, ip=0))))
    assert "di" in shown and "si" not in shown, shown


def test_a_held_operand_names_a_value_and_not_a_register() -> None:
    """The operand kind rule 5 needs.

    A pass rewriting an operand from a cell to a register had no way to say
    "the register this value is in", so it said `ir.Reg(register=BX)` --
    naming a register, which is the allocator's answer. forward.py carries
    22 machine references for exactly that reason.
    """
    from iced_x86 import Register

    got = select._operand(ir.Held(value=7, width=2), None, {7: Register.EBX})
    assert got == ir.Reg(register=Register.BX, width=2), "resolved at the width asked for"
    wide = select._operand(ir.Held(value=7, width=4), None, {7: Register.EBX})
    assert wide == ir.Reg(register=Register.EBX, width=4)


def test_an_unresolved_held_is_refused_rather_than_guessed() -> None:
    """A Held the allocation had no register for is a value a pass asked to
    be somewhere and nothing decided where.

    Guessing a register there is how the five attempts in this project's
    history produced wrong programs, so emit() returns None and the caller
    keeps what BC wrote.
    """
    what = ir.Semantics(
        ir.Operation.MOVE,
        "mov",
        dests=(ir.Held(value=9, width=2),),
        sources=(ir.Imm(value=1, width=2),),
    )
    assert select.emit(what, held={}) is None, "no register for value 9"
    from iced_x86 import Register

    assert select.emit(what, held={9: Register.EBX}) is not None, "and it emits once there is"


def test_an_absorbed_site_comes_back_with_a_field_for_every_fixup() -> None:
    """974 of the corpus's 1,211 absorbable sites carry two.

    `x * y` over two static addresses is `mov eax,[x] / imul eax,[y]` and
    both are relocated -- which is why Emitted reports a field per
    instruction rather than one. The sequence itself is calls.py's and this
    is the seam: emission belongs here, and the machine arm is what phase D
    retires.
    """
    from pathlib import Path

    from qbopt.legacy import calls
    from qbopt.analysis import flags
    from qbopt.backend import select
    from qbopt.objectfile import omf
    from qbopt.objectfile import module
    from qbopt.frontend import blocks as split
    from qbopt.frontend.blocks import code_map

    seen = both = 0
    for name in ("chain-p-g2", "lngmix-p-g2", "matrix-p-g2", "press-p-g2"):
        found = module.of(omf.parse((Path("fixtures/omf") / f"{name}.obj".lower()).read_bytes()))
        assert found is not None
        mapped = code_map(found)
        if isinstance(mapped, str):
            continue
        blocks = split.partition(found, mapped)
        reached = [insn for one in blocks for insn in one.insns]
        for site in calls.sites(found, reached, blocks):
            theirs = calls.absorb(site, flags.Flag(0))
            if isinstance(theirs, str):
                continue
            seen += 1
            ours = select.absorbed(site, flags.Flag(0))
            assert not isinstance(ours, str), ours
            assert ours.code == theirs.code, f"{site.at:#x} {site.name}: different bytes"
            wanted = select.absorbed_fixups(site, flags.Flag(0))
            assert len(ours.places) == len(wanted), f"{site.at:#x}: {len(ours.places)} fields for {len(wanted)} fixups"
            both += len(ours.places) == 2
    assert seen, "no site absorbed, so this proves nothing"
    assert both, "none of them carried two, which is the case this exists for"


def _funnel(count, name="shrd") -> ir.Semantics:
    """`shld`/`shrd eax,edx,count` as this layer says it."""
    low = ir.Reg(Register.EAX, 4)
    return ir.Semantics(
        ir.Operation.FUNNEL,
        name,
        dests=(low,),
        sources=(low, ir.Reg(Register.EDX, 4), count),
    )


def test_a_funnel_shift_is_two_address_in_its_low_half() -> None:
    """`shrd r,r,n` shifts the destination and reads it, like every other
    two-address form: an allocation that moves the destination without the
    operand it also reads shifts whatever that register happened to hold."""
    from qbopt.backend import target

    assert target.tied(_funnel(ir.Imm(16, 1))) is Register.EAX


def test_a_funnel_shift_by_a_register_takes_its_count_in_cl() -> None:
    """The only register `shrd` can count from. Nothing else about it is
    fixed -- the two sources are whichever registers the allocation picked."""
    from qbopt.backend import target

    dynamic = target.reads(_funnel(ir.Reg(Register.CL, 1)))
    assert dynamic.get(Register.ECX) is not None and dynamic[Register.ECX].fixed is Register.ECX
    assert Register.EAX not in target.reads(_funnel(ir.Imm(16, 1)))
    assert not target.writes(_funnel(ir.Imm(16, 1))), "shrd writes only what it names"


@pytest.mark.parametrize(
    ("count", "want"),
    [(ir.Imm(16, 1), "660fac d0 10"), (ir.Reg(Register.CL, 1), "660fad d0")],
)
def test_a_funnel_shift_emits_the_form_its_count_asks_for(count, want: str) -> None:
    """Immediate and cl are different opcodes, 0F AC and 0F AD."""
    made = select.emit(_funnel(count))
    assert made is not None
    assert made.code.hex() == want.replace(" ", "")


def test_a_left_funnel_shift_emits_shld() -> None:
    """DX:AX return extraction needs SHLD's incoming high bits, not SHRD's low bits."""
    made = select.emit(_funnel(ir.Imm(16, 1), "shld"))
    assert made is not None
    assert made.code.hex() == "660fa4d010"


def _restoring(wide: Register_, low: Register_, high: Register_) -> ir.Semantics:
    """A wide value split back into the two halves BC reads it as."""
    return ir.restoring(ir.Reg(wide, 4), ir.Reg(low, 2), ir.Reg(high, 2))


def test_a_restore_says_which_value_it_splits_and_into_which_halves() -> None:
    """The idiom had no operands at all: a `pair` number chose the registers,
    so whoever built the node picked ax and dx rather than the allocation."""
    what = _restoring(Register.EAX, Register.AX, Register.DX)
    assert what.op is ir.Operation.RESTORE
    assert what.sources == (ir.Reg(Register.EAX, 4),)
    assert what.dests == (ir.Reg(Register.AX, 2), ir.Reg(Register.DX, 2))


@pytest.mark.parametrize(
    ("wide", "low", "high", "want"),
    [
        (Register.EAX, Register.AX, Register.DX, "6650585a"),
        (Register.ECX, Register.CX, Register.BX, "6651595b"),
        (Register.ESI, Register.SI, Register.DI, "66565e5f"),
    ],
)
def test_a_restore_encodes_the_registers_the_allocation_chose(wide, low, high, want: str) -> None:
    """`push wide / pop low / pop high`, whichever three they are. The first
    two are what the pair table already held, so the old encoding stands."""
    made = select.emit(_restoring(wide, low, high))
    assert made is not None and made.code.hex() == want


def test_a_relocated_cell_is_reached_through_the_register_it_was_placed_in() -> None:
    """The base BC wrote is not the base the allocation chose.

    `mov ax,[si+arr]` is how BC writes an array element, and the raise keeps
    si in `addr.base`. Once a pass recomputes that offset into a value of
    its own, the allocation answers with whatever register it placed it in
    and `through` says so -- but this branch went on encoding `addr.base`,
    which names element zero through whatever si happens to hold.
    """
    from qbopt.model import ir
    from qbopt.objectfile.module import Addr
    from qbopt.objectfile.module import Space

    where = Addr(Space.SEGMENT, 0x2, base=Register.SI)
    placed = ir.Mem(where, 2, Register.BX, 2, 1, base=ir.Held(17, 2))
    # Against the encoded bytes: iced's MemoryOperand does not read back.
    made = select.move_from(Register.AX, placed)
    assert made is not None, "a placed cell is encodable"
    got = next(iter(Decoder(BITNESS, made.code, ip=0)))
    assert got.memory_base == Register.BX, f"encoded through {got.memory_base}, not the register it was placed in"
    assert select.operand_of(placed)[1] is True, "a segment address still needs its fixup moved"

    # A cell nothing based keeps BC's own base: that si is the whole address.
    plain = select.move_from(Register.AX, ir.Mem(where, 2))
    assert plain is not None
    assert next(iter(Decoder(BITNESS, plain.code, ip=0))).memory_base == Register.SI
    assert select.operand_of(ir.Mem(where, 2))[1] is True


def test_a_literal_cell_is_reached_through_the_register_it_was_placed_in() -> None:
    """The same defect one space over, and this is the one arrprm hits.

    Its element address is `[abs+si+0x2]` -- a displacement no fixup claims,
    reached through si. Encoding `addr.base` there read element zero and the
    program printed ' 0  0' where it wanted ' 7  8'. The displacement size
    follows the register actually encoded, not the one BC wrote.
    """
    from qbopt.model import ir
    from qbopt.objectfile.module import Addr
    from qbopt.objectfile.module import Space

    where = Addr(Space.LITERAL, 0x2, base=Register.SI)
    placed = ir.Mem(where, 2, Register.BX, 2, 1, base=ir.Held(17, 2))
    made = select.move_from(Register.AX, placed)
    assert made is not None, "a placed cell is encodable"
    got = next(iter(Decoder(BITNESS, made.code, ip=0)))
    assert got.memory_base == Register.BX, f"encoded through {got.memory_base}, not the register it was placed in"
    assert got.memory_displacement == 2, f"the displacement moved to {got.memory_displacement}"

    # A cell nothing based keeps BC's own base, and both forms still encode.
    plain = select.move_from(Register.AX, ir.Mem(where, 2))
    assert plain is not None
    back = next(iter(Decoder(BITNESS, plain.code, ip=0)))
    assert back.memory_base == Register.SI
    assert back.memory_displacement == 2


def test_a_far_cell_is_reached_through_the_register_it_was_placed_in() -> None:
    """The third space with the same defect, and arrprm's own.

    A $DYNAMIC element is `es:[bx]`, where bx held the byte offset BC
    computed. Once a pass recomputes that offset the allocation places it
    somewhere of its choosing, and encoding `addr.base` writes through
    whatever bx still holds -- which is how ' 7  8' reached the wrong
    four bytes and the program printed ' 0  0'.
    """
    from qbopt.model import ir
    from qbopt.objectfile.module import Addr
    from qbopt.objectfile.module import Space

    where = Addr(Space.FAR, 0, base=Register.BX, segment=Register.ES)
    placed = ir.Mem(where, 2, Register.DI, 0, 0, base=ir.Held(21, 2))
    made = select.move_from(Register.AX, placed)
    assert made is not None, "a placed cell is encodable"
    got = next(iter(Decoder(BITNESS, made.code, ip=0)))
    assert got.memory_base == Register.DI, f"encoded through {got.memory_base}, not the register it was placed in"
    assert got.memory_segment == Register.ES, "the override is part of the address"

    # A cell nothing based keeps the register BC wrote it through.
    plain = select.move_from(Register.AX, ir.Mem(where, 2))
    assert plain is not None
    back = next(iter(Decoder(BITNESS, plain.code, ip=0)))
    assert back.memory_base == Register.BX
    assert back.memory_segment == Register.ES


def test_a_frame_slots_displacement_is_not_a_relocatable_field() -> None:
    """`[bp-22h]` is an offset from the frame pointer, not an address.

    lngmix printed S= 775174098 for 142900 with both its divides hoisted:
    the accumulator spilled, `add cx,[a]` became a load and
    `add [bp-22h],bx`, and the fixup that named `a` was bound to the add's
    own displacement -- so the loop added whatever lived at that address
    and the slot number was overwritten.

    An indexed data reference is a different thing and keeps its fixup:
    `mov [si+x],bx` reaches an array through si and the displacement is
    the array's own address.
    """
    from iced_x86 import Register

    from qbopt.model import ir
    from qbopt.backend import select

    slot = ir.Mem(addr=None, width=2, through=Register.BP, offset=-0x22, disp_width=2)
    store = select.emit(ir.Semantics(ir.Operation.MOVE, "mov", (slot,), (ir.Reg(Register.BX, 2),)))
    assert store is not None
    assert store.places == (), f"the frame slot offered a field at {store.places}"

    from qbopt.objectfile.module import Addr
    from qbopt.objectfile.module import Space

    array = ir.Mem(Addr(Space.SEGMENT, 0x6, base=Register.SI), 2, through=Register.SI)
    indexed = select.emit(ir.Semantics(ir.Operation.MOVE, "mov", (array,), (ir.Reg(Register.BX, 2),)))
    assert indexed is not None
    assert indexed.places, "an array reached through si still carries its own address"


def test_an_instruction_holding_a_moved_operand_is_not_the_site_it_came_from() -> None:
    """It keeps the id, because the id is how its fixup is found.

    Everything else the id answers is about the operation's own bytes, and
    the instruction has none: read as the site it would emit the whole
    absorbed divide again in place of the `mov` it is. And one recorded
    fixup or none -- two means the operation had two relocated operands
    and only one moved, and the count cannot say which is which.
    """
    from qbopt.model import ir
    from qbopt.model import lir
    from qbopt.model import mir
    from qbopt.backend import asm
    from qbopt.objectfile import omf
    from qbopt.objectfile import module
    from qbopt.frontend import blocks as split
    from qbopt.frontend.blocks import code_map

    found = module.of(omf.parse(Path("fixtures/omf/cmpord-p-g2.obj").read_bytes()))
    raised = mir.bodies(found, split.partition(found, code_map(found)))
    bodies = list(raised)
    site = next(one for one in raised.source.absorbed)
    # Kind and all, so the site's own record would answer for it: the
    # kind check alone lets one through whose operation still raises as a
    # divide, and what says no is that the operand moved here.
    kind = raised.source.absorbed[site][0]
    lifted = mir.Op(
        at=kind.start,
        op=ir.Operation.MOVE,
        name="mov",
        defines=(),
        uses=(),
        kind=mir.absorbs(kind.name),
        id=site,
        symbol=True,
    )
    address = ir.Addr(ir.Space.SEGMENT, 0)
    instruction = lir.Insn(
        kind.start,
        (kind.start, kind.start),
        ir.Semantics(
            ir.Operation.MOVE,
            "mov",
            (ir.Reg(Register.AX, 2),),
            (ir.Imm(0, 2, address),),
        ),
        (),
        (),
        op=lifted,
        symbol=True,
    )
    assert asm._folded_site(instruction, found, raised.source) is None, "the moved operand was read as the site"
    assert bodies, "the fixture raised nothing"

    # And the fixup: one is its own, two is a guess.
    was = raised.source.refs.get(site)
    raised.source.refs[site] = (0x40,)
    assert asm._field_in(found, instruction, frozenset({0x40}), raised.source) == 0x40
    raised.source.refs[site] = (0x40, 0x44)
    assert asm._field_in(found, instruction, frozenset({0x40, 0x44}), raised.source) is None, (
        "one of two fixups was picked"
    )
    raised.source.refs[site] = was


@pytest.mark.parametrize(("count", "width"), [(Register.CL, 1), (Register.CX, 2), (Register.ECX, 4)])
def test_a_shift_counts_from_cl_whatever_width_holds_the_count(count: Register_, width: int) -> None:
    """A u16 count held in cx selected nothing: `sar eax,cx` had no encoding. The shift reads only cl."""
    eax = ir.Reg(Register.EAX, 4)
    made = select.emit(ir.Semantics(ir.Operation.BINARY, "sar", (eax,), (eax, ir.Reg(count, width))))
    assert made is not None
    assert made.code.hex() == "66d3f8"


def test_a_count_in_ch_is_not_one_in_cl() -> None:
    eax = ir.Reg(Register.EAX, 4)
    assert select.emit(ir.Semantics(ir.Operation.BINARY, "sar", (eax,), (eax, ir.Reg(Register.CH, 1)))) is None


def test_a_global_cell_reached_through_base_and_index_keeps_its_relocation() -> None:
    """Lowering folds `base + index` into a global array's cell; nothing encoded it."""
    from iced_x86 import Register

    from qbopt.backend import select
    from qbopt.model import ir
    from qbopt.objectfile.module import Addr, Space

    cell = ir.Mem(
        Addr(Space.SEGMENT, 6),
        2,
        through=Register.BX,
        base=ir.Held(1, 2),
        index=ir.Held(2, 2),
        index_through=Register.DI,
    )
    load = select.emit(ir.Semantics(ir.Operation.MOVE, "mov", (ir.Reg(Register.DX, 2),), (cell,)))
    assert load is not None
    back = next(iter(Decoder(BITNESS, load.code, ip=0)))
    assert (back.memory_base, back.memory_index) == (Register.BX, Register.DI)
    assert load.places, "the relocation lost its field"
