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

from qbopt.model import ir
from qbopt.backend import target
from qbopt.frontend.declen import BITNESS
from qbopt.objectfile.module import Space

# The 32-bit roots this can name, and their 16-bit halves.
# The six mir.py tracks, plus bp and sp. Those two are not values -- they
# are where values live -- but they are still registers an instruction
# names, and every procedure opens `push bp / mov bp,sp` and closes by
# popping it back. A selector that could not say them could not emit a
# prologue.


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
    # Where each relocatable field landed, in the order the instructions
    # were emitted. One entry for one instruction, which is every case but
    # an idiom: absorbing a long divide is `mov eax,[a] / mov ecx,[b] / cdq
    # / idiv ecx` and two of those four carry a fixup. `relocated_at` is the
    # first and is what a caller with one wants.
    fields: tuple[int, ...] = ()
    # Whether a displacement here can be a symbol at all. `displacement_at`
    # stays factual -- it says where the field is, not what names it -- and
    # a frame slot's displacement is an offset from bp that no fixup may be
    # bound to. Default True so an Emitted built from its own bytes keeps
    # saying what it always said.
    symbolic: bool = True

    @property
    def relocated_at(self) -> int | None:
        """Where this instruction's own relocatable field landed."""
        if self.fields:
            return self.fields[0]
        if not self.symbolic:
            return None
        return self.displacement_at if self.displacement_at is not None else self.immediate_at

    @property
    def places(self) -> tuple[int, ...]:
        """Every field, however this was built.

        Not every Emitted comes from _assemble -- an idiom is built from
        its own bytes and says where its fields are, and the plain ones
        report the two offsets iced read back. This is the one answer a
        caller wants and works for both.
        """
        if self.fields:
            return self.fields
        one = self.relocated_at
        return () if one is None else (one,)


