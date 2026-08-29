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
from iced_x86 import Encoder
from iced_x86 import Register
from iced_x86 import Instruction
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

COMPARE = "B$CPI4"
MULTIPLY = "B$MUI4"
DIVIDE = "B$DVI4"
REMAINDER = "B$RMI4"

# True where the left operand is pushed first. Uniform across the compilers,
# opposite between the two routines.
LEFT_FIRST = {COMPARE: True, MULTIPLY: False, DIVIDE: False, REMAINDER: False}

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
    if low is None or last == 0:
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

    Refuses anything it cannot account for exactly: two long arguments, nothing
    else in between, and every push adjacent to the next.
    """
    call = reached[index]
    if module.calls.get(call.at) not in LEFT_FIRST:
        return None

    found: list[Operand] = []
    last = index - 1
    while len(found) < 2 and last >= 0:
        if reached[last].end != (call.at if not found else reached[last + 1].at):
            return None
        operand = one_operand(module, reached, last)
        if operand is None:
            return None
        found.append(operand)
        last -= operand.length
    if len(found) != 2:
        return None

    return CallSite(call.at, call.end, reached[last + 1].at, module.calls[call.at], (found[1], found[0]))


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

# Divide is not here, and the reason is measured rather than assumed.
# -2147483648 \\ -1 returns from B$DVI4 without raising anything, where idiv
# traps with #DE; and division by zero raises BASIC error 11 where idiv again
# traps. Multiply is different: B$MUI4 wraps on overflow and so does imul, so
# absorbing it changes nothing. suite/divmod.bas holds both measurements.
ABSORBED = {COMPARE: Code.CMP_R32_RM32, MULTIPLY: Code.IMUL_R32_RM32}

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
    if name is COMPARE:
        code = Code.CMP_RM32_IMM8 if short else Code.CMP_EAX_IMM32
        return Instruction.create_reg_i32(code, RESULT, operand.value)
    code = Code.IMUL_R32_RM32_IMM8 if short else Code.IMUL_R32_RM32_IMM32
    return Instruction.create_reg_reg_i32(code, RESULT, RESULT, operand.value)


def absorb(site: CallSite, live: Flag) -> Emitted | str:
    """The call replaced by two 386 instructions, or why it cannot be.

    Nine to sixteen bytes against fifteen and twenty-one, and it removes a far
    call and the routine behind it.
    """
    if site.name not in ABSORBED:
        return f"{site.name} is not absorbed"
    if site.name is COMPARE and live & SYNTHESISED:
        return f"the site's {live & SYNTHESISED!r} comes from the runtime, not from a comparison"
    if site.name is MULTIPLY and live & ALL:
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
    restore = b"" if site.name is COMPARE else FIXUP[0]
    return Emitted(bytes(code) + restore, tuple(relocations))
