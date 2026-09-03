"""
MIR to machine bytes: the instructions this pass writes itself.

Everything emitted before this module was a rearrangement of bytes BC had
already written -- calls.py assembles from a fixed repertoire around a call
site, reencode.py re-encodes an instruction that already exists, and
lower() hands back each op's own origin span. None of them can produce an
instruction the input did not contain, which mir.lower()'s docstring names
as the missing piece and the reason a transform can delete but not rewrite.

Deliberately a small table rather than a general selector. Every entry here
exists because something measured needed it, and an op this does not know
is refused rather than approximated -- the same discipline as the rest of
the pass, and the reason a caller can treat None as "leave it alone".

Sixteen-bit code, so a 32-bit operand carries the 0x66 prefix and
`mov ecx,eax` is three bytes, not two. That matters to callers: this is the
one emitter whose output can be *longer* than what it replaces, and the
win is instructions and cycles rather than size.
"""

from dataclasses import replace
from dataclasses import dataclass

from iced_x86 import Code
from iced_x86 import Code_
from iced_x86 import Decoder
from iced_x86 import Encoder
from iced_x86 import Register
from iced_x86 import Register_
from iced_x86 import Instruction
from iced_x86 import MemoryOperand
from iced_x86 import RepPrefixKind

from qbopt import ir
from qbopt.module import Space
from qbopt.declen import BITNESS

# The 32-bit roots this can name, and their 16-bit halves.
# The six mir.py tracks, plus bp and sp. Those two are not values -- they
# are where values live -- but they are still registers an instruction
# names, and every procedure opens `push bp / mov bp,sp` and closes by
# popping it back. A selector that could not say them could not emit a
# prologue.
_WIDE = {Register.EAX, Register.ECX, Register.EDX, Register.EBX, Register.ESI, Register.EDI, Register.EBP, Register.ESP}
_NARROW = {Register.AX, Register.CX, Register.DX, Register.BX, Register.SI, Register.DI, Register.BP, Register.SP}
# The byte halves. BC reaches for them to clear a high byte (`xor bh,bh`)
# and to read one byte of an array.
_BYTE = {
    Register.AL,
    Register.CL,
    Register.DL,
    Register.BL,
    Register.AH,
    Register.CH,
    Register.DH,
    Register.BH,
}


@dataclass(frozen=True, slots=True)
class Emitted:
    """The bytes, and where a relocated displacement ended up inside them.

    An instruction relocates at most one field, and it is not always the
    displacement: `push offset X` and `mov ax,offset X` put the address in
    the immediate, and 464 of the corpus's fixups are in one of those two.
    Both offsets are reported and `relocated_at` is what a caller wants.

    Where the field is, not whether it is relocated: a frame slot has a
    displacement too and it is a real number in the code, `8b 86 e8 ff`
    meaning [bp-18h]. Which fields a fixup names is the module's to say, so
    the caller asks that and uses these to know where to put the answer.
    """

    code: bytes
    displacement_at: int | None = None
    immediate_at: int | None = None

    @property
    def relocated_at(self) -> int | None:
        """Where this instruction's one relocatable field landed."""
        return self.displacement_at if self.displacement_at is not None else self.immediate_at


def _assemble(made: Instruction, at: int) -> Emitted | None:
    """The bytes, with both constant fields located.

    Read back off the encoded bytes rather than predicted, which is what
    calls.py's own assemble() does and for the same reason: the encoder
    chooses the form, so only it knows where a field ended up.
    """
    encoder = Encoder(BITNESS)
    try:
        encoder.encode(made, at)
    except ValueError:
        return None
    code = encoder.take_buffer()
    decoder = Decoder(BITNESS, code, ip=at)
    decoded = next(iter(decoder), None)
    if decoded is None:
        return None
    offsets = decoder.get_constant_offsets(decoded)
    return Emitted(
        code,
        offsets.displacement_offset if offsets.has_displacement else None,
        offsets.immediate_offset if offsets.has_immediate else None,
    )


def _displacement_size(base: Register_, value: int) -> int:
    """How many bytes the displacement needs.

    One where it fits a signed byte, which is every frame slot BC writes and
    a byte cheaper each time -- `mov [bp-14h],dx` is `89 56 ec` and not
    `89 96 ec ff`. Zero where there is none, except through bp: mod=00 with
    r/m=110 is a direct address in 16-bit encoding, so `[bp]` has to be
    written `[bp+0]`.
    """
    if value == 0 and base is not Register.BP:
        return 0
    return 1 if -128 <= value <= 127 else 2


def operand_of(what: ir.Mem) -> tuple[MemoryOperand, bool] | None:
    """`what` as an encodable memory operand, and whether it is relocated.

    Two spaces: a relocated segment address, emitted as zero with its fixup
    moved, and a frame slot, whose displacement really is in the code.
    Everything else is refused -- a Space.FAR address needs a segment
    override this does not model, a Space.GROUP one is refused everywhere in
    this project, and a Space.STACK one is mir.py's own name for a push slot
    rather than anything an instruction encodes.

    A segment address may carry a base register, which is how BC writes an
    array element: `mov ax,[si+arr]`, where the fixup names the array and si
    holds the offset into it. The base comes along -- dropping it would
    silently name element zero -- and lift.relocated_memory() builds the
    same operand for the same reason.
    """
    addr = what.addr
    if addr is None:
        # No nameable address, but the operand is still encodable where it
        # is reached through a register: `mov ax,[si]` means whatever si
        # points at, which is exactly what the encoding says.
        if what.through in (Register.SI, Register.DI, Register.BX, Register.BP):
            # The displacement comes too. `push dword [bx+4]` emitted as
            # `push [bx]` is a working program reading the wrong four bytes,
            # and byref2 printed 0 where it wanted 16 on two configurations
            # before this line said `what.offset`.
            # The field's own width where it had one, since a fixup fills a
            # displacement of zero and dropping it reads the wrong address.
            wide = what.disp_width or _displacement_size(what.through, what.offset)
            return MemoryOperand(base=what.through, displ=what.offset, displ_size=wide), False
        return None
    match addr.space:
        case Space.SEGMENT:
            return MemoryOperand(base=addr.base, displ=0, displ_size=2), True
        case Space.FRAME if addr.base == Register.NONE:
            return (
                MemoryOperand(
                    base=Register.BP,
                    displ=addr.disp,
                    displ_size=_displacement_size(Register.BP, addr.disp),
                ),
                False,
            )
        case Space.FAR:
            # A $DYNAMIC array element: `es:[bx]`, where bx holds the byte
            # offset BC computed and es whatever a prior `mov es,[desc+2]`
            # loaded. The override is part of the address's identity -- and
            # of the encoding -- so it comes along.
            if addr.segment == Register.NONE:
                return None
            return (
                MemoryOperand(
                    base=addr.base,
                    displ=addr.disp,
                    displ_size=_displacement_size(addr.base, addr.disp),
                    seg=addr.segment,
                ),
                False,
            )
        case Space.LITERAL:
            # A displacement no fixup claims, so the number in the code is
            # the address and nothing has to move with it. BC writes these
            # for the runtime's own fixed locations.
            #
            # Two bytes without a base, and only without one: 16-bit mod=00
            # r/m=110 is the direct-address form and it carries a word,
            # while the mod=01 encoding that would hold a byte means
            # `[bp+disp8]` -- a different address. Through a register there
            # is no such collision and a byte usually fits, which is how BC
            # reaches a field of a record: `add bx,[si+0Ah]` is `03 5c 0a`.
            # Hardcoding two cost a byte at 1,247 add sites and 785 mov
            # sites in qb-qrender, and none at all in fixtures/omf.
            wide = 2 if addr.base == Register.NONE else _displacement_size(addr.base, addr.disp)
            return MemoryOperand(base=addr.base, displ=addr.disp, displ_size=wide), False
        case _:
            return None


