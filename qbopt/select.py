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

from dataclasses import dataclass

from iced_x86 import Code
from iced_x86 import Code_
from iced_x86 import Decoder
from iced_x86 import Encoder
from iced_x86 import Register
from iced_x86 import Register_
from iced_x86 import Instruction
from iced_x86 import MemoryOperand

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
            wide = 2 if what.offset else 0
            return MemoryOperand(base=what.through, displ=what.offset, displ_size=wide), False
        return None
    match addr.space:
        case Space.SEGMENT:
            return MemoryOperand(base=addr.base, displ=0, displ_size=2), True
        case Space.FRAME if addr.base == Register.NONE:
            return MemoryOperand(base=Register.BP, displ=addr.disp, displ_size=2), False
        case Space.LITERAL:
            # A displacement no fixup claims, so the number in the code is
            # the address and nothing has to move with it. BC writes these
            # for the runtime's own fixed locations.
            return MemoryOperand(base=addr.base, displ=addr.disp, displ_size=2), False
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
for _row, _size in ((_WIDE, 4), (_NARROW, 2)):
    for _one in _row:
        WIDTHS[_one] = _size


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


def arith_imm(name: str, dest: Register_, value: int, at: int = 0) -> Emitted | None:
    """`<name> dest, imm`, in the shorter form where the value allows."""
    if name not in TWO_OPERAND:
        return None
    width = WIDTHS.get(dest)
    if width is None:
        return None
    for bits in (8, width * 8) if fits_in_a_byte(value) else (width * 8,):
        code = _code(f"{name.upper()}_RM{width * 8}_IMM{bits}")
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


def push_imm(value: int, width: int = 2, at: int = 0) -> Emitted | None:
    """A literal onto the stack, at the width the operand names.

    The width is not decoration: `push 3` puts two bytes on the stack and
    `pushd 3` puts four, and a caller that pops a dword after the first one
    reads two bytes of whatever was under it. iced spells the wide one
    PUSHD_IMM32, breaking the PUSH_* pattern the rest of this table follows,
    which is why it is named here rather than built from the width.
    """
    # PUSHD_IMM8 pushes four bytes from a one-byte immediate, so it is the
    # wide push in a narrower encoding rather than a narrow push. There is no
    # PUSH_IMM8 that pushes two, which is why only the wide one shrinks.
    names = ["PUSHD_IMM32"] if width == 4 else ["PUSH_IMM16"]
    if width == 4 and fits_in_a_byte(value):
        names.insert(0, "PUSHD_IMM8")
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
ACCUMULATOR = {2: Register.AX, 4: Register.EAX}
MOFFS_LOAD = {2: "MOV_AX_MOFFS16", 4: "MOV_EAX_MOFFS32"}
MOFFS_STORE = {2: "MOV_MOFFS16_AX", 4: "MOV_MOFFS32_EAX"}


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


def compare(dest: ir.Loc, value: int, at: int = 0) -> Emitted | None:
    """`cmp <dest>, imm`. Flags are the whole result, so there is no dest."""
    match dest:
        case ir.Reg(register=register):
            return arith_imm("cmp", register, value, at)
        case ir.Mem() as cell:
            built = operand_of(cell)
            if built is None or cell.width not in (2, 4):
                return None
            code = _code(f"CMP_RM{cell.width * 8}_IMM{cell.width * 8}")
            if code is None:
                return None
            where, _relocated = built
            try:
                return _assemble(Instruction.create_mem_i32(code, where, value), at)
            except (ValueError, OverflowError):
                return None
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
    """`lea into,[cell]` -- the address as a value, reading no memory."""
    width = WIDTHS.get(into)
    if width is None or cell.addr is None:
        return None
    built = operand_of(ir.Mem(cell.addr, width))
    if built is None:
        return None
    code = _code(f"LEA_R{width * 8}_M")
    if code is None:
        return None
    where, _relocated = built
    return _assemble(Instruction.create_reg_mem(code, into, where), at)


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


