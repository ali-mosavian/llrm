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
from qbopt import ir
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

    Every operation in the corpus, and every one of the 46,535 emitted
    across the corpus and qb-qrender together decodes to what it was asked
    for. A canary: if this stops being all of them, something narrowed.
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
    # The 44 that do not come back are the movsw of suite/fpdeep.bas, the one
    # encoding this deliberately refuses -- see REFUSED in tests/test_ir.py.
    # Every other operation in the corpus selects.
    assert (total, emitted) == (37825, 37781)
    assert total - emitted == 44


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


def test_a_literal_address_through_a_register_uses_the_byte_displacement() -> None:
    """`add bx,[si+0Ah]` is `03 5c 0a`, not `03 9c 0a 00`.

    Space.LITERAL hardcoded a two-byte displacement. That is right for a
    bare direct address and a byte too many everywhere BC reaches one
    through a register, which is how it writes a field of a record: 1,247
    `add` sites and 785 `mov` sites in qb-qrender, a byte each.
    """
    from qbopt.module import Addr
    from qbopt.module import Space

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
    from qbopt.module import Addr
    from qbopt.module import Space

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
    assert select.push_imm(0, 2, relocated=True).code.hex() == "680000"
    assert select.push_imm(0, 2, relocated=False).code.hex() == "6a00"
    # and the wide push shrinks on the same rule
    assert select.push_imm(3, 4, relocated=True).code.hex() == "666803000000"
    assert select.push_imm(3, 4, relocated=False).code.hex() == "666a03"


@pytest.mark.parametrize(
    ("name", "reg", "want"),
    [("shl", Register.AX, "d1e0"), ("shl", Register.BX, "d1e3"), ("sar", Register.AX, "d1f8")],
)
def test_a_shift_by_one_takes_its_own_opcode(name: str, reg: Register, want: str) -> None:
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
    assert select.arith_imm("add", Register.AX, 3).code.hex() == "83c003"
    assert select.arith_imm("add", Register.BX, 3).code.hex() == "83c303"
    assert select.arith_imm("add", Register.BX, 0x1286).code.hex() == "81c38612"


@pytest.mark.parametrize(("value", "want"), [(0x32, "837ee232"), (0, "837ee200"), (-1, "837ee2ff")])
def test_a_compare_of_memory_against_a_small_literal_takes_the_byte_form(value: int, want: str) -> None:
    """`cmp word ptr [bp-1Eh],32h` is `83 7e e2 32`, not `81 7e e2 32 00`.

    arith_into_imm has tried the byte form since it was written; compare's
    own memory arm hardcoded the word one, so every `cmp` against a frame
    slot paid a byte. 209 sites in qb-qrender, and the fix is to stop having
    two of these.
    """
    from qbopt.module import Addr
    from qbopt.module import Space

    cell = ir.Mem(addr=Addr(Space.FRAME, -0x1E, 0), width=2)
    made = select.compare(cell, value)
    assert made is not None
    assert made.code.hex() == want


def test_a_compare_of_memory_against_a_large_literal_stays_wide() -> None:
    from qbopt.module import Addr
    from qbopt.module import Space

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
    from qbopt.module import Addr
    from qbopt.module import Space

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
    from qbopt import declen
    from qbopt import layout
    from qbopt import mir
    from qbopt import omf
    from qbopt import blocks as split
    from qbopt.blocks import code_map

    found = corpus.loaded(obj)
    assert found is not None
    mapped = code_map(found)
    if isinstance(mapped, str):
        return
    fields = frozenset(one.offset for one in omf.fixups(omf.parse(obj.read_bytes())) if one.seg == found.seg)
    for _name, body in mir.bodies(found, split.partition(found, mapped)):
        for block in body.blocks:
            for op in block.ops:
                what = layout._semantics(op)
                field = layout._field_in(found, op, fields)
                if what is None or field is None:
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
    from qbopt import declen
    from qbopt.module import Addr
    from qbopt.module import Space

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
def test_a_comparison_against_zero_takes_the_test_form(reg: Register, want: str) -> None:
    """`cmp reg,0` is a byte longer than `test reg,reg` and asks the same thing.

    Both set SF, ZF and PF from the value and clear CF and OF, so every
    conditional jump reads the same answer -- and `test` needs no immediate
    at all. `cmp eax,0` is `66 83 f8 00` against `66 85 c0`.

    BC never writes `cmp reg,0`: every one of these is something absorption
    emits, so this is a peephole on this pass's own output. 54 bytes over
    qb-qrender, 8 over the corpus.

    `or reg,reg` would save the same byte and was the other candidate. It
    writes the register -- the same value, but MIR records a definition --
    and three consumers here key on which value is in a register:
    avail.redundant's map, simplify._target_survives, and transform.placed.
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
