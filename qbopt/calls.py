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


@dataclass(frozen=True, slots=True)
class Operand:
    addr: Addr
    # the displacement field it came from, so its fixup can be reused
    at: int


@dataclass(frozen=True, slots=True)
class CallSite:
    at: int  # the call instruction
    end: int
    start: int  # where the first push begins
    name: str
    pushed: tuple[Operand, ...]  # in the order they reach the stack
    wide: bool  # one dword push per argument, rather than two words

    @property
    def operands(self) -> tuple[Operand, Operand]:
        """(left, right), whichever way this routine takes them."""
        first, second = self.pushed
        return (first, second) if LEFT_FIRST[self.name] else (second, first)


def pushed_operand(module: Module, insn: Insn) -> Operand | None:
    """The static this instruction pushes, if that is what it does."""
    if insn.code not in PUSHES or insn.disp_at is None or insn.memory_base != Register.NONE:
        return None
    addr = module.operands.get(insn.disp_at)
    return Operand(addr, insn.disp_at) if addr is not None and addr.space is Space.SEGMENT else None


def match(module: Module, reached: list[Insn], index: int) -> CallSite | None:
    """The call at `reached[index]` with its arguments, or None.

    Refuses anything it cannot account for exactly: two long arguments, all of
    them statics, and nothing else in between.
    """
    call = reached[index]
    name = module.calls.get(call.at)
    if name not in LEFT_FIRST:
        return None

    wide = reached[index - 1].code == Code.PUSH_RM32 if index else False
    count = 2 if wide else 4
    if index < count:
        return None

    window = reached[index - count : index]
    if any(insn.end != later.at for insn, later in zip(window, window[1:], strict=False)):
        return None
    if window[-1].end != call.at:
        return None

    operands = [pushed_operand(module, insn) for insn in window]
    if any(operand is None for operand in operands):
        return None
    found: list[Operand] = [operand for operand in operands if operand is not None]

    if wide:
        pushed = tuple(found)
    else:
        # two words each, high first, so the low one names the long
        halves = [(found[0], found[1]), (found[2], found[3])]
        if any(high.addr != low.addr.plus(2) for high, low in halves):
            return None
        pushed = tuple(low for _high, low in halves)

    return CallSite(call.at, call.end, window[0].at, name, pushed, wide)


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


def absorb(site: CallSite, live: Flag) -> Emitted | str:
    """The call replaced by two 386 instructions, or why it cannot be.

    Nine or fourteen bytes against fifteen and twenty-one, and it removes a far
    call and the routine behind it.
    """
    operation = ABSORBED.get(site.name)
    if operation is None:
        return f"{site.name} is not absorbed"
    if site.name is COMPARE and live & SYNTHESISED:
        return f"the site's {live & SYNTHESISED!r} comes from the runtime, not from a comparison"
    if site.name is MULTIPLY and live & ALL:
        # imul sets the flags where the runtime left whatever it happened to
        return f"something reads {live & ALL!r} after the multiply"

    left, right = site.operands
    code = bytearray()
    relocations = []
    for step, operand in ((Code.MOV_EAX_MOFFS32, left), (operation, right)):
        encoder = Encoder(BITNESS)
        encoder.encode(Instruction.create_reg_mem(step, RESULT, MemoryOperand(displ=0, displ_size=2)), 0)
        where = encoder.get_constant_offsets()
        relocations.append((len(code) + where.displacement_offset, operand.at))
        code += encoder.take_buffer()

    # a comparison leaves its answer in the flags; a multiply leaves a value, and
    # BC reads its high half from dx
    restore = b"" if site.name is COMPARE else FIXUP[0]
    return Emitted(bytes(code) + restore, tuple(relocations))
