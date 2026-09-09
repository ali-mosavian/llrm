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
from qbopt.objectfile.module import Space

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

    @property
    def size(self) -> int:
        """How many bytes the prologue has to reserve beyond BC's own."""
        return -(min(self.slots.values(), default=self.floor) - self.floor)

    def slot(self, value: SlotKey, width: int) -> int:
        """This value's displacement, creating one where it has none."""
        if value not in self.slots:
            lowest = min(self.slots.values(), default=self.floor)
            self.slots[value] = lowest - max(width, WORD)
        return self.slots[value]

    def cell(self, value: SlotKey, width: int) -> ir.Mem:
        """The memory operand that reads or writes this value's slot."""
        return ir.Mem(ir.Addr(Space.FRAME, self.slot(value, width)), width, Register.BP, 0, 2)


def of(body: lir.LirBody, calls: dict | None = None) -> Frame:
    """A frame for this body, starting below everything it already reaches."""
    floor = 0
    constants = {}
    for block in body.blocks:
        for one in block.insns:
            if one.what is None:
                continue
            match one.what:
                case ir.Semantics(op=ir.Operation.MOVE, dests=(ir.Held(value, _),), sources=(ir.Imm(count, _),)):
                    constants[value] = count
                case ir.Semantics(op=ir.Operation.CALL, sources=(ir.Held(value, _),)) if (calls or {}).get(
                    one.at
                ) == ENTER:
                    if value not in constants:
                        raise Refused("runtime frame size is not a known constant")
                    floor = min(floor, -RUNTIME_SIZE - constants[value])
            for where in (*one.what.dests, *one.what.sources):
                if isinstance(where, ir.Mem) and where.addr is not None and where.addr.space is Space.FRAME:
                    floor = min(floor, where.addr.disp)
    return Frame(floor=floor)