def move(into: Register_, outof: Register_, at: int = 0) -> Emitted | None:
    """`mov into, outof`, or None if this cannot name that pair.

    Both registers at one width, and a width this knows. A move between
    different widths is a different instruction with different semantics --
    zero or sign extension -- and guessing which was meant is exactly what
    this refuses to do.
    """
    if into is outof:
        return Emitted(b"")  # a move to itself is no instruction at all
    if into in _WIDE and outof in _WIDE:
        code = Code.MOV_R32_RM32
    elif into in _NARROW and outof in _NARROW:
        code = Code.MOV_R16_RM16
    else:
        return None
    return _assemble(Instruction.create_reg_reg(code, into, outof), at)


# What one MIR operation is called in iced's own Code names. The names are
# systematic -- MOV_R16_IMM16, ADD_R16_RM16, NEG_RM16 -- so the table is the
# mnemonic and the shape, and the width fills itself in. Anything absent is
# refused rather than approximated, which is the whole discipline of this
# module.
#
# adc and sbb are here and matter: BC writes every 32-bit operation as a pair
# and the second half is one of them, so a selector that could not emit them
# could not emit BC's own arithmetic back.
TWO_OPERAND = ("add", "adc", "sub", "sbb", "and", "or", "xor", "cmp")
ONE_OPERAND = ("neg", "not", "inc", "dec")

# The width each register names, and the register file at each width. One
# table rather than three lookups, and the only place widths are written down.
WIDTHS: dict[Register_, int] = {}
# And the inverse, which ir.Held needs: a root and a width name one register.
# There is no byte-wide si, so a missing entry means the width cannot be had
# and the caller keeps the root.
AT_WIDTH: dict[Register_, dict[int, Register_]] = {}
for _row, _size in ((_WIDE, 4), (_NARROW, 2), (_BYTE, 1)):
    for _one in _row:
        WIDTHS[_one] = _size
        # setdefault, not assignment: al and ah are both one byte and both
        # root to eax, and the later one was winning -- an ir.Held of width
        # 1 resolved to `ah`, which is a different register holding a
        # different byte.
        AT_WIDTH.setdefault(ir.ROOT.get(_one, _one), {}).setdefault(_size, _one)


def _code(name: str) -> Code_ | None:
    """iced's Code value by its own name, or None where there is no such form.

    Looked up by name rather than written out: iced spells them
    systematically -- MOV_R16_IMM16, ADD_R16_RM16, NEG_RM16 -- so the
    mnemonic and the width build the name, and a form that does not exist
    comes back None instead of being guessed at.

    Code_ rather than Code: iced exports the module as the second and the
    type as the first, the way it does for Register.
    """
    found = getattr(Code, name, None)
    return found


def _width_of(what: ir.Loc) -> int | None:
    match what:
        case ir.Reg(register=register):
            return WIDTHS.get(register)
        case ir.Imm(width=width):
            return width if width in (2, 4) else None
        case _:
            return None


def _remapped(register: Register_, where: dict[Register_, Register_] | None) -> Register_:
    return (where or {}).get(register, register)


def _operand(one: ir.Loc, where: dict[Register_, Register_] | None, held: dict | None = None) -> ir.Loc:
    """One operand with every register in it remapped.

    Every register: a cell is reached by one as much as an accumulator is
    held in one. Remapping only the register operands is what made an
    allocation unwritable -- moving a value out of si rewrote `mov si,0`
    and left `[si+0Ah]` behind it reading a register nothing had set, and
    segld printed 0 for 1050 with every host test passing.
    """
    # A Held names a value, not a register, and the allocation says which
    # register that is. Resolved before the remap below, because what comes
    # out is an ordinary register operand from there on.
    if isinstance(one, ir.Held):
        got = (held or {}).get(one.value)
        if got is None:
            return one  # emit() refuses it; see its own note
        wide = AT_WIDTH.get(ir.ROOT.get(got, got), {}).get(one.width, got)
        one = ir.Reg(register=wide, width=one.width)
    if not where:
        return one
    if isinstance(one, ir.Reg):
        return replace(one, register=_remapped(one.register, where))
    if isinstance(one, (ir.Mem, ir.Address)):
        moved = {
            name: _remapped(getattr(one, name), where)
            for name in ("through", "index")
            if getattr(one, name, None) is not None
        }
        # And inside the address itself, which is what the operand is built
        # from: `through` says which register a cell is reached by, and
        # Addr.base is the one that gets encoded. Remapping only the first
        # changed nothing at all and did it silently.
        addr = getattr(one, "addr", None)
        if addr is not None and getattr(addr, "base", Register.NONE) is not Register.NONE:
            moved["addr"] = replace(addr, base=_remapped(addr.base, where))
        return replace(one, **moved) if moved else one
    return one


def load(into: Register_, value: int, at: int = 0) -> Emitted | None:
    """`mov into, imm`, at the width `into` names."""
    width = WIDTHS.get(into)
    if width is None:
        return None
    code = _code(f"MOV_R{width * 8}_IMM{width * 8}")
    if code is None:
        return None
    try:
        return _assemble(Instruction.create_reg_i32(code, into, value), at)
    except (ValueError, OverflowError):
        return None


def arith(name: str, dest: Register_, source: Register_, at: int = 0) -> Emitted | None:
    """`<name> dest, source`, both registers, at the width they name."""
    if name not in TWO_OPERAND:
        return None
    width = WIDTHS.get(dest)
    if width is None or WIDTHS.get(source) != width:
        return None  # a mixed-width operation is a different instruction
    code = _code(f"{name.upper()}_R{width * 8}_RM{width * 8}")
    if code is None:
        return None
    return _assemble(Instruction.create_reg_reg(code, dest, source), at)


