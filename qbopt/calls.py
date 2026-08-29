"""
The runtime calls, and what they can be replaced with.

`*`, `\\`, `MOD` and every comparison are calls into BC's runtime, and they are
what the widening cannot touch: the pass sees a far call and stops. In an object
the call site is a FIXUPP naming an EXTDEF, so which routine it is, is a lookup.

The order the arguments reach the stack is the thing to get right, and it is not
the same for both. Comparison pushes its left operand first; multiply, divide
and remainder push it second. That holds across all four configurations and is
opposite between the two routines, and getting it backwards is a different
answer rather than a crash.
"""

from enum import StrEnum
from dataclasses import replace
from dataclasses import dataclass

from iced_x86 import Code
from iced_x86 import Decoder
from iced_x86 import Encoder
from iced_x86 import Register
from iced_x86 import Instruction
from iced_x86 import BlockEncoder
from iced_x86 import MemoryOperand

from qbopt.flags import ALL
from qbopt.flags import Flag
from qbopt.lift import FIXUP
from qbopt.declen import Insn
from qbopt.module import Addr
from qbopt.lift import Emitted
from qbopt.module import Space
from qbopt.module import Module
from qbopt.declen import BITNESS
from qbopt.declen import to_signed

COMPARE = "B$CPI4"
MULTIPLY = "B$MUI4"
DIVIDE = "B$DVI4"
REMAINDER = "B$RMI4"

# A user-declared `declare function fixMul& (byval a as long, byval b as long,
# byval fixShift as long)` has no body anywhere -- LINK never sees it, because
# absorbing the call drops its only fixup. BC never emits a type suffix into
# the EXTDEF, so the name it writes is the identifier alone, uppercased the
# way every BASIC identifier is. Measured: BC pushes a `declare`d function's
# arguments in the order written, first argument first -- the runtime's own
# routines do not, which is what the module docstring above is about, and is
# unrelated to this one.
#
# The shift is a third argument rather than a fixed constant: N.M times N.M is
# N.2M, which does not fit back in 32 bits without the shift that undoes the
# doubled fraction, and the width of that fraction is the caller's format to
# choose, not this pass's to assume. fixShift is `as long` only so it reaches
# the stack the same way a and b do -- one_operand() already knows every shape
# that arrives in.
FIX_MULTIPLY = "FIXMUL"

# True where the left operand is pushed first. Uniform across the compilers,
# opposite between the two runtime routines. FIX_MULTIPLY is not the runtime's:
# it is pushed in the order written, which happens to agree with COMPARE's.
LEFT_FIRST = {COMPARE: True, MULTIPLY: False, DIVIDE: False, REMAINDER: False, FIX_MULTIPLY: True}

# Every routine here takes two long arguments except fixMul&, which takes three.
ARITY = {FIX_MULTIPLY: 3}

# one dword per argument under VBDOS /G3, two words everywhere else
PUSHES = {Code.PUSH_RM16, Code.PUSH_RM32}
CONSTANTS = {Code.PUSHW_IMM8, Code.PUSHD_IMM8, Code.PUSH_IMM16, Code.PUSHD_IMM32}
WIDE_PUSHES = {Code.PUSH_RM32, Code.PUSHD_IMM8, Code.PUSHD_IMM32}


class Kind(StrEnum):
    STATIC = "static"  # a bare displacement, whose address is a fixup
    CONSTANT = "constant"  # an immediate pushed straight to the stack


@dataclass(frozen=True, slots=True)
class Operand:
    kind: Kind
    addr: Addr | None = None
    # where the displacement field was, so its fixup can be reused
    at: int | None = None
    value: int = 0
    length: int = 0  # instructions it took to push


@dataclass(frozen=True, slots=True)
class CallSite:
    at: int  # the call instruction
    end: int
    start: int  # where the first push begins
    name: str
    pushed: tuple[Operand, ...]  # in the order they reach the stack

    @property
    def operands(self) -> tuple[Operand, Operand]:
        """(left, right), whichever way this routine takes them."""
        first, second = self.pushed
        return (first, second) if LEFT_FIRST[self.name] else (second, first)


