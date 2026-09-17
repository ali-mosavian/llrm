"""The stack slots a body needs, and where they are.

LLVM's `MachineFrameInfo`. It exists because the spiller cannot invent a
place to put a value: something has to own the frame, hand out slots, and
say how much bigger the frame got so that the prologue can reserve it.

BC's own frame is what the runtime set up before the body ran, and the
deepest slot it uses is what this starts below. A slot is two bytes at
`[bp-n]`, and `size` is what the prologue has to take off sp.

`CreateSpillStackObject` is the LLVM name for `slot()`.
"""

from dataclasses import field
from dataclasses import dataclass

from iced_x86 import Register

from qbopt.model import ir
from qbopt.model import lir
from qbopt.backend import nativeframe
from qbopt.objectfile.module import Space
from qbopt.backend.nativeframe import Plan

# BC's frame is word-aligned. Extended floating spills occupy five words.
WORD = 2
type SlotKey = int | tuple[str, int]
ENTER = "B$ENRA"
LEAVE = "B$EXSA"
RUNTIME_SIZE = 10  # runtime/inc/stack.inc: FR_SIZE, below BP and above locals.


class Refused(Exception):
    """The existing runtime frame has no established extent."""


@dataclass
class Frame:
    """Where a body's stack objects are. Mutable: slots are handed out."""

    # The deepest displacement BC's own code already reaches, which is
    # negative. Everything this hands out is below it.
    floor: int
    slots: dict[SlotKey, int] = field(default_factory=dict)
    native: Plan | None = None
    native_pins: dict[int, int] = field(default_factory=dict)
    # Capacity belongs to the stack object, not to whichever virtual value
    # first received it.  Slot coloring may assign later non-overlapping
    # values to the same displacement, including across allocation rounds.
    capacities: dict[int, int] = field(default_factory=dict)

    @property
    def size(self) -> int:
        """How many bytes the prologue has to reserve beyond BC's own."""
        return -(min(self.slots.values(), default=self.floor) - self.floor)

    def slot(self, value: SlotKey, width: int) -> int:
        """This value's displacement, creating one where it has none."""
        if self.native is not None and not self.native.framed:
            raise Refused("a frameless native procedure cannot hold a spill below its caller's BP")
        if value not in self.slots:
            lowest = min(self.slots.values(), default=self.floor)
            capacity = max(width, WORD)
            self.slots[value] = lowest - capacity
            self.capacities[self.slots[value]] = capacity
        return self.slots[value]

    def cell(self, value: SlotKey, width: int) -> ir.Mem:
        """The memory operand that reads or writes this value's slot."""
        return ir.Mem(ir.Addr(Space.FRAME, self.slot(value, width)), width, Register.BP, 0, 2)


def of(body: lir.LirBody, calls: dict | None = None, *, family: str = "", native: Plan | None = None) -> Frame:
    """A frame for this body, starting below everything it already reaches."""
    floor = native.entry.floor if native is not None else 0
    # VBDCL10E rtenexit 0024..0036 pushes ten words before SUB SP,CX.
    runtime_size = 20 if family == "vbdos" else RUNTIME_SIZE
    constants = {}
    for block in body.blocks:
        for one in block.insns:
            if one.what is None:
                continue
            match one.what:
                case ir.Semantics(op=ir.Operation.MOVE, dests=(ir.Held(value, _),), sources=(ir.Imm(count, _),)):
                    constants[value] = count
                case ir.Semantics(op=ir.Operation.CALL) if (calls or {}).get(one.at) == ENTER:
                    sizes = [held.value for held, reg in one.requires if reg == Register.CX]
                    if len(sizes) != 1 or sizes[0] not in constants:
                        raise Refused("runtime frame size is not a known constant")
                    floor = min(floor, -runtime_size - constants[sizes[0]])
            for where in (*one.what.dests, *one.what.sources):
                if (
                    isinstance(where, (ir.Mem, ir.Address))
                    and where.addr is not None
                    and where.addr.space is Space.FRAME
                ):
                    if (
                        native is not None
                        and isinstance(where, ir.Mem)
                        and where.stack_argument
                        and (one.at, where.addr.disp, where.width) in native.outgoing
                    ):
                        continue
                    floor = min(floor, where.addr.disp)
    if native is not None and floor < native.entry.floor:
        raise Refused("native frame references extend below its established reservation")
    return Frame(floor=floor, native=native, native_pins=nativeframe.pins(body, native) if native else {})