def fits_in_a_byte(value: int) -> bool:
    """Whether the sign-extended one-byte form says the same number.

    x86 has a short encoding for most immediates where the value fits a
    signed byte, and it is two bytes narrower at 32 bits. `sub eax,5` is
    seven bytes as IMM32 and four as IMM8, which is worth taking: nothing
    about layout depends on it, since the choice is made from the value
    alone and not from any address.
    """
    return -128 <= value <= 127


# The accumulator's own arithmetic opcodes, which need no modrm byte and so
# are a byte shorter than the general form. iced spells them ADD_AX_IMM16 and
# ADD_EAX_IMM32 rather than by the RM pattern the rest of this table follows.
ACCUMULATOR = {2: Register.AX, 4: Register.EAX}


def arith_imm(name: str, dest: Register_, value: int, at: int = 0, relocated: bool = False) -> Emitted | None:
    """`<name> dest, imm`, in the shortest form the value and register allow.

    Three encodings, tried shortest first. A sign-extended byte immediate is
    two bytes plus the modrm and works on any register. The accumulator's
    own opcode drops the modrm instead, so it is the same length for a small
    value and a byte shorter for a large one -- `add ax,1286h` is `05 86 12`
    against `81 c0 86 12`, and qb-qrender has 592 of those. Everything else
    takes the general form.

    `relocated` keeps the full width, the same reason push_imm has it: an
    immediate a fixup names has to stay the size the fixup expects.
    """
    if name not in TWO_OPERAND:
        return None
    width = WIDTHS.get(dest)
    if width is None:
        return None
    shapes: list[tuple[str, int]] = []
    if fits_in_a_byte(value) and not relocated:
        shapes.append((f"{name.upper()}_RM{width * 8}_IMM8", 8))
    if dest is ACCUMULATOR.get(width):
        shapes.append((f"{name.upper()}_{'AX' if width == 2 else 'EAX'}_IMM{width * 8}", width * 8))
    shapes.append((f"{name.upper()}_RM{width * 8}_IMM{width * 8}", width * 8))
    for shape, _bits in shapes:
        code = _code(shape)
        if code is None:
            continue
        try:
            return _assemble(Instruction.create_reg_i32(code, dest, value), at)
        except (ValueError, OverflowError):
            continue
    return None


def unary(name: str, dest: Register_, at: int = 0) -> Emitted | None:
    """`neg`, `not`, `inc` or `dec` of one register.

    inc and dec of a register have a one-byte encoding -- the whole opcode
    is `40+r` -- where the general RM form takes two. Preferred, because it
    is 384 bytes across the corpus's own bodies and costs nothing to take:
    the choice is made from the operand and not from any address.
    """
    if name not in ONE_OPERAND:
        return None
    width = WIDTHS.get(dest)
    if width is None:
        return None
    for shape in (f"{name.upper()}_R{width * 8}", f"{name.upper()}_RM{width * 8}"):
        code = _code(shape)
        if code is None:
            continue
        try:
            return _assemble(Instruction.create_reg(code, dest), at)
        except (ValueError, OverflowError):
            continue
    return None


# Two functions rather than one taking either, because iced's Register_ IS an
# int -- Register.EAX is the number 37 -- so `isinstance(what, int)` is true of
# a register and a one-function version pushed 0x25 where it meant `push eax`.
def push(one: Register_, at: int = 0) -> Emitted | None:
    """A register onto the stack, at the width it names."""
    width = WIDTHS.get(one)
    if width is None:
        return None
    code = _code(f"PUSH_R{width * 8}")
    return None if code is None else _assemble(Instruction.create_reg(code, one), at)


def push_imm(value: int, width: int = 2, at: int = 0, relocated: bool = False) -> Emitted | None:
    """A literal onto the stack, at the width the operand names.

    The width is not decoration: `push 3` puts two bytes on the stack and
    `pushd 3` puts four, and a caller that pops a dword after the first one
    reads two bytes of whatever was under it. iced spells the wide one
    PUSHD_IMM32, breaking the PUSH_* pattern the rest of this table follows,
    which is why it is named here rather than built from the width.

    `relocated` is the caller saying a fixup names this immediate, and it
    keeps the wide form. BC writes `push offset X` as `68 00 00` with the
    address filled in at link time, so the operand arrives here as
    Imm(value=0) -- indistinguishable from a real `push 0` until someone
    who can see the fixups says so. Shrinking one leaves a two-byte
    relocation pointing at a one-byte field.
    """
    # Both widths have a one-byte-immediate form, and each pushes its own
    # width from a sign-extended byte: PUSHD_IMM8 four, PUSHW_IMM8 two. This
    # used to claim the narrow one did not exist and shrank only the wide
    # one, which cost a byte at every `push 0` BC writes -- 513 of them in
    # qb-qrender. fixtures/omf could not show it, because the suite pushes
    # addresses and long literals rather than small constants.
    names = ["PUSHD_IMM32"] if width == 4 else ["PUSH_IMM16"]
    if fits_in_a_byte(value) and not relocated:
        names.insert(0, "PUSHD_IMM8" if width == 4 else "PUSHW_IMM8")
    for one in names:
        code = _code(one)
        if code is None:
            continue
        try:
            return _assemble(Instruction.create_i32(code, value), at)
        except (ValueError, OverflowError):
            continue
    return None


# The accumulator's own load and store against a bare address -- `a1 xxxx`
# rather than `8b 06 xxxx`, one byte shorter and available to ax/eax alone.
# 470 bytes across the corpus's bodies, which is the largest single reason a
# laid-out body was bigger than BC's.
ACCUMULATOR = {1: Register.AL, 2: Register.AX, 4: Register.EAX}
MOFFS_LOAD = {1: "MOV_AL_MOFFS8", 2: "MOV_AX_MOFFS16", 4: "MOV_EAX_MOFFS32"}
MOFFS_STORE = {1: "MOV_MOFFS8_AL", 2: "MOV_MOFFS16_AX", 4: "MOV_MOFFS32_EAX"}


def _moffs(shape: dict[int, str], register: Register_, cell: ir.Mem, width: int) -> Code_ | None:
    """The accumulator form, where this is one it applies to.

    Only for a bare displacement: the encoding has no modrm byte, so there
    is nowhere to put a base register even when the address has one.
    """
    if register is not ACCUMULATOR.get(width) or cell.addr is None or cell.addr.base != Register.NONE:
        return None
    return _code(shape.get(width, ""))