def _assemble(made: Instruction, at: int, symbolic: bool = True) -> Emitted | None:
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
    # A displacement is a relocatable field only where the operand names a
    # symbol -- `operand_of` says which, and it is the address's own space
    # and not the register it is reached through. A frame slot's
    # displacement is an offset from bp, so binding a fixup to it writes a
    # segment address over the slot number: lngmix, with both its divides
    # hoisted, spilled the accumulator and turned `add cx,[a]` into a load
    # and `add [bp-22h],bx`, and the fixup that named `a` went to the add's
    # own displacement. An array element is symbolic and keeps its fixup
    # though it is reached through si.
    absolute = offsets.has_displacement and symbolic
    where = offsets.displacement_offset if absolute else offsets.immediate_offset if offsets.has_immediate else None
    return Emitted(
        code,
        offsets.displacement_offset if offsets.has_displacement else None,
        offsets.immediate_offset if offsets.has_immediate else None,
        () if where is None else (where,),
        symbolic=symbolic,
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
    if what.index is not None:
        return _scaled_operand(what)
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
        case Space.SEGMENT | Space.EXTERNAL:
            # `addr.base` is the register BC wrote the element through, and
            # it is the answer only while nothing has recomputed the offset.
            # Once a pass makes a value of it the allocation places it, and
            # `through` is where it went -- encoding BC's own register then
            # reads element zero through whatever si happens to hold.
            base = what.through if what.base is not None else addr.base
            return MemoryOperand(base=base, displ=0, displ_size=2, seg=addr.segment), True
        case Space.FRAME if addr.base == Register.NONE:
            index = what.index_through
            if index != Register.NONE and index not in _WORD_INDEXES:
                return None
            return (
                MemoryOperand(
                    base=Register.BP,
                    index=index,
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
            # `addr.base` is the register BC computed the offset into, and
            # holds only while nothing has recomputed it; once a pass makes
            # a value of it the allocation answers with `through`. The
            # width follows the register actually encoded.
            base = what.through if what.base is not None else addr.base
            return (
                MemoryOperand(
                    base=base,
                    displ=addr.disp,
                    displ_size=_displacement_size(base, addr.disp),
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
            # `addr.base` is BC's own register and holds only while nothing
            # has recomputed the offset; once a pass makes a value of it,
            # `through` is where the allocation put it. The width follows
            # the register actually encoded -- asking the other one gives
            # the collision above for a base that is not there.
            base = what.through if what.base is not None else addr.base
            wide = 2 if base == Register.NONE else _displacement_size(base, addr.disp)
            return MemoryOperand(base=base, displ=addr.disp, displ_size=wide, seg=addr.segment), False
        case _:
            return None


_WORD_BASES = frozenset({Register.BX, Register.BP})
_WORD_INDEXES = frozenset({Register.SI, Register.DI})


def _scaled_operand(what: ir.Mem) -> tuple[MemoryOperand, bool] | None:
    """`[base+index*scale+disp]`, for a cell no fixup names.

    A word index is 16-bit addressing, `[bx+si]`, which wraps the way the
    word arithmetic it replaces does; a dword index is 32-bit addressing.
    A relocated address carries a 16-bit fixup a 32-bit displacement field
    would not hold, so only a far cell or a literal one.
    """
    addr = what.addr
    if addr is None or addr.space not in (Space.FAR, Space.LITERAL) or what.index_through == Register.NONE:
        return None
    if addr.space is Space.FAR and addr.segment == Register.NONE:
        return None
    base, disp = what.through, addr.disp
    segment = addr.segment if addr.space in (Space.FAR, Space.LITERAL) else Register.NONE
    if what.index_through in _WORD_INDEXES:
        if what.scale != 1 or base not in _WORD_BASES:
            return None
        size = _displacement_size(base, disp)
        return MemoryOperand(base=base, index=what.index_through, displ=disp, displ_size=size, seg=segment), False
    if base == Register.NONE:
        size = 4
    elif disp == 0 and base != Register.EBP:
        size = 0
    else:
        size = 1 if -128 <= disp <= 127 else 4
    operand = MemoryOperand(
        base=base, index=what.index_through, scale=what.scale, displ=disp, displ_size=size, seg=segment
    )
    return operand, False


def move(into: Register_, outof: Register_, at: int = 0) -> Emitted | None:
    """`mov into, outof`, or None if this cannot name that pair.

    Both registers at one width, and a width this knows. A move between
    different widths is a different instruction with different semantics --
    zero or sign extension -- and guessing which was meant is exactly what
    this refuses to do.
    """
    if into is outof:
        return Emitted(b"")  # a move to itself is no instruction at all
    if into in target.WIDE and outof in target.WIDE:
        code = Code.MOV_R32_RM32
    elif into in target.NARROW and outof in target.NARROW:
        code = Code.MOV_R16_RM16
    elif into in target.BYTE and outof in target.BYTE:
        code = Code.MOV_R8_RM8
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

# The register file's own tables. This module had a second copy of both,
# and regalloc.py a third of AT_WIDTH covering only two widths -- reading
# the two as duplicates and keeping the narrower one broke every object in
# the corpus, because an ir.Held of width 1 then resolved to its root.
WIDTHS = target.WIDTHS
AT_WIDTH = target.AT_WIDTH


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


def _immediate(value: int, width: int) -> int:
    """Signed at its width: a word's -1 arrives as 65535 as often as -1."""
    if width not in (2, 4):
        return value
    sign = 1 << (8 * width - 1)
    return ((value & (2 * sign - 1)) ^ sign) - sign


def load(into: Register_, value: int, at: int = 0) -> Emitted | None:
    """`mov into, imm`, at the width `into` names."""
    width = WIDTHS.get(into)
    if width is None:
        return None
    code = _code(f"MOV_R{width * 8}_IMM{width * 8}")
    if code is None:
        return None
    try:
        return _assemble(Instruction.create_reg_i32(code, into, _immediate(value, width)), at)
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
    value = _immediate(value, width)
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
    value = _immediate(value, width)
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
    where, relocated = built
    short = _moffs(MOFFS_LOAD, into, cell, width)
    if short is not None:
        made = _assemble(Instruction.create_reg_mem(short, into, where), at, relocated)
        if made is not None:
            return made
    code = _code(f"MOV_R{width * 8}_RM{width * 8}")
    if code is None:
        return None
    return _assemble(Instruction.create_reg_mem(code, into, where), at, relocated)


def move_into(cell: ir.Mem, outof: Register_, at: int = 0) -> Emitted | None:
    """`mov [cell], outof`."""
    width = WIDTHS.get(outof)
    built = operand_of(cell)
    if width is None or built is None or cell.width != width:
        return None
    where, relocated = built
    short = _moffs(MOFFS_STORE, outof, cell, width)
    if short is not None:
        made = _assemble(Instruction.create_mem_reg(short, where, outof), at, relocated)
        if made is not None:
            return made
    code = _code(f"MOV_RM{width * 8}_R{width * 8}")
    if code is None:
        return None
    return _assemble(Instruction.create_mem_reg(code, where, outof), at, relocated)


def store_imm(cell: ir.Mem, value: int, at: int = 0) -> Emitted | None:
    """`mov [cell], imm`, at the cell's own width.

    The width is the cell's and not the value's: `mov byte ptr [x],0`,
    `mov word ptr [x],0` and `mov dword ptr [x],0` write one, two and four
    bytes, and the immediate says nothing about which was meant. Deedlines'
    COPPER procedure stores a folded zero into `es:[si]`; excluding the byte
    form here left an ordinary x86 instruction unselectable at 0x32e0.
    """
    built = operand_of(cell)
    if built is None or cell.width not in (1, 2, 4):
        return None
    code = _code(f"MOV_RM{cell.width * 8}_IMM{cell.width * 8}")
    if code is None:
        return None
    where, relocated = built
    try:
        return _assemble(Instruction.create_mem_i32(code, where, _immediate(value, cell.width)), at, relocated)
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
    where, relocated = built
    return _assemble(Instruction.create_reg_mem(code, dest, where), at, relocated)


def push_mem(cell: ir.Mem, at: int = 0) -> Emitted | None:
    """`push [cell]`, at the cell's own width."""
    built = operand_of(cell)
    if built is None or cell.width not in (2, 4):
        return None
    code = _code(f"PUSH_RM{cell.width * 8}")
    if code is None:
        return None
    where, relocated = built
    return _assemble(Instruction.create_mem(code, where), at, relocated)


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
    "sahf": "SAHF",
}
# The x87 control and status words, each against a word of memory.
CONTROL_WORD = {"fldcw": "FLDCW_M2BYTE", "fnstcw": "FNSTCW_M2BYTE"}


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


def test_immediate(dest: ir.Loc, value: int, at: int = 0) -> Emitted | None:
    if not isinstance(dest, (ir.Reg, ir.Mem)) or dest.width not in (1, 2, 4):
        return None
    code = _code(f"TEST_RM{dest.width * 8}_IMM{dest.width * 8}")
    if code is None:
        return None
    value = _immediate(value, dest.width)
    match dest:
        case ir.Reg(register=register):
            return _assemble(Instruction.create_reg_i32(code, register, value), at)
        case ir.Mem() as cell:
            built = operand_of(cell)
            if built is not None:
                return _assemble(Instruction.create_mem_i32(code, built[0], value), at, built[1])
    return None


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
INT_MEMORY = ("fild", "fistp", "fist", "fiadd", "fisub", "fimul", "fidiv", "fisubr", "fidivr")


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
    where, relocated = built
    return _assemble(Instruction.create_mem(code, where), at, relocated)


def arith_into(name: str, cell: ir.Mem, source: Register_, at: int = 0) -> Emitted | None:
    """`<name> [cell], source` -- the accumulate whose destination is memory."""
    width = WIDTHS.get(source)
    built = operand_of(cell)
    if name not in TWO_OPERAND or width is None or built is None or cell.width != width:
        return None
    code = _code(f"{name.upper()}_RM{width * 8}_R{width * 8}")
    if code is None:
        return None
    where, relocated = built
    return _assemble(Instruction.create_mem_reg(code, where, source), at, relocated)


# calls.py's own restore idiom, byte for byte: push the root, pop the two
# halves back. It is one node with no single instruction behind it, so it is
# named here rather than selected -- there is nothing to choose.
def restore_of(wide: Register_, low: Register_, high: Register_, at: int = 0) -> Emitted | None:
    """`push wide / pop low / pop high` -- whichever three registers those are.

    One encoder. The pair table below is two of its answers rather than
    the only ones: which registers a restore names is the allocation's to
    decide, and a number standing for them said otherwise.
    """
    if WIDTHS.get(wide) != 4 or WIDTHS.get(low) != 2 or WIDTHS.get(high) != 2:
        return None
    push, pop = _code("PUSH_R32"), _code("POP_R16")
    if push is None or pop is None:
        return None
    out = bytearray()
    for code, register in ((push, wide), (pop, low), (pop, high)):
        made = _assemble(Instruction.create_reg(code, register), at + len(out))
        if made is None:
            return None
        out += made.code
    return Emitted(bytes(out))


# calls.py's own restore idiom: push the root, pop the two halves back.
# Built from the encoder above rather than written out again, so the pair
# a node names and the registers an allocation chose cannot disagree.
RESTORE = {
    pair: made.code
    for pair, made in (
        (0, restore_of(Register.EAX, Register.AX, Register.DX)),
        (1, restore_of(Register.ECX, Register.CX, Register.BX)),
    )
    if made is not None
}


def absorbed(site, live, restore: bool = True) -> "Emitted | str":
    """One absorbable runtime call as the instructions that replace it.

    Four of them for a long divide -- `mov eax,[a] / mov ecx,[b] / cdq /
    idiv ecx` -- and two carry a fixup, which is why Emitted reports a field
    per instruction rather than one. `fields` comes back in the order the
    instructions were emitted, and the fixups the caller pairs them with are
    in `absorbed_fixups`.

    calls.py builds the sequence and this is the seam: emission belongs
    here, and the machine arm is what phase D retires. Imported inside the
    function until it is, so nothing above this layer picks calls.py up.
    """
    from qbopt.legacy import calls as machine

    made = machine.absorb(site, live, restore)
    if isinstance(made, str):
        return made
    return Emitted(made.code, fields=tuple(where for where, _field in made.relocations))


def divides(op, seats: "tuple[Register_, Register_]", restore: bool = True) -> "Emitted | str":
    """A divide emitted from the operation's own operands.

    The same instructions `absorbed()` produces, built from a different
    thing: the MIR operation rather than the CallSite frozen at the raise.
    That is the whole difference and the point of it -- a pass that rewrites
    an operand rewrites what is emitted, where a frozen site goes on
    emitting what BC pushed however the operation has since changed.

    `fields` names one offset per operand that reads memory, in the order
    the instructions are emitted. A caller pairing them with the fixups the
    raise recorded has to check the counts still agree: an operand a pass
    has turned into a register carries no fixup, and pairing what is left
    in order would put the dividend's relocation on the divisor.
    """
    from qbopt.model import mir
    from qbopt.legacy import calls as machine

    if op.kind is not mir.Kind.DIVMOD or len(op.args) != 2:
        return f"{op.name}: not a divide over two operands"

    steps: list[Instruction] = []
    reads: list[int] = []  # which steps carry a relocatable field

    def into(where: Register_, one) -> str | None:
        """`where` given whatever this operand is, or why it cannot be."""
        if isinstance(one, mir.Const):
            steps.append(Instruction.create_reg_i32(Code.MOV_R32_IMM32, where, one.n))
            return None
        if isinstance(one, mir.Cell) and one.ref.addr is not None:
            # The same encoding the machine arm picks, from the same
            # helpers: the accumulator's moffs form has no ModRM byte and
            # is a byte shorter, and building the general form here made
            # this sequence one byte longer than the site it replaces.
            base = one.ref.addr.base
            code = Code.MOV_EAX_MOFFS32 if base == Register.NONE and where == machine.RESULT else Code.MOV_R32_RM32
            reads.append(len(steps))
            steps.append(Instruction.create_reg_mem(code, where, machine.relocated_memory(base)))
            return None
        # A value in a register is where SSA substitution arrives, and it
        # needs the allocation to say which register that is. Refused until
        # the assignment reaches here, so a substituted operand falls back
        # to the site's own bytes rather than being emitted from a guess.
        return f"{one} is not an operand a divide can read yet"

    for where, one in ((machine.RESULT, op.args[0]), (machine.DIVISOR, op.args[1])):
        refused = into(where, one)
        if refused is not None:
            return refused

    steps.append(Instruction.create(Code.CDQ))
    steps.append(Instruction.create_reg(Code.IDIV_RM32, machine.DIVISOR))
    # idiv leaves the quotient in eax and the remainder in edx, and the
    # operation says which of its two results is which -- so where each
    # goes is `seats`, and nothing here has to know which runtime routine
    # BC called.
    #
    # Two moves that happen at once, which is what parcopy.py says about a
    # phi's edge and is true here for the same reason: an allocation that
    # wants the quotient in edx and the remainder in eax makes each move's
    # destination the other's source, and writing them in either order
    # destroys one. Ordered where one is free, exchanged where neither is.
    quotient, remainder = seats
    moves = [(quotient, machine.RESULT), (remainder, Register.EDX)]
    moves = [(into, outof) for into, outof in moves if into != outof]
    if len(moves) == 2 and moves[0][0] == moves[1][1] and moves[1][0] == moves[0][1]:
        steps.append(Instruction.create_reg_reg(Code.XCHG_RM32_R32, moves[0][0], moves[0][1]))
    else:
        # Whichever move nothing else reads out of, first.
        if len(moves) == 2 and moves[0][0] == moves[1][1]:
            moves.reverse()
        for into, outof in moves:
            steps.append(Instruction.create_reg_reg(Code.MOV_R32_RM32, into, outof))
    if restore:
        steps.extend(machine.restoring())
    # Each reading step named as its own fixup, so what comes back is the
    # offsets alone: which fixup belongs to which is the caller's, and it
    # is the one thing this must not decide.
    made = machine.assemble(steps, {index: index for index in reads})
    return Emitted(made.code, fields=tuple(where for where, _which in made.relocations))


def absorbed_fixups(site, live, restore: bool = True) -> tuple[int, ...]:
    """Which fixup each of an absorbed site's fields names, in the same order.

    The same relocations `absorbed` reads the offsets from, so the two
    cannot disagree about how many there are or which is which.
    """
    from qbopt.legacy import calls as machine

    made = machine.absorb(site, live, restore)
    return () if isinstance(made, str) else tuple(field for _where, field in made.relocations)


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
        return _assemble(Instruction.create_reg_mem(code, into, built[0]), at, built[1])
    if cell.through == Register.NONE and cell.index == Register.NONE:
        return None
    where = MemoryOperand(
        base=cell.through,
        index=cell.index,
        scale=cell.scale,
        displ=cell.offset,
        displ_size=cell.disp_width or _displacement_size(cell.through, cell.offset),
    )
    # No address to name, so the displacement is arithmetic and not a symbol.
    return _assemble(Instruction.create_reg_mem(code, into, where), at, False)


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
SEGMENTS = {
    Register.CS: "CS",
    Register.DS: "DS",
    Register.ES: "ES",
    Register.SS: "SS",
    Register.FS: "FS",
    Register.GS: "GS",
}


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


def funnel(name: str, dest: Register_, other: Register_, count: int | None, at: int = 0) -> Emitted | None:
    """`shld`/`shrd dest,other,count` -- two registers shifted as one number.

    `count` of None means cl, which is the only register the instruction
    can take a count from. Both operands are 32-bit: there is a 16-bit
    form, and nothing here has a use for it.
    """
    if name not in {"shld", "shrd"} or WIDTHS.get(dest) != 4 or WIDTHS.get(other) != 4:
        return None
    opcode = name.upper()
    if count is None:
        code = _code(f"{opcode}_RM32_R32_CL")
        if code is None:
            return None
        return _assemble(Instruction.create_reg_reg_reg(code, dest, other, Register.CL), at)
    code = _code(f"{opcode}_RM32_R32_IMM8")
    if code is None:
        return None
    return _assemble(Instruction.create_reg_reg_i32(code, dest, other, count), at)


def shift(name: str, dest: Register_ | ir.Mem, count: int | None, at: int = 0) -> Emitted | None:
    """Shift a register or spill cell. `count` of None means by cl."""
    if name not in SHIFTS:
        return None
    memory = isinstance(dest, ir.Mem)
    built = operand_of(dest) if memory else None
    if memory and built is None:
        return None
    width = dest.width if memory else WIDTHS.get(dest)
    if width is None:
        return None
    if count is None:
        code = _code(f"{name.upper()}_RM{width * 8}_CL")
        if code is None:
            return None
        instruction = (
            Instruction.create_mem_reg(code, built[0], Register.CL)
            if memory
            else Instruction.create_reg_reg(code, dest, Register.CL)
        )
        return _assemble(instruction, at, built[1] if memory else False)
    # `shl reg,1` has its own opcode, a byte shorter than the immediate form
    # and what BC writes for a doubling. iced models the implicit 1 as a real
    # operand, so it is built with the count like the immediate form -- with
    # create_reg it comes out `shl ax,???` and the assembler refuses it,
    # which is how this shape sat in the table emitting nothing.
    for shape in (f"{name.upper()}_RM{width * 8}_1", f"{name.upper()}_RM{width * 8}_IMM8"):
        if shape.endswith("_1") and count != 1:
            continue
        code = _code(shape)
        if code is None:
            continue
        instruction = (
            Instruction.create_mem_i32(code, built[0], count)
            if memory
            else Instruction.create_reg_i32(code, dest, count)
        )
        made = _assemble(instruction, at, built[1] if memory else False)
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


def float_stack(what: ir.Semantics, at: int = 0) -> Emitted | None:
    """Select explicit stack operands without changing their evaluation order."""
    if any(
        not 0 <= operand.index < len(STACK_REGISTERS)
        for operand in (*what.dests, *what.sources)
        if isinstance(operand, ir.St)
    ):
        return None
    match what.op, what.name, what.dests, what.sources:
        case ir.Operation.FLOAT_LOAD, "fld", (ir.St(index=0),), (ir.St(index=index),):
            return _assemble(Instruction.create_reg(Code.FLD_STI, STACK_REGISTERS[index]), at)
        case ir.Operation.EXCHANGE, "fxch", (ir.St(index=0), ir.St(index=index)), sources if sources == what.dests:
            return _assemble(Instruction.create_reg_reg(Code.FXCH_ST0_STI, Register.ST0, STACK_REGISTERS[index]), at)
        case ir.Operation.FLOAT_ARITH, name, (ir.St(index=dest),), (left, ir.St(index=source)) if left == what.dests[0]:
            if name not in ("fadd", "fsub", "fsubr", "fmul", "fdiv", "fdivr") or (dest != 0 and source != 0):
                return None
            form = "ST0_STI" if dest == 0 else "STI_ST0"
            code = _code(f"{name.upper()}_{form}")
            return (
                None
                if code is None
                else _assemble(Instruction.create_reg_reg(code, STACK_REGISTERS[dest], STACK_REGISTERS[source]), at)
            )
    return None


def divide_mem(name: str, cell: ir.Mem, at: int = 0) -> Emitted | None:
    """`idiv [x]` -- the divisor in memory rather than a register."""
    if name not in ("idiv", "div"):
        return None
    built = operand_of(cell)
    if built is None or cell.width not in (2, 4):
        return None
    code = _code(f"{name.upper()}_RM{cell.width * 8}")
    return None if code is None else _assemble(Instruction.create_mem(code, built[0]), at, built[1])


def multiply(name: str, source: Register_ | ir.Mem, at: int = 0) -> Emitted | None:
    """The one-operand `imul`/`mul`, whose result is dx:ax and is not encoded."""
    if name not in ("imul", "mul"):
        return None
    if isinstance(source, ir.Mem):
        built = operand_of(source)
        if built is None or source.width not in (2, 4):
            return None
        code = _code(f"{name.upper()}_RM{source.width * 8}")
        return None if code is None else _assemble(Instruction.create_mem(code, built[0]), at, built[1])
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
            return None if code is None else _assemble(Instruction.create_reg_mem(code, dest, built[0]), at, built[1])
        bits = 8 if fits_in_a_byte(value) else width * 8
        code = _code(f"IMUL_R{width * 8}_RM{width * 8}_IMM{bits}")
        return (
            None
            if code is None
            else _assemble(Instruction.create_reg_mem_i32(code, dest, built[0], value), at, built[1])
        )
    if WIDTHS.get(source) != width:
        return None
    if value is None:
        code = _code(f"IMUL_R{width * 8}_RM{width * 8}")
        return None if code is None else _assemble(Instruction.create_reg_reg(code, dest, source), at)
    bits = 8 if fits_in_a_byte(value) else width * 8
    code = _code(f"IMUL_R{width * 8}_RM{width * 8}_IMM{bits}")
    return None if code is None else _assemble(Instruction.create_reg_reg_i32(code, dest, source, value), at)


# Each segment register a far pointer can be loaded into with its offset, and the instruction that does it.
FAR_LOADS = {segment: (name, f"{name.upper()}_R16_M1616") for segment, name in target.FAR_LOADS.items()}


def far_load(name: str, into: Register_, segment: Register_, cell: ir.Mem, at: int = 0) -> Emitted | None:
    """`les bx,[cell]`: a far pointer's offset word into `into` and its segment word into `segment`."""
    spelled, code_name = FAR_LOADS.get(segment, ("", ""))
    built = operand_of(cell)
    if name != spelled or built is None or cell.width != 4 or WIDTHS.get(into) != 2 or into in SEGMENTS:
        return None
    code = _code(code_name)
    if code is None:
        return None
    where, relocated = built
    return _assemble(Instruction.create_reg_mem(code, into, where), at, relocated)


def move_segment(into: Register_, outof: Register_ | ir.Mem, at: int = 0) -> Emitted | None:
    """`mov es,[si+2]` and `mov [x],es` -- how a far pointer is loaded.

    A segment register is not a value mir.py tracks, and it is still where a
    $DYNAMIC array's base lives: qb-qrender does this 614 times.
    """
    if into in SEGMENTS and not isinstance(outof, ir.Mem) and outof in SEGMENTS:
        # No `mov sreg,sreg`: through the stack, which writes no flags.
        pushed = push_segment(outof, 2, at)
        popped = None if pushed is None else pop_segment(into, 2, at + len(pushed.code))
        return None if popped is None else Emitted(pushed.code + popped.code)
    if into in SEGMENTS:
        code = _code("MOV_SREG_RM16")
        if code is None:
            return None
        if isinstance(outof, ir.Mem):
            built = operand_of(outof)
            return None if built is None else _assemble(Instruction.create_reg_mem(code, into, built[0]), at, built[1])
        return _assemble(Instruction.create_reg_reg(code, into, outof), at)
    code = _code("MOV_RM16_SREG")
    if code is None or not isinstance(outof, Register_ | int) or outof not in SEGMENTS:
        return None
    return _assemble(Instruction.create_reg_reg(code, into, outof), at)


# `pop cs` is 8086-only -- 80286 and later fault on it -- so cs is not here
# and a constant into cs has no form at all.
_POP_SEGMENT = {
    Register.DS: "POPW_DS",
    Register.ES: "POPW_ES",
    Register.SS: "POPW_SS",
    Register.FS: "POPW_FS",
    Register.GS: "POPW_GS",
}


def load_segment(into: Register_, value: int, at: int = 0) -> Emitted | None:
    """`push 0A000h / pop es` -- a constant into a segment register.

    There is no `mov sreg,imm`: a segment register loads only from r/m16.
    The other form is `mov r,imm / mov sreg,r`, which needs a scratch
    register, and selection runs after allocation with none to borrow.
    This needs none, is a byte shorter, and writes no flags.
    """
    named = _POP_SEGMENT.get(into)
    push = _code("PUSH_IMM16")
    if named is None or push is None:
        return None
    pop = _code(named)
    if pop is None:
        return None
    out = bytearray()
    for made in (Instruction.create_u32(push, value & 0xFFFF), Instruction.create_reg(pop, into)):
        got = _assemble(made, at + len(out))
        if got is None:
            return None
        out += got.code
    return Emitted(bytes(out))


def store_segment(cell: ir.Mem, outof: Register_, at: int = 0) -> Emitted | None:
    """`mov [bx+2],ds` -- half a far pointer written out."""
    if outof not in SEGMENTS:
        return None
    built = operand_of(cell)
    code = _code("MOV_RM16_SREG")
    if built is None or code is None:
        return None
    return _assemble(Instruction.create_mem_reg(code, built[0], outof), at, built[1])


def arith_into_imm(name: str, cell: ir.Mem, value: int, at: int = 0, relocated: bool = False) -> Emitted | None:
    """`add word ptr [bp-16h],4` -- accumulate into memory.

    `relocated` keeps the immediate its full width, for the same reason
    push_imm and arith_imm have it: a fixup may name the immediate rather
    than the displacement, and this cannot tell which from a bool. Keeping
    the wide form costs a byte where the displacement was the relocated one
    and is the only answer that is right in both cases.
    """
    built = operand_of(cell)
    if name not in TWO_OPERAND or built is None or cell.width not in (1, 2, 4):
        return None
    value = _immediate(value, cell.width)
    if cell.width == 1:
        value = ((value & 0xFF) ^ 0x80) - 0x80
    for bits in (8, cell.width * 8) if fits_in_a_byte(value) and not relocated else (cell.width * 8,):
        code = _code(f"{name.upper()}_RM{cell.width * 8}_IMM{bits}")
        if code is None:
            continue
        try:
            return _assemble(Instruction.create_mem_i32(code, built[0], value), at, built[1])
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


def exchange_mem(register: Register_, cell: ir.Mem, at: int = 0) -> Emitted | None:
    """Exchange a register with a same-width memory cell."""
    width = WIDTHS.get(register)
    built = operand_of(cell)
    if width not in (1, 2, 4) or cell.width != width or built is None:
        return None
    code = _code(f"XCHG_RM{width * 8}_R{width * 8}")
    return None if code is None else _assemble(Instruction.create_mem_reg(code, built[0], register), at, built[1])


def unary_mem(name: str, cell: ir.Mem, at: int = 0) -> Emitted | None:
    """`neg`, `not`, `inc` or `dec` of a memory cell."""
    if name not in ONE_OPERAND:
        return None
    built = operand_of(cell)
    if built is None or cell.width not in (2, 4):
        return None
    code = _code(f"{name.upper()}_RM{cell.width * 8}")
    return None if code is None else _assemble(Instruction.create_mem(code, built[0]), at, built[1])


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
    # A cell whose address value nothing placed. Refusing is the same rule
    # one line up: guessing a base register is how arrprm stored one array
    # element through another's address.
    if any(
        isinstance(one, ir.Mem) and one.base is not None and one.through == Register.NONE
        for one in (*what.dests, *what.sources)
    ):
        return None

    dests, sources = what.dests, what.sources
    if (
        what.op in (ir.Operation.FLOAT_LOAD, ir.Operation.FLOAT_ARITH, ir.Operation.EXCHANGE)
        and sources
        and all(isinstance(operand, ir.St) for operand in (*dests, *sources))
    ):
        return float_stack(what, at)
    match what.op:
        case ir.Operation.EXTEND if what.name in ("movsx", "movzx") and len(dests) == len(sources) == 1:
            match dests[0], sources[0]:
                case ir.Reg(register=into), ir.Reg(register=outof):
                    code = _code(f"{what.name.upper()}_R{WIDTHS.get(into, 0) * 8}_RM{WIDTHS.get(outof, 0) * 8}")
                    if code is not None and WIDTHS[into] > WIDTHS[outof]:
                        return _assemble(Instruction.create_reg_reg(code, into, outof), at)
                case ir.Reg(register=into), ir.Mem() as cell:
                    code = _code(f"{what.name.upper()}_R{WIDTHS.get(into, 0) * 8}_RM{cell.width * 8}")
                    built = operand_of(cell)
                    if code is not None and built is not None and WIDTHS[into] > cell.width:
                        return _assemble(Instruction.create_reg_mem(code, into, built[0]), at, built[1])
            return None
        case ir.Operation.MOVE if len(dests) == 2 and len(sources) == 1:
            match (dests[0], dests[1], sources[0]):
                case (ir.Reg(register=into), ir.Reg(register=segment), ir.Mem() as cell):
                    return far_load(what.name or "", into, segment, cell, at)
            return None
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
                # Before the plain immediate load, which would build the
                # `mov sreg,imm` the machine has no encoding for.
                case (ir.Reg(register=into), ir.Imm(value=value)) if into in SEGMENTS:
                    return load_segment(into, value, at)
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
        case ir.Operation.RESTORE if len(dests) == 2 and len(sources) == 1:
            match (sources[0], dests[0], dests[1]):
                case (ir.Reg(register=wide), ir.Reg(register=low), ir.Reg(register=high)):
                    return restore_of(wide, low, high, at)
        case ir.Operation.FUNNEL if len(dests) == 1 and len(sources) == 3:
            match (dests[0], sources[1], sources[2]):
                case (ir.Reg(register=into), ir.Reg(register=other), ir.Imm(value=count)):
                    return funnel(what.name or "", into, other, count, at)
                case (ir.Reg(register=into), ir.Reg(register=other), ir.Reg(register=Register.CL)):
                    return funnel(what.name or "", into, other, None, at)
        case ir.Operation.BINARY if (what.name or "") in SHIFTS and len(dests) == 1 and len(sources) == 2:
            match (dests[0], sources[1]):
                case (ir.Reg(register=into), ir.Imm(value=count)):
                    return shift(what.name or "", into, count, at)
                case (ir.Reg(register=into), ir.Reg(register=Register.CL)):
                    return shift(what.name or "", into, None, at)
                case (ir.Mem() as cell, ir.Imm(value=count)):
                    return shift(what.name or "", cell, count, at)
                case (ir.Mem() as cell, ir.Reg(register=Register.CL)):
                    return shift(what.name or "", cell, None, at)
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
        case ir.Operation.CALL if what.indirect and len(sources) == 1:
            match sources[0]:
                case ir.Reg(register=one, width=2):
                    return _assemble(Instruction.create_reg(Code.CALL_RM16, one), at)
                case ir.Mem(width=2) as cell:
                    built = operand_of(cell)
                    if built is not None:
                        return _assemble(Instruction.create_mem(Code.CALL_RM16, built[0]), at, built[1])
                case ir.Mem(width=4) as cell:
                    built = operand_of(cell)
                    if built is not None:
                        return _assemble(Instruction.create_mem(Code.CALL_M1616, built[0]), at, built[1])
        case ir.Operation.CALL:
            return call_far(at) if what.target is None else call_near(what.target, at)
        case ir.Operation.ESCAPE if what.target is None:
            return jump_far(at)
        case ir.Operation.FLOAT_LOAD if what.name in ("fldz", "fld1") and not sources and dests == (ir.St(0),):
            return _assemble(Instruction.create(Code.FLDZ if what.name == "fldz" else Code.FLD1), at)
        case ir.Operation.FLOAT_LOAD | ir.Operation.FLOAT_ARITH if sources:
            match sources[-1]:
                case ir.Mem() as cell:
                    return float_memory(what.name or "", cell, at)
        case ir.Operation.FLOAT_STORE if len(dests) == 1:
            match dests[0]:
                case ir.Mem() as cell:
                    return float_memory(what.name or "", cell, at)
                case ir.St(index=index) if what.name == "fstp" and sources == (ir.St(0),):
                    return _assemble(Instruction.create_reg(Code.FSTP_STI, STACK_REGISTERS[index]), at)
        case ir.Operation.EXCHANGE if len(dests) == 2:
            match (dests[0], dests[1]):
                case (ir.Reg(register=one), ir.Reg(register=other)):
                    return exchange(one, other, at)
                case (ir.Reg(register=one), ir.Mem() as cell) | (ir.Mem() as cell, ir.Reg(register=one)):
                    return exchange_mem(one, cell, at)
        case ir.Operation.COMPARE if (what.name or "").startswith("f"):
            # st(0) is implicit; what is encoded is the memory operand, if any.
            memory = [one for one in sources if isinstance(one, ir.Mem)]
            name = what.name or ""
            return float_memory(name, memory[0], at) if memory else bare(name, at)
        case ir.Operation.BARRIER if what.name == "fnstsw" and dests == (ir.Reg(Register.AX, 2),):
            return _assemble(Instruction.create_reg(Code.FNSTSW_AX, Register.AX), at)
        case ir.Operation.BARRIER if what.name in CONTROL_WORD and len(dests + sources) == 1:
            match (dests + sources)[0]:
                case ir.Mem(width=2) as cell:
                    built = operand_of(cell)
                    code = _code(CONTROL_WORD[what.name])
                    if built is not None and code is not None:
                        return _assemble(Instruction.create_mem(code, built[0]), at, built[1])
            return None
        case ir.Operation.COMPARE if len(sources) == 2:
            match (sources[0], sources[1]):
                case (_, ir.Imm(value=value)) if what.name == "test":
                    return test_immediate(sources[0], value, at)
                case (_, ir.Imm(value=value)) if (what.name or "cmp") == "cmp":
                    return compare(sources[0], value, at, relocated)
                case (ir.Reg(register=into), ir.Mem() as cell) if (what.name or "cmp") == "cmp":
                    return compare_mem(into, cell, at)
                case (ir.Reg(register=into), ir.Reg(register=outof)):
                    return compare_registers(what.name or "cmp", into, outof, at)
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
                    # The product of the first source, which is the
                    # destination only when something tied it there:
                    # `imul cx,[bp-0Eh],2` is not `imul cx,2`.
                    match sources[0]:
                        case ir.Reg(register=one):
                            return multiply_into(into, one, only, at)
                        case ir.Mem() as cell:
                            return multiply_into(into, cell, only, at)
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
        case ir.Operation.FILL if len(sources) in (3, 4):
            return fill(what.name or "", at, repeated=len(sources) == 4)
        case ir.Operation.NOTHING if not what.name:
            return Emitted(b"")
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
                case ir.Mem() as cell:
                    built = operand_of(cell)
                    code = _code(f"POP_RM{cell.width * 8}") if cell.width in (2, 4) else None
                    return (
                        None
                        if built is None or code is None
                        else _assemble(Instruction.create_mem(code, built[0]), at, built[1])
                    )
        case ir.Operation.RETURN:
            if not sources:
                return bare("ret", at) if what.name == "ret" else ret_far(0, at)
            match sources[0]:
                case ir.Imm(value=value):
                    return ret_far(value, at)
    return None