def static_at(module: Module, insn: Insn) -> Operand | None:
    if insn.code not in PUSHES or insn.disp_at is None or insn.memory_base != Register.NONE:
        return None
    addr = module.operands.get(insn.disp_at)
    if addr is None or addr.space is not Space.SEGMENT:
        return None
    return Operand(Kind.STATIC, addr, insn.disp_at, length=1)


def constant_at(insn: Insn) -> Operand | None:
    if insn.code not in CONSTANTS or insn.imm_at is None:
        return None
    return Operand(Kind.CONSTANT, value=insn.insn.immediate(0), length=1)


def widened_constant_at(reached: list[Insn], last: int) -> Operand | None:
    """An INTEGER literal, widened to the LONG a `byval` parameter takes.

    `mov ax,imm16 / cwd / push dx / push ax` -- PDS and QB 4.5 have no dword
    push, so a constant argument to a user function goes through the same
    sign-extension the language itself does for `INTEGER` to `LONG`, four
    instructions where a runtime call's own small-constant form is one.
    """
    if last < 3:
        return None
    mov_ax, cwd, push_dx, push_ax = reached[last - 3], reached[last - 2], reached[last - 1], reached[last]
    if not (
        mov_ax.code == Code.MOV_R16_IMM16
        and mov_ax.insn.op0_register == Register.AX
        and cwd.code == Code.CWD
        and cwd.at == mov_ax.end
        and push_dx.code == Code.PUSH_R16
        and push_dx.insn.op0_register == Register.DX
        and push_dx.at == cwd.end
        and push_ax.code == Code.PUSH_R16
        and push_ax.insn.op0_register == Register.AX
        and push_ax.at == push_dx.end
    ):
        return None
    return Operand(Kind.CONSTANT, value=to_signed(mov_ax.insn.immediate(1), 2), length=4)


def one_operand(module: Module, reached: list[Insn], last: int) -> Operand | None:
    """The long argument whose pushes end at `reached[last]`, or None.

    A long reaches the stack either as one dword -- VBDOS /G3, and immediates
    everywhere -- or as two words, high first. The `+2` on the word form is the
    same discipline as the pair test in the lifter and fails the same silent way
    if it is dropped.
    """
    insn = reached[last]
    if insn.code in WIDE_PUSHES:
        return static_at(module, insn) or constant_at(insn)

    low = static_at(module, insn) or constant_at(insn)
    if low is None:
        return widened_constant_at(reached, last)
    if last == 0:
        return None
    high = static_at(module, reached[last - 1]) or constant_at(reached[last - 1])
    if high is None or reached[last - 1].end != insn.at:
        return None
    if low.kind is not high.kind:
        return None
    if low.kind is Kind.STATIC:
        if low.addr is None or high.addr != low.addr.plus(2):
            return None
        return replace(low, length=2)
    return replace(low, value=(high.value << 16) | (low.value & 0xFFFF), length=2)


def match(module: Module, reached: list[Insn], index: int) -> CallSite | None:
    """The call at `reached[index]` with its arguments, or None.

    Refuses anything it cannot account for exactly: every long argument the
    routine takes, nothing else in between, and every push adjacent to the
    next.
    """
    call = reached[index]
    name = module.calls.get(call.at)
    if name not in LEFT_FIRST:
        return None
    arity = ARITY.get(name, 2)

    found: list[Operand] = []
    last = index - 1
    while len(found) < arity and last >= 0:
        if reached[last].end != (call.at if not found else reached[last + 1].at):
            return None
        operand = one_operand(module, reached, last)
        if operand is None:
            return None
        found.append(operand)
        last -= operand.length
    if len(found) != arity:
        return None

    return CallSite(call.at, call.end, reached[last + 1].at, name, tuple(reversed(found)))


def sites(module: Module, reached: list[Insn]) -> list[CallSite]:
    found = []
    for index, insn in enumerate(reached):
        if insn.at in module.calls and (site := match(module, reached, index)) is not None:
            found.append(site)
    return found


# B$CPI4 rebuilds its answer through lahf/sahf, because an 8086 cannot compare a
# long in one go. A 386 can, and the flags a cmp leaves are the ones the
# following jcc wants -- but only the signed and equality ones. CF, PF and AF
# are the runtime's synthesis rather than the comparison's, so a site whose CF
# is read afterwards has to be left alone.
SYNTHESISED = Flag.CF | Flag.PF | Flag.AF

ABSORBED = {COMPARE: Code.CMP_R32_RM32, MULTIPLY: Code.IMUL_R32_RM32}

