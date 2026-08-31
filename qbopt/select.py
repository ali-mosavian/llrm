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
_WIDE = {Register.EAX, Register.ECX, Register.EDX, Register.EBX, Register.ESI, Register.EDI}
_NARROW = {Register.AX, Register.CX, Register.DX, Register.BX, Register.SI, Register.DI}


@dataclass(frozen=True, slots=True)
class Emitted:
    """The bytes, and where a relocated displacement ended up inside them.

    `displacement_at` is None for everything with no memory operand and for
    a frame slot, whose displacement is a real number in the code. A
    relocated address is emitted as zero and the caller has to move the
    fixup that names it -- LINK adds what is in the code to the fixup's
    target, so a displacement left in would be added to the real address.
    """

    code: bytes
    displacement_at: int | None = None


def _assemble(made: Instruction, at: int, relocated: bool = False) -> Emitted | None:
    encoder = Encoder(BITNESS)
    try:
        encoder.encode(made, at)
    except ValueError:
        return None
    code = encoder.take_buffer()
    if not relocated:
        return Emitted(code)
    # Read back off the encoded bytes rather than predicted, which is what
    # calls.py's own assemble() does and for the same reason.
    decoder = Decoder(BITNESS, code, ip=at)
    decoded = next(iter(decoder), None)
    if decoded is None:
        return None
    offsets = decoder.get_constant_offsets(decoded)
    return Emitted(code, offsets.displacement_offset if offsets.has_displacement else None)


def operand_of(what: ir.Mem) -> tuple[MemoryOperand, bool] | None:
    """`what` as an encodable memory operand, and whether it is relocated.

    Two spaces, which is 97.6% of the corpus's memory operands: a relocated
    segment address, emitted as zero with its fixup moved, and a frame slot,
    whose displacement really is in the code. Everything else is refused --
    a Space.FAR address needs a segment override this does not model, a
    Space.GROUP one is refused everywhere in this project, and a
    Space.STACK one is mir.py's own name for a push slot rather than
    anything an instruction encodes.
    """
    addr = what.addr
    if addr is None or addr.base != Register.NONE:
        return None
    match addr.space:
        case Space.SEGMENT:
            return MemoryOperand(displ=0, displ_size=2), True
        case Space.FRAME:
            return MemoryOperand(base=Register.BP, displ=addr.disp, displ_size=2), False
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


def arith_imm(name: str, dest: Register_, value: int, at: int = 0) -> Emitted | None:
    """`<name> dest, imm`."""
    if name not in TWO_OPERAND:
        return None
    width = WIDTHS.get(dest)
    if width is None:
        return None
    code = _code(f"{name.upper()}_RM{width * 8}_IMM{width * 8}")
    if code is None:
        return None
    try:
        return _assemble(Instruction.create_reg_i32(code, dest, value), at)
    except (ValueError, OverflowError):
        return None


def unary(name: str, dest: Register_, at: int = 0) -> Emitted | None:
    """`neg`, `not`, `inc` or `dec` of one register."""
    if name not in ONE_OPERAND:
        return None
    width = WIDTHS.get(dest)
    if width is None:
        return None
    code = _code(f"{name.upper()}_RM{width * 8}")
    if code is None:
        return None
    return _assemble(Instruction.create_reg(code, dest), at)


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
    code = _code("PUSHD_IMM32" if width == 4 else "PUSH_IMM16")
    if code is None:
        return None
    try:
        return _assemble(Instruction.create_i32(code, value), at)
    except (ValueError, OverflowError):
        return None


def move_from(into: Register_, cell: ir.Mem, at: int = 0) -> Emitted | None:
    """`mov into, [cell]`."""
    width = WIDTHS.get(into)
    built = operand_of(cell)
    if width is None or built is None or cell.width != width:
        return None
    code = _code(f"MOV_R{width * 8}_RM{width * 8}")
    if code is None:
        return None
    where, relocated = built
    return _assemble(Instruction.create_reg_mem(code, into, where), at, relocated)


def move_into(cell: ir.Mem, outof: Register_, at: int = 0) -> Emitted | None:
    """`mov [cell], outof`."""
    width = WIDTHS.get(outof)
    built = operand_of(cell)
    if width is None or built is None or cell.width != width:
        return None
    code = _code(f"MOV_RM{width * 8}_R{width * 8}")
    if code is None:
        return None
    where, relocated = built
    return _assemble(Instruction.create_mem_reg(code, where, outof), at, relocated)


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


def emit(what: ir.Semantics, at: int = 0, where: dict[Register_, Register_] | None = None) -> Emitted | None:
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
        case ir.Operation.BINARY if len(dests) == 1 and len(sources) == 2:
            # BINARY's own rule: sources[0] IS dests[0].
            match (dests[0], sources[1]):
                case (ir.Reg(register=into), ir.Reg(register=outof)):
                    return arith(what.name or "", _remapped(into, where), _remapped(outof, where), at)
                case (ir.Reg(register=into), ir.Imm(value=value)):
                    return arith_imm(what.name or "", _remapped(into, where), value, at)
                case (ir.Reg(register=into), ir.Mem() as cell):
                    return arith_mem(what.name or "", _remapped(into, where), cell, at)
        case ir.Operation.UNARY if len(dests) == 1 and len(sources) == 1:
            match dests[0]:
                case ir.Reg(register=into):
                    return unary(what.name or "", _remapped(into, where), at)
        case ir.Operation.PUSH if len(sources) == 1:
            match sources[0]:
                case ir.Reg(register=one):
                    return push(_remapped(one, where), at)
                case ir.Imm(value=value, width=width):
                    return push_imm(value, width, at)
                case ir.Mem() as cell:
                    return push_mem(cell, at)
    return None