def move_from(into: Register_, cell: ir.Mem, at: int = 0) -> Emitted | None:
    """`mov into, [cell]`."""
    width = WIDTHS.get(into)
    built = operand_of(cell)
    if width is None or built is None or cell.width != width:
        return None
    where, _relocated = built
    short = _moffs(MOFFS_LOAD, into, cell, width)
    if short is not None:
        made = _assemble(Instruction.create_reg_mem(short, into, where), at)
        if made is not None:
            return made
    code = _code(f"MOV_R{width * 8}_RM{width * 8}")
    if code is None:
        return None
    return _assemble(Instruction.create_reg_mem(code, into, where), at)


def move_into(cell: ir.Mem, outof: Register_, at: int = 0) -> Emitted | None:
    """`mov [cell], outof`."""
    width = WIDTHS.get(outof)
    built = operand_of(cell)
    if width is None or built is None or cell.width != width:
        return None
    where, _relocated = built
    short = _moffs(MOFFS_STORE, outof, cell, width)
    if short is not None:
        made = _assemble(Instruction.create_mem_reg(short, where, outof), at)
        if made is not None:
            return made
    code = _code(f"MOV_RM{width * 8}_R{width * 8}")
    if code is None:
        return None
    return _assemble(Instruction.create_mem_reg(code, where, outof), at)


def store_imm(cell: ir.Mem, value: int, at: int = 0) -> Emitted | None:
    """`mov [cell], imm`, at the cell's own width.

    The width is the cell's and not the value's: `mov word ptr [x],0` and
    `mov dword ptr [x],0` write two bytes and four, and the immediate says
    nothing about which was meant.
    """
    built = operand_of(cell)
    if built is None or cell.width not in (2, 4):
        return None
    code = _code(f"MOV_RM{cell.width * 8}_IMM{cell.width * 8}")
    if code is None:
        return None
    where, _relocated = built
    try:
        return _assemble(Instruction.create_mem_i32(code, where, value), at)
    except (ValueError, OverflowError):
        return None


def arith_mem(name: str, dest: Register_, cell: ir.Mem, at: int = 0) -> Emitted | None:
    """`<name> dest, [cell]`."""
    width = WIDTHS.get(dest)
    built = operand_of(cell)
    if name not in TWO_OPERAND or width is None or built is None or cell.width != width:
        return None
    code = _code(f"{name.upper()}_R{width * 8}_RM{width * 8}")
    if code is None:
        return None
    where, _relocated = built
    return _assemble(Instruction.create_reg_mem(code, dest, where), at)


def push_mem(cell: ir.Mem, at: int = 0) -> Emitted | None:
    """`push [cell]`, at the cell's own width."""
    built = operand_of(cell)
    if built is None or cell.width not in (2, 4):
        return None
    code = _code(f"PUSH_RM{cell.width * 8}")
    if code is None:
        return None
    where, _relocated = built
    return _assemble(Instruction.create_mem(code, where), at)


def branch(name: str, target: int, at: int = 0, short: bool = False) -> Emitted | None:
    """`<name> target`, a conditional branch to an absolute address.

    `short` asks for the two-byte form, which reaches -128..127 from the end
    of the instruction. Whether it reaches is a question about addresses, so
    only a caller laying a body out can answer it -- layout.py does, by
    starting every branch long and shrinking to a fixed point.
    """
    code = _code(f"{name.upper()}_REL8_16" if short else f"{name.upper()}_REL16")
    if code is None:
        return None
    try:
        return _assemble(Instruction.create_branch(code, target), at)
    except (ValueError, OverflowError):
        return None


def jump(target: int, at: int = 0, short: bool = False) -> Emitted | None:
    """`jmp target`, near unless the short form is asked for."""
    code = _code("JMP_REL8_16" if short else "JMP_REL16")
    if code is None:
        return None
    try:
        return _assemble(Instruction.create_branch(code, target), at)
    except (ValueError, OverflowError):
        return None


def call_near(target: int, at: int = 0) -> Emitted | None:
    """`call target`, within this segment.

    BC emits one of these for a procedure in the same module and a far call
    for everything in the runtime, and `ir.Semantics.target` is what tells
    them apart: a near call names an address, a far one does not have it to
    name.
    """
    code = _code("CALL_REL16")
    if code is None:
        return None
    try:
        return _assemble(Instruction.create_branch(code, target), at)
    except (ValueError, OverflowError):
        return None


def jump_far(at: int = 0) -> Emitted | None:
    """`jmp far ptr 0:0`, the far end of a body BC jumps out of.

    The same shape as a far call and for the same reason: the target is not
    in the code, the four bytes are zero and a fixup names where it goes.
    """
    return Emitted(bytes([0xEA, 0, 0, 0, 0]), 1)


def call_far(at: int = 0) -> Emitted | None:
    """`call far ptr 0:0`, the shape BC emits for every runtime call.

    The target is not in the code and never was: the four bytes are zero and
    a fixup names the routine, exactly as a relocated memory displacement
    works. So this emits the shape and says where the field is, and the
    caller moves the fixup that fills it.
    """
    made = bytes([0x9A, 0, 0, 0, 0])
    return Emitted(made, 1)


# The no-operand and one-operand forms that carry no address and no target,
# named here because iced spells each of them differently enough that the
# width-and-mnemonic rule the rest of this table follows does not reach them.
BARE = {
    "wait": "WAIT",
    "nop": "NOPW",
    "ret": "RETNW",
    "retf": "RETFW",
    "cwd": "CWD",
    "cdq": "CDQ",
    # PDS /Ot closes a procedure with it: `mov sp,bp` then `pop bp` in one
    # byte. extent.py names that difference; this is the encoding of it.
    "leave": "LEAVEW",
    # the x87 ones that take no operand at all
    "fsqrt": "FSQRT",
    "fchs": "FCHS",
    "fabs": "FABS",
    "fld1": "FLD1",
    "fldz": "FLDZ",
    "fcompp": "FCOMPP",
}


def bare(name: str, at: int = 0) -> Emitted | None:
    """An instruction with no operands at all."""
    code = _code(BARE.get(name, ""))
    if code is None:
        return None
    return _assemble(Instruction.create(code), at)


def pop(into: Register_, at: int = 0) -> Emitted | None:
    """`pop into`, at the width it names."""
    width = WIDTHS.get(into)
    if width is None:
        return None
    code = _code(f"POP_R{width * 8}")
    return None if code is None else _assemble(Instruction.create_reg(code, into), at)


def ret_far(popped: int, at: int = 0) -> Emitted | None:
    """`retf n`, which is how every BC procedure ends."""
    code = _code("RETFW_IMM16" if popped else "RETFW")
    if code is None:
        return None
    made = Instruction.create_i32(code, popped) if popped else Instruction.create(code)
    return _assemble(made, at)