# Divide and remainder are C's: one idiv, no test of the divisor. `x / 0` and
# `-2147483648 / -1` are undefined in C and fault on this machine, where BC's
# runtime raised BASIC error 11 for the first and returned silently from the
# second. That behaviour does not survive, deliberately.
DIVIDES = {DIVIDE, REMAINDER}

RESULT = Register.EAX  # what the runtime returns a long in, as ax:dx


def load_of(operand: Operand) -> Instruction:
    if operand.kind is Kind.CONSTANT:
        return Instruction.create_reg_i32(Code.MOV_R32_IMM32, RESULT, operand.value)
    return Instruction.create_reg_mem(Code.MOV_EAX_MOFFS32, RESULT, MemoryOperand(displ=0, displ_size=2))


def fits_in_a_byte(value: int) -> bool:
    return -128 <= value < 128


def apply_to(name: str, operand: Operand) -> Instruction:
    if operand.kind is not Kind.CONSTANT:
        return Instruction.create_reg_mem(ABSORBED[name], RESULT, MemoryOperand(displ=0, displ_size=2))
    # the sign-extended byte forms are two or three bytes shorter, and a long
    # compared or multiplied by a small constant is the common case
    short = fits_in_a_byte(operand.value)
    if name == COMPARE:
        code = Code.CMP_RM32_IMM8 if short else Code.CMP_EAX_IMM32
        return Instruction.create_reg_i32(code, RESULT, operand.value)
    code = Code.IMUL_R32_RM32_IMM8 if short else Code.IMUL_R32_RM32_IMM32
    return Instruction.create_reg_reg_i32(code, RESULT, RESULT, operand.value)


def absorb(site: CallSite, live: Flag) -> Emitted | str:
    """The call replaced by two 386 instructions, or why it cannot be.

    Nine to sixteen bytes against fifteen and twenty-one, and it removes a far
    call and the routine behind it.
    """
    if site.name == FIX_MULTIPLY:
        return fix_multiply(site, live)
    if site.name in DIVIDES:
        return dividing(site, live)
    if site.name not in ABSORBED:
        return f"{site.name} is not absorbed"
    if site.name == COMPARE and live & SYNTHESISED:
        return f"the site's {live & SYNTHESISED!r} comes from the runtime, not from a comparison"
    if site.name == MULTIPLY and live & ALL:
        # imul sets the flags where the runtime left whatever it happened to
        return f"something reads {live & ALL!r} after the multiply"

    left, right = site.operands
    code = bytearray()
    relocations = []
    for step, operand in ((load_of(left), left), (apply_to(site.name, right), right)):
        encoder = Encoder(BITNESS)
        encoder.encode(step, 0)
        where = encoder.get_constant_offsets()
        if operand.kind is Kind.STATIC and operand.at is not None:
            relocations.append((len(code) + where.displacement_offset, operand.at))
        code += encoder.take_buffer()

    # a comparison leaves its answer in the flags; a multiply leaves a value, and
    # BC reads its high half from dx
    restore = b"" if site.name == COMPARE else FIXUP[0]
    return Emitted(bytes(code) + restore, tuple(relocations))


def assemble(steps: list[Instruction], relocated: dict[int, int]) -> Emitted:
    """Encode a block whose branches name one another by instruction index.

    Each instruction carries its index as its ip, so a branch target is an
    index; iced resolves them and picks the short forms. Where the operands
    finally landed is read back off the encoded bytes rather than predicted.
    """
    for index, insn in enumerate(steps):
        insn.ip = index
    encoder = BlockEncoder(BITNESS)
    encoder.add_many(steps)
    code = encoder.encode(0)

    decoder = Decoder(BITNESS, code, ip=0)
    placed = [(insn.ip, decoder.get_constant_offsets(insn)) for insn in decoder]
    if len(placed) != len(steps):
        raise ValueError(f"encoded {len(placed)} instructions from {len(steps)}")
    return Emitted(
        code,
        tuple((placed[index][0] + placed[index][1].displacement_offset, field) for index, field in relocated.items()),
    )


def restoring() -> list[Instruction]:
    """Put the high half back where BC reads it, through the stack."""
    return [
        Instruction.create_reg(Code.PUSH_R32, RESULT),
        Instruction.create_reg(Code.POP_R16, Register.AX),
        Instruction.create_reg(Code.POP_R16, Register.DX),
    ]