def emit(
    what: ir.Semantics,
    at: int = 0,
    where: dict[Register_, Register_] | None = None,
    short: bool = False,
) -> Emitted | None:
    """One MIR operation as machine bytes, or None where this cannot say it.

    Driven by ir.Semantics rather than by an instruction, which is what makes
    it selection rather than re-encoding: the input names what to compute and
    in which locations, and nothing about how BC happened to write it.

    Register and immediate operands only, for now. A memory operand carries
    an address, and an address carries a fixup that has to move with it --
    that is the layout half of M1 and is not this function's yet.
    """
    dests, sources = what.dests, what.sources
    match what.op:
        case ir.Operation.MOVE if len(dests) == 1 and len(sources) == 1:
            match (dests[0], sources[0]):
                case (ir.Reg(register=into), ir.Reg(register=outof)):
                    return move(_remapped(into, where), _remapped(outof, where), at)
                case (ir.Reg(register=into), ir.Imm(value=value)):
                    return load(_remapped(into, where), value, at)
                case (ir.Reg(register=into), ir.Mem() as cell):
                    return move_from(_remapped(into, where), cell, at)
                case (ir.Mem() as cell, ir.Reg(register=outof)):
                    return move_into(cell, _remapped(outof, where), at)
                case (ir.Mem() as cell, ir.Imm(value=value)):
                    return store_imm(cell, value, at)
        case ir.Operation.BINARY if len(dests) == 1 and len(sources) == 2:
            # BINARY's own rule: sources[0] IS dests[0].
            match (dests[0], sources[1]):
                case (ir.Reg(register=into), ir.Reg(register=outof)):
                    return arith(what.name or "", _remapped(into, where), _remapped(outof, where), at)
                case (ir.Reg(register=into), ir.Imm(value=value)):
                    return arith_imm(what.name or "", _remapped(into, where), value, at)
                case (ir.Reg(register=into), ir.Mem() as cell):
                    return arith_mem(what.name or "", _remapped(into, where), cell, at)
                case (ir.Mem() as cell, ir.Reg(register=outof)):
                    return arith_into(what.name or "", cell, _remapped(outof, where), at)
        case ir.Operation.UNARY if len(dests) == 1 and len(sources) == 1:
            match dests[0]:
                case ir.Reg(register=into):
                    return unary(what.name or "", _remapped(into, where), at)
        case ir.Operation.PUSH if len(sources) == 1:
            match sources[0]:
                case ir.Reg(register=one) if one in SEGMENTS:
                    return push_segment(one, sources[0].width, at)
                case ir.Reg(register=one):
                    return push(_remapped(one, where), at)
                case ir.Imm(value=value, width=width):
                    return push_imm(value, width, at)
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
        case ir.Operation.COMPARE if len(sources) == 2:
            match (sources[0], sources[1]):
                case (_, ir.Imm(value=value)):
                    return compare(sources[0], value, at)
                case (ir.Reg(register=into), ir.Mem() as cell):
                    return compare_mem(_remapped(into, where), cell, at)
                case (ir.Reg(register=into), ir.Reg(register=outof)):
                    return arith("cmp", _remapped(into, where), _remapped(outof, where), at)
                case (ir.Mem() as cell, ir.Reg(register=outof)):
                    return arith_into("cmp", cell, _remapped(outof, where), at)
        case ir.Operation.DIVIDE if sources:
            match sources[-1]:
                case ir.Reg(register=one):
                    return divide(what.name or "", _remapped(one, where), at)
        case ir.Operation.ADDRESS if len(dests) == 1 and len(sources) == 1:
            match (dests[0], sources[0]):
                case (ir.Reg(register=into), ir.Address() as cell):
                    return address_of(_remapped(into, where), cell, at)
        case ir.Operation.EXTEND | ir.Operation.NOTHING:
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
                case ir.Reg(register=into):
                    return pop(_remapped(into, where), at)
        case ir.Operation.RETURN:
            if not sources:
                return bare("ret", at) if what.name == "ret" else ret_far(0, at)
            match sources[0]:
                case ir.Imm(value=value):
                    return ret_far(value, at)
    return None