def compare(dest: ir.Loc, value: int, at: int = 0, relocated: bool = False) -> Emitted | None:
    """`cmp <dest>, imm`. Flags are the whole result, so there is no dest."""
    match dest:
        case ir.Reg(register=register) if value == 0 and not relocated:
            # `test reg,reg` asks the same question a byte shorter: both set
            # SF, ZF and PF from the value and clear CF and OF, so every
            # conditional jump reads the same answer, and test carries no
            # immediate. BC never writes `cmp reg,0` -- these are absorption's
            # own, so this is a peephole on this pass's output.
            #
            # Not when relocated: `cmp ax,offset X` arrives here as
            # `cmp ax,0` the same way `add ax,offset X` does, and testing ax
            # would ask about ax rather than about the address.
            made = compare_registers("test", register, register, at)
            if made is not None:
                return made
            return arith_imm("cmp", register, value, at, relocated)
        case ir.Reg(register=register):
            return arith_imm("cmp", register, value, at, relocated)
        case ir.Mem() as cell:
            # The same shape arith_into_imm already emits, byte immediate
            # first. This used to hardcode the word one, so every `cmp`
            # against a frame slot cost a byte -- 209 of them in qb-qrender.
            # A compare writes no destination, which is the only thing that
            # made it look like a different instruction.
            return arith_into_imm("cmp", cell, value, at, relocated)
        case _:
            return None


# The x87 forms that take a memory operand. Named by whether the bytes are a
# float or an integer, which is the distinction the opcode makes and the one
# the mnemonic already carries: fld reads a float, fild an integer.
FLOAT_SIZED = {4: "M32FP", 8: "M64FP", 10: "M80FP"}
INT_SIZED = {2: "M16INT", 4: "M32INT", 8: "M64INT"}
FLOAT_MEMORY = ("fld", "fstp", "fst", "fadd", "fsub", "fmul", "fdiv", "fsubr", "fdivr", "fcom", "fcomp")
INT_MEMORY = ("fild", "fistp", "fist", "fiadd", "fisub", "fimul", "fidiv")


def float_memory(name: str, cell: ir.Mem, at: int = 0) -> Emitted | None:
    """An x87 instruction against memory -- `fld [x]`, `fmul [x]`, `fistp [x]`.

    The stack operand is not encoded: every one of these is implicitly about
    st(0), which is why ir.St carries an index and this does not need it.
    What the opcode does carry is whether the bytes are a float or an
    integer, and the mnemonic already says which.
    """
    sized = INT_SIZED if name in INT_MEMORY else FLOAT_SIZED if name in FLOAT_MEMORY else None
    if sized is None:
        return None
    built = operand_of(cell)
    suffix = sized.get(cell.width)
    if built is None or suffix is None:
        return None
    code = _code(f"{name.upper()}_{suffix}")
    if code is None:
        return None
    where, _relocated = built
    return _assemble(Instruction.create_mem(code, where), at)


def arith_into(name: str, cell: ir.Mem, source: Register_, at: int = 0) -> Emitted | None:
    """`<name> [cell], source` -- the accumulate whose destination is memory."""
    width = WIDTHS.get(source)
    built = operand_of(cell)
    if name not in TWO_OPERAND or width is None or built is None or cell.width != width:
        return None
    code = _code(f"{name.upper()}_RM{width * 8}_R{width * 8}")
    if code is None:
        return None
    where, _relocated = built
    return _assemble(Instruction.create_mem_reg(code, where, source), at)


# calls.py's own restore idiom, byte for byte: push the root, pop the two
# halves back. It is one node with no single instruction behind it, so it is
# named here rather than selected -- there is nothing to choose.
RESTORE = {
    0: bytes([0x66, 0x50, 0x58, 0x5A]),  # push eax / pop ax / pop dx
    1: bytes([0x66, 0x51, 0x59, 0x5B]),  # push ecx / pop cx / pop bx
}


def restore(pair: int) -> Emitted | None:
    """The idiom that puts a widened value's halves back where BC reads them."""
    made = RESTORE.get(pair)
    return None if made is None else Emitted(made)


def divide(name: str, divisor: Register_, at: int = 0) -> Emitted | None:
    """`idiv` or `div` by a register.

    One named operand: the dividend is edx:eax and the quotient and
    remainder come back in them, which is why ir.Semantics gives this two
    dests and three sources and none of them are encoded.
    """
    if name not in ("idiv", "div"):
        return None
    width = WIDTHS.get(divisor)
    if width is None:
        return None
    code = _code(f"{name.upper()}_RM{width * 8}")
    return None if code is None else _assemble(Instruction.create_reg(code, divisor), at)


def address_of(into: Register_, cell: ir.Address, at: int = 0) -> Emitted | None:
    """`lea into,[cell]` -- the address as a value, reading no memory.

    Built from the operand's own parts rather than from a named address,
    because `lea` is where an address becomes arithmetic: calls.py writes
    `lea eax,[eax+eax*2]` for a multiply by three and there is no address
    to name.
    """
    width = WIDTHS.get(into)
    if width is None:
        return None
    code = _code(f"LEA_R{width * 8}_M")
    if code is None:
        return None
    if cell.addr is not None and cell.index == Register.NONE:
        built = operand_of(ir.Mem(cell.addr, width))
        if built is None:
            return None
        return _assemble(Instruction.create_reg_mem(code, into, built[0]), at)
    if cell.through == Register.NONE and cell.index == Register.NONE:
        return None
    where = MemoryOperand(
        base=cell.through,
        index=cell.index,
        scale=cell.scale,
        displ=cell.offset,
        displ_size=cell.disp_width or _displacement_size(cell.through, cell.offset),
    )
    return _assemble(Instruction.create_reg_mem(code, into, where), at)


def compare_registers(name: str, one: Register_, other: Register_, at: int = 0) -> Emitted | None:
    """`cmp a,b` or `test a,b` -- both flags-only, and not the same question.

    `cmp` sets the flags from a-b and `test` from a AND b, so `test ax,ax`
    asks about the value's own sign and zero where `cmp ax,ax` answers
    "equal" no matter what is in it. They share ir.Operation.COMPARE and are
    told apart by the mnemonic, which is what Semantics.name is for.

    TEST has only the `85 /r` shape -- there is no `test r,rm` distinct from
    `test rm,r` -- so it is named here rather than built from the pattern
    the rest of this table follows.
    """
    if name == "cmp":
        return arith("cmp", one, other, at)
    if name != "test":
        return None
    width = WIDTHS.get(one)
    if width is None or WIDTHS.get(other) != width:
        return None
    code = _code(f"TEST_RM{width * 8}_R{width * 8}")
    return None if code is None else _assemble(Instruction.create_reg_reg(code, one, other), at)


