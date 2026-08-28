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
from iced_x86 import Register

from qbopt.flags import Flag
from qbopt.declen import Insn
from qbopt.module import Addr
from qbopt.lift import Emitted
from qbopt.module import Space
from qbopt.module import Module

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

LOAD_EAX = bytes([0x66, 0xA1])  # mov eax,[disp16]
CMP_EAX = bytes([0x66, 0x3B, 0x06])  # cmp eax,[disp16]


def emit_compare(site: CallSite, live: Flag) -> Emitted | str:
    """`mov eax,[a] / cmp eax,[b]` in place of the call, or why not.

    Nine bytes against fifteen or twenty-one, and it removes a far call and the
    eleven instructions behind it.
    """
    if site.name != COMPARE:
        return f"{site.name} is not absorbed yet"
    if live & SYNTHESISED:
        return f"the site's {live & SYNTHESISED!r} comes from the runtime, not from a comparison"

    left, right = site.operands
    code = LOAD_EAX + bytes(2) + CMP_EAX + bytes(2)
    return Emitted(code, ((len(LOAD_EAX), left.at), (len(LOAD_EAX) + 2 + len(CMP_EAX), right.at)))