def dividing(site: CallSite, live: Flag) -> Emitted | str:
    """A long divide, as C compiles one.

        mov eax,[a] / mov ecx,[b] / cdq / idiv ecx

    and the remainder from edx. No test of the divisor, because C does not make
    one: `x / 0` and `-2147483648 / -1` are undefined, and on this machine they
    fault. BC's runtime raised BASIC error 11 for the first and returned
    silently from the second; neither survives, and that is the point.
    """
    if live & ALL:
        return f"something reads {live & ALL!r} after it, and idiv leaves the flags undefined"

    left, right = site.operands
    divisor = Register.ECX
    steps: list[Instruction] = []
    relocated: dict[int, int] = {}

    def add(insn: Instruction) -> int:
        steps.append(insn)
        return len(steps) - 1

    where = add(load_of(left))
    if left.kind is Kind.STATIC and left.at is not None:
        relocated[where] = left.at

    if right.kind is Kind.CONSTANT:
        add(Instruction.create_reg_i32(Code.MOV_R32_IMM32, divisor, right.value))
    else:
        where = add(Instruction.create_reg_mem(Code.MOV_R32_RM32, divisor, MemoryOperand(displ=0, displ_size=2)))
        if right.at is not None:
            relocated[where] = right.at

    add(Instruction.create(Code.CDQ))
    add(Instruction.create_reg(Code.IDIV_RM32, divisor))
    if site.name == REMAINDER:
        add(Instruction.create_reg_reg(Code.MOV_R32_RM32, RESULT, Register.EDX))
    for insn in restoring():
        add(insn)

    return assemble(steps, relocated)


def fix_multiply(site: CallSite, live: Flag) -> Emitted | str:
    """`fixMul&(a, b, fixShift)`, as C would write the shift it means:
    `(int32)(((int64)a * b) >> fixShift)`.

    One `imul` against the register form gives the full 64-bit product in
    `edx:eax`, and `shrd` is a pure bit shift across the pair -- extracting a
    32-bit window of a two's-complement value needs no sign correction, so it
    is right whatever the signs of `a` and `b` are. Multiplication commutes
    exactly in two's complement, so which of `a`/`b` loads into `eax` cannot
    change the result; `LEFT_FIRST` records the order BC actually pushed them
    in, but nothing here depends on it.

    `fixShift` is always known at compile time in practice -- nobody picks
    their fixed-point format at runtime -- so a literal goes straight into
    `shrd`'s own immediate byte. A variable still works: it loads into `cl`,
    the one register `shrd` can take a shift count from.
    """
    if live & ALL:
        return f"something reads {live & ALL!r} after it, and imul leaves the flags undefined"

    a, b, shift = site.pushed
    factor = Register.ECX
    steps: list[Instruction] = []
    relocated: dict[int, int] = {}

    def add(insn: Instruction) -> int:
        steps.append(insn)
        return len(steps) - 1

    where = add(load_of(a))
    if a.kind is Kind.STATIC and a.at is not None:
        relocated[where] = a.at

    if b.kind is Kind.STATIC:
        where = add(Instruction.create_mem(Code.IMUL_RM32, MemoryOperand(displ=0, displ_size=2)))
        if b.at is not None:
            relocated[where] = b.at
    else:
        add(Instruction.create_reg_i32(Code.MOV_R32_IMM32, factor, b.value))
        add(Instruction.create_reg(Code.IMUL_RM32, factor))

    if shift.kind is Kind.CONSTANT:
        if not 0 <= shift.value < 32:
            return f"a shift of {shift.value} normalises nothing back into 32 bits"
        add(Instruction.create_reg_reg_i32(Code.SHRD_RM32_R32_IMM8, RESULT, Register.EDX, shift.value))
    else:
        where = add(Instruction.create_reg_mem(Code.MOV_R16_RM16, Register.CX, MemoryOperand(displ=0, displ_size=2)))
        if shift.at is not None:
            relocated[where] = shift.at
        add(Instruction.create_reg_reg_reg(Code.SHRD_RM32_R32_CL, RESULT, Register.EDX, Register.CL))

    for insn in restoring():
        add(insn)

    return assemble(steps, relocated)