def compare_mem(dest: Register_, cell: ir.Mem, at: int = 0) -> Emitted | None:
    """`cmp dest,[cell]`. Flags are the whole result, so there is no dest."""
    return arith_mem("cmp", dest, cell, at)


# The segment registers, which are not values -- mir.PHYSICAL keeps them
# out -- and which an instruction still names. BC pushes cs to build a far
# return address.
SEGMENTS = {Register.CS: "CS", Register.DS: "DS", Register.ES: "ES", Register.SS: "SS"}


def push_segment(one: Register_, width: int = 2, at: int = 0) -> Emitted | None:
    """`push cs` and its kind, at the width the push actually moves."""
    named = SEGMENTS.get(one)
    if named is None:
        return None
    code = _code(f"PUSH{'D' if width == 4 else 'W'}_{named}")
    # Named even though the opcode implies it: iced wants the operand.
    return None if code is None else _assemble(Instruction.create_reg(code, one), at)


def pop_segment(one: Register_, width: int = 2, at: int = 0) -> Emitted | None:
    """`pop es` and its kind. BC saves es around a far-pointer access."""
    named = SEGMENTS.get(one)
    if named is None or one is Register.CS:
        return None  # popping cs is not an instruction on anything after the 8086
    code = _code(f"POP{'D' if width == 4 else 'W'}_{named}")
    return None if code is None else _assemble(Instruction.create_reg(code, one), at)


# The string stores BC emits to clear an array. Nothing about them is
# encoded in operands -- es:di is the destination, cx the count, and the
# opcode names it all -- so iced builds them from the width alone.
STRING = {
    "stosb": Instruction.create_stosb,
    "stosw": Instruction.create_stosw,
    "stosd": Instruction.create_stosd,
}


def fill(name: str, at: int = 0, repeated: bool = True) -> Emitted | None:
    """`rep stosw` and its kind, at this pass's own 16-bit address size."""
    make = STRING.get(name)
    if make is None:
        return None
    return _assemble(make(BITNESS, RepPrefixKind.REPE) if repeated else make(BITNESS), at)


# The shifts and rotates, all of which take a count that is 1, an immediate
# byte, or cl -- never a general register.
SHIFTS = ("shl", "shr", "sar", "rol", "ror", "rcl", "rcr", "sal")


def shift(name: str, dest: Register_, count: int | None, at: int = 0) -> Emitted | None:
    """`shl reg,imm` and its kind. `count` of None means by cl."""
    if name not in SHIFTS:
        return None
    width = WIDTHS.get(dest)
    if width is None:
        return None
    if count is None:
        code = _code(f"{name.upper()}_RM{width * 8}_CL")
        return None if code is None else _assemble(Instruction.create_reg_reg(code, dest, Register.CL), at)
    # `shl reg,1` has its own opcode, a byte shorter than the immediate form
    # and what BC writes for a doubling. iced models the implicit 1 as a real
    # operand, so it is built with the count like the immediate form -- with
    # create_reg it comes out `shl ax,???` and the assembler refuses it,
    # which is how this shape sat in the table emitting nothing.
    for shape, build in (
        (f"{name.upper()}_RM{width * 8}_1", lambda c: Instruction.create_reg_i32(c, dest, count)),
        (f"{name.upper()}_RM{width * 8}_IMM8", lambda c: Instruction.create_reg_i32(c, dest, count)),
    ):
        if shape.endswith("_1") and count != 1:
            continue
        code = _code(shape)
        if code is None:
            continue
        made = _assemble(build(code), at)
        if made is not None:
            return made
    return None


# The popping x87 arithmetic: `faddp st(1),st(0)` and its kind. Both
# operands are stack positions and the first is the only one encoded.
FLOAT_POP = ("faddp", "fsubp", "fmulp", "fdivp", "fsubrp", "fdivrp")
STACK_REGISTERS = (
    Register.ST0,
    Register.ST1,
    Register.ST2,
    Register.ST3,
    Register.ST4,
    Register.ST5,
    Register.ST6,
    Register.ST7,
)


def float_pop(name: str, index: int, at: int = 0) -> Emitted | None:
    """`faddp st(i),st(0)` -- the arithmetic that pops its own operand."""
    if name not in FLOAT_POP or not 0 <= index < len(STACK_REGISTERS):
        return None
    code = _code(f"{name.upper()}_STI_ST0")
    if code is None:
        return None
    return _assemble(Instruction.create_reg_reg(code, STACK_REGISTERS[index], Register.ST0), at)


def divide_mem(name: str, cell: ir.Mem, at: int = 0) -> Emitted | None:
    """`idiv [x]` -- the divisor in memory rather than a register."""
    if name not in ("idiv", "div"):
        return None
    built = operand_of(cell)
    if built is None or cell.width not in (2, 4):
        return None
    code = _code(f"{name.upper()}_RM{cell.width * 8}")
    return None if code is None else _assemble(Instruction.create_mem(code, built[0]), at)


def multiply(name: str, source: Register_ | ir.Mem, at: int = 0) -> Emitted | None:
    """The one-operand `imul`/`mul`, whose result is dx:ax and is not encoded."""
    if name not in ("imul", "mul"):
        return None
    if isinstance(source, ir.Mem):
        built = operand_of(source)
        if built is None or source.width not in (2, 4):
            return None
        code = _code(f"{name.upper()}_RM{source.width * 8}")
        return None if code is None else _assemble(Instruction.create_mem(code, built[0]), at)
    width = WIDTHS.get(source)
    if width is None:
        return None
    code = _code(f"{name.upper()}_RM{width * 8}")
    return None if code is None else _assemble(Instruction.create_reg(code, source), at)


def multiply_into(dest: Register_, source: Register_ | ir.Mem, value: int | None = None, at: int = 0) -> Emitted | None:
    """`imul eax,ecx` and `imul ax,[x],3` -- the forms that name their result.

    Unlike the one-operand widening multiply, these say where the product
    goes and produce only the low half. This is what absorption emits for a
    long multiply, so a selector without it cannot re-emit qbopt's own
    output.
    """
    width = WIDTHS.get(dest)
    if width is None:
        return None
    if isinstance(source, ir.Mem):
        built = operand_of(source)
        if built is None or source.width != width:
            return None
        if value is None:
            code = _code(f"IMUL_R{width * 8}_RM{width * 8}")
            return None if code is None else _assemble(Instruction.create_reg_mem(code, dest, built[0]), at)
        bits = 8 if fits_in_a_byte(value) else width * 8
        code = _code(f"IMUL_R{width * 8}_RM{width * 8}_IMM{bits}")
        return None if code is None else _assemble(Instruction.create_reg_mem_i32(code, dest, built[0], value), at)
    if WIDTHS.get(source) != width:
        return None
    if value is None:
        code = _code(f"IMUL_R{width * 8}_RM{width * 8}")
        return None if code is None else _assemble(Instruction.create_reg_reg(code, dest, source), at)
    bits = 8 if fits_in_a_byte(value) else width * 8
    code = _code(f"IMUL_R{width * 8}_RM{width * 8}_IMM{bits}")
    return None if code is None else _assemble(Instruction.create_reg_reg_i32(code, dest, source, value), at)


def move_segment(into: Register_, outof: Register_ | ir.Mem, at: int = 0) -> Emitted | None:
    """`mov es,[si+2]` and `mov [x],es` -- how a far pointer is loaded.

    A segment register is not a value mir.py tracks, and it is still where a
    $DYNAMIC array's base lives: qb-qrender does this 614 times.
    """
    if into in SEGMENTS:
        code = _code("MOV_SREG_RM16")
        if code is None:
            return None
        if isinstance(outof, ir.Mem):
            built = operand_of(outof)
            return None if built is None else _assemble(Instruction.create_reg_mem(code, into, built[0]), at)
        return _assemble(Instruction.create_reg_reg(code, into, outof), at)
    code = _code("MOV_RM16_SREG")
    if code is None or not isinstance(outof, Register_ | int) or outof not in SEGMENTS:
        return None
    return _assemble(Instruction.create_reg_reg(code, into, outof), at)


def store_segment(cell: ir.Mem, outof: Register_, at: int = 0) -> Emitted | None:
    """`mov [bx+2],ds` -- half a far pointer written out."""
    if outof not in SEGMENTS:
        return None
    built = operand_of(cell)
    code = _code("MOV_RM16_SREG")
    if built is None or code is None:
        return None
    return _assemble(Instruction.create_mem_reg(code, built[0], outof), at)


def arith_into_imm(name: str, cell: ir.Mem, value: int, at: int = 0, relocated: bool = False) -> Emitted | None:
    """`add word ptr [bp-16h],4` -- accumulate into memory.

    `relocated` keeps the immediate its full width, for the same reason
    push_imm and arith_imm have it: a fixup may name the immediate rather
    than the displacement, and this cannot tell which from a bool. Keeping
    the wide form costs a byte where the displacement was the relocated one
    and is the only answer that is right in both cases.
    """
    built = operand_of(cell)
    if name not in TWO_OPERAND or built is None or cell.width not in (2, 4):
        return None
    for bits in ((8, cell.width * 8) if fits_in_a_byte(value) and not relocated else (cell.width * 8,)):
        code = _code(f"{name.upper()}_RM{cell.width * 8}_IMM{bits}")
        if code is None:
            continue
        try:
            return _assemble(Instruction.create_mem_i32(code, built[0], value), at)
        except (ValueError, OverflowError):
            continue
    return None


def exchange(one: Register_, other: Register_, at: int = 0) -> Emitted | None:
    """`xchg cx,ax`, which has a one-byte form against the accumulator."""
    width = WIDTHS.get(one)
    if width is None or WIDTHS.get(other) != width:
        return None
    for shape in (f"XCHG_R{width * 8}_{'AX' if width == 2 else 'EAX'}", f"XCHG_RM{width * 8}_R{width * 8}"):
        code = _code(shape)
        if code is None:
            continue
        made = _assemble(Instruction.create_reg_reg(code, one, other), at)
        if made is not None:
            return made
    return None


def unary_mem(name: str, cell: ir.Mem, at: int = 0) -> Emitted | None:
    """`neg`, `not`, `inc` or `dec` of a memory cell."""
    if name not in ONE_OPERAND:
        return None
    built = operand_of(cell)
    if built is None or cell.width not in (2, 4):
        return None
    code = _code(f"{name.upper()}_RM{cell.width * 8}")
    return None if code is None else _assemble(Instruction.create_mem(code, built[0]), at)


def emit(
    what: ir.Semantics,
    at: int = 0,
    where: dict[Register_, Register_] | tuple[dict, dict] | None = None,
    short: bool = False,
    relocated: bool = False,
    held: dict | None = None,
) -> Emitted | None:
    """One MIR operation as machine bytes, or None where this cannot say it.

    Driven by ir.Semantics rather than by an instruction, which is what makes
    it selection rather than re-encoding: the input names what to compute and
    in which locations, and nothing about how BC happened to write it.

    Register and immediate operands only, for now. A memory operand carries
    an address, and an address carries a fixup that has to move with it --
    that is the layout half of M1 and is not this function's yet.
    """
    # Once, over every operand, rather than at each register site below:
    # the cases that take a cell never reached _remapped at all, and the
    # segment moves reached it for neither of theirs.
    if where or held:
        # By side. `mov ax,1` defines one value and preserves another in the
        # same register, and one map cannot say two things about eax.
        into, outof = where if isinstance(where, tuple) else (where, where)
        what = replace(
            what,
            dests=tuple(_operand(one, into, held) for one in what.dests),
            sources=tuple(_operand(one, outof, held) for one in what.sources),
        )
        where = None

    # A Held that reached here is one the allocation had no register for.
    # Refusing is the whole point: it is a value a pass asked to be
    # somewhere and nothing decided where, and guessing a register is how
    # the five attempts in the history produced wrong programs.
    if any(isinstance(one, ir.Held) for one in (*what.dests, *what.sources)):
        return None

    dests, sources = what.dests, what.sources
    match what.op:
        case ir.Operation.MOVE if len(dests) == 1 and len(sources) == 1:
            match (dests[0], sources[0]):
                # Before the plain register move, which cannot say a
                # segment register and would refuse `mov ax,es`.
                case (ir.Reg(register=into), ir.Mem() as cell) if into in SEGMENTS:
                    return move_segment(into, cell, at)
                case (ir.Mem() as cell, ir.Reg(register=outof)) if outof in SEGMENTS:
                    return store_segment(cell, outof, at)
                case (ir.Reg(register=into), ir.Reg(register=outof)) if into in SEGMENTS or outof in SEGMENTS:
                    return move_segment(into, outof, at)
                case (ir.Reg(register=into), ir.Reg(register=outof)):
                    return move(into, outof, at)
                case (ir.Reg(register=into), ir.Imm(value=value)):
                    return load(into, value, at)
                case (ir.Reg(register=into), ir.Mem() as cell):
                    return move_from(into, cell, at)
                case (ir.Mem() as cell, ir.Reg(register=outof)):
                    return move_into(cell, outof, at)
                case (ir.Mem() as cell, ir.Imm(value=value)):
                    return store_imm(cell, value, at)
        case ir.Operation.BINARY if (what.name or "") in SHIFTS and len(dests) == 1 and len(sources) == 2:
            match (dests[0], sources[1]):
                case (ir.Reg(register=into), ir.Imm(value=count)):
                    return shift(what.name or "", into, count, at)
                case (ir.Reg(register=into), ir.Reg(register=Register.CL)):
                    return shift(what.name or "", into, None, at)
        case ir.Operation.BINARY if len(dests) == 1 and len(sources) == 2:
            # BINARY's own rule: sources[0] IS dests[0].
            match (dests[0], sources[1]):
                case (ir.Reg(register=into), ir.Reg(register=outof)):
                    return arith(what.name or "", into, outof, at)
                case (ir.Reg(register=into), ir.Imm(value=value)):
                    return arith_imm(what.name or "", into, value, at, relocated)
                case (ir.Reg(register=into), ir.Mem() as cell):
                    return arith_mem(what.name or "", into, cell, at)
                case (ir.Mem() as cell, ir.Reg(register=outof)):
                    return arith_into(what.name or "", cell, outof, at)
                case (ir.Mem() as cell, ir.Imm(value=value)):
                    return arith_into_imm(what.name or "", cell, value, at, relocated)
        case ir.Operation.UNARY if len(dests) == 1 and len(sources) == 1:
            match dests[0]:
                case ir.Reg(register=into):
                    return unary(what.name or "", into, at)
                case ir.Mem() as cell:
                    return unary_mem(what.name or "", cell, at)
        case ir.Operation.PUSH if len(sources) == 1:
            match sources[0]:
                case ir.Reg(register=one) if one in SEGMENTS:
                    return push_segment(one, sources[0].width, at)
                case ir.Reg(register=one):
                    return push(one, at)
                case ir.Imm(value=value, width=width):
                    return push_imm(value, width, at, relocated)
                case ir.Mem() as cell:
                    return push_mem(cell, at)
        case ir.Operation.BRANCH if what.target is not None:
            return branch(what.name or "", what.target, at, short)
        case ir.Operation.JUMP if what.target is not None:
            return jump(what.target, at, short)
        case ir.Operation.CALL:
            return call_far(at) if what.target is None else call_near(what.target, at)
        case ir.Operation.ESCAPE if what.target is None:
            return jump_far(at)
        case ir.Operation.FLOAT_LOAD | ir.Operation.FLOAT_ARITH if sources:
            match sources[-1]:
                case ir.Mem() as cell:
                    return float_memory(what.name or "", cell, at)
        case ir.Operation.FLOAT_STORE if len(dests) == 1:
            match dests[0]:
                case ir.Mem() as cell:
                    return float_memory(what.name or "", cell, at)
        case ir.Operation.EXCHANGE if len(dests) == 2:
            match (dests[0], dests[1]):
                case (ir.Reg(register=one), ir.Reg(register=other)):
                    return exchange(one, other, at)
        case ir.Operation.COMPARE if len(sources) == 2:
            match (sources[0], sources[1]):
                case (_, ir.Imm(value=value)) if (what.name or "cmp") == "cmp":
                    return compare(sources[0], value, at, relocated)
                case (ir.Reg(register=into), ir.Mem() as cell) if (what.name or "cmp") == "cmp":
                    return compare_mem(into, cell, at)
                case (ir.Reg(register=into), ir.Reg(register=outof)):
                    return compare_registers(
                        what.name or "cmp", into, outof, at
                    )
                case (ir.Mem() as cell, ir.Reg(register=outof)):
                    # Only `cmp` here: `test` against memory has its own
                    # shapes and BC writes none of them, so this refuses
                    # rather than emitting the wrong comparison.
                    if (what.name or "cmp") != "cmp":
                        return None
                    return arith_into("cmp", cell, outof, at)
        case ir.Operation.MULTIPLY if len(dests) == 1 and len(sources) >= 2:
            # One destination is the naming form: `imul eax,ecx`, and with a
            # third source `imul ax,[x],3`.
            count = sources[2].value if len(sources) > 2 and isinstance(sources[2], ir.Imm) else None
            match (dests[0], sources[1]):
                case (ir.Reg(register=into), ir.Reg(register=one)):
                    return multiply_into(into, one, count, at)
                case (ir.Reg(register=into), ir.Mem() as cell):
                    return multiply_into(into, cell, count, at)
                case (ir.Reg(register=into), ir.Imm(value=only)):
                    return multiply_into(into, into, only, at)
        case ir.Operation.MULTIPLY if len(dests) == 2 and sources:
            # Two destinations means the widening form: dx:ax, neither
            # encoded. The three-operand `imul r,rm,imm` has one.
            match sources[-1]:
                case ir.Reg(register=one):
                    return multiply(what.name or "", one, at)
                case ir.Mem() as cell:
                    return multiply(what.name or "", cell, at)
        case ir.Operation.FLOAT_ARITH_POP if dests:
            match dests[0]:
                case ir.St(index=index):
                    return float_pop(what.name or "", index, at)
        case ir.Operation.DIVIDE if sources:
            match sources[-1]:
                case ir.Reg(register=one):
                    return divide(what.name or "", one, at)
                case ir.Mem() as cell:
                    return divide_mem(what.name or "", cell, at)
        case ir.Operation.ADDRESS if len(dests) == 1 and len(sources) == 1:
            match (dests[0], sources[0]):
                case (ir.Reg(register=into), ir.Address() as cell):
                    return address_of(into, cell, at)
        case ir.Operation.FILL:
            return fill(what.name or "", at)
        case ir.Operation.EXTEND | ir.Operation.NOTHING | ir.Operation.LEAVE:
            return bare(what.name or "", at)
        case ir.Operation.FLOAT_UNARY | ir.Operation.FLOAT_ARITH if not any(
            isinstance(one, ir.Mem) for one in dests + sources
        ):
            # `fsqrt` and its kind name st(0) in both dests and sources, and
            # encode neither: the operand is implicit. So what decides is
            # whether any of it is memory, not whether there are operands.
            return bare(what.name or "", at)
        case ir.Operation.POP if len(dests) == 1:
            match dests[0]:
                case ir.Reg(register=into) if into in SEGMENTS:
                    return pop_segment(into, dests[0].width, at)
                case ir.Reg(register=into):
                    return pop(into, at)
        case ir.Operation.RETURN:
            if not sources:
                return bare("ret", at) if what.name == "ret" else ret_far(0, at)
            match sources[0]:
                case ir.Imm(value=value):
                    return ret_far(value, at)
    return None
