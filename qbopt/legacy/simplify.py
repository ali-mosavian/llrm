"""
Legacy register-identity elimination of split/rejoin sequences.

Absorption emits each call site in isolation -- calls.py has no view past
its own call -- so every site ends by putting the pair back the way BC's
16-bit code reads it, and the next site immediately rebuilds the 32-bit
value from those halves. In bench/nbody.bas there are 24 of these, 23
inside loops and 8 at loop depth 3.

The whole idiom is identity, and until now nothing could say so. What it
took was three things MIR did not have, each added in its own stage of
docs/variables.md: a value that is not named after a register, halves that
are sayable, and a stack whose slots have addresses.

    0x0177 half.tolow  v151 <- (v150, v142)   low16(v151) = high16(v150)
    0x0181 push v151     st [sp-0x6] 2
    0x0182 push v150     st [sp-0x8] 2
    0x0183 pop  -> v152  ld [sp-0x8] 4        spans both slots

    v152 = concat(high16(v150), low16(v150))
         = v150

This is deletion, not synthesis. `ir.emit` slices each surviving node's own
span, so dropping a node is already expressible and no instruction selector
is needed -- which is the whole reason this stage lands before one exists.

The register question answers itself, and is worth stating because it is
what makes the deletion safe rather than merely correct in the abstract:
the round trip hands the value back to the register it came from -- BC's
own `pop eax` reading what BC's own `push eax` split -- so removing it
leaves that register holding the value every later instruction already
expected to find there. Nothing downstream is rewritten. Checked, not
assumed: a triple whose ends disagree about the register is refused.
"""

from dataclasses import dataclass

from iced_x86 import Register_

from qbopt.analysis import liveness
from qbopt.model import mir
from qbopt.model.mir import Op
from qbopt.legacy import regalloc
from qbopt.model.mir import Value
from qbopt.model.mir import MirBody
from qbopt.objectfile.module import Space

# Two halves make a dword; nothing here composes anything else.
HALF = 2
WHOLE = 4


@dataclass(frozen=True, slots=True)
class RoundTrip:
    """A split and rejoin that computes nothing beyond, at most, a move."""

    at: tuple[int, ...]  # every op to delete, in address order
    value: Value  # what the pop produced, which is what went in
    source: Register_  # where the value already is
    target: Register_  # where the pop puts it
    pushes: tuple[int, int]  # the two pushes, low half first

    @property
    def free(self) -> bool:
        """Whether removing this needs nothing emitted in its place."""
        return self.source is self.target


def _slot(ref: mir.MemRef) -> int | None:
    return ref.addr.disp if ref.addr is not None and ref.addr.space is Space.STACK else None


def _pushed(op: Op) -> tuple[int, int, Value | None] | None:
    """The slot and width this op pushes to, and the value if it can be named.

    A value of None means the slot is taken by something this cannot name --
    `push dword [x]` reads memory as well as writing the stack. That still
    occupies the slot and must not be mistaken for one holding a half, but
    it is not a reason to forget every other slot, which is what refusing it
    outright did: the six other rejoins in bench/nbody.bas all have such a
    push between their halves.
    """
    if len(op.stores) != 1:
        return None
    where = _slot(op.stores[0])
    if where is None:
        return None
    real = [one for one in op.uses if not one.flags]
    named = real[0] if len(real) == 1 and not op.loads else None
    return where, op.stores[0].width, named


def _popped(op: Op) -> tuple[int, int, Value] | None:
    """The slot, width and value this op pops, if it pops one value."""
    if len(op.loads) != 1 or op.stores:
        return None
    where = _slot(op.loads[0])
    real = [one for one in op.defines if not one.flags]
    if where is None or len(real) != 1:
        return None
    return where, op.loads[0].width, real[0]


def _high_half_of(op: Op) -> tuple[Value, Value] | None:
    """(result, source) where the result's low 16 bits are the source's high.

    Exactly what mir.Synth.HALF_TO_LOW means, and the one fact that makes a
    rejoin an identity rather than a fresh value.
    """
    if op.op is not mir.Synth.HALF_TO_LOW or len(op.defines) != 1:
        return None
    source = next((one for one in op.uses if one != op.defines[0]), None)
    return None if source is None else (op.defines[0], source)


def _target_is_free(body: MirBody, block: mir.MirBlock, lo: int, hi: int, target: Register_, made: Value) -> bool:
    """Whether `target` holds nothing live anywhere in [lo, hi].

    A rejoin into another register is removed by emitting the move where the
    PUSHES are, not where the pop is -- by the pop the source may be gone,
    and in both of nbody's sites a `pop eax` has overwritten it. Moving the
    write earlier is only sound while the target is dead for the whole span
    it moves across, which is what this asks.

    `made` is the value the pop itself defines -- the one being replaced. It
    is live after the pop by construction, since something reads what the
    rejoin produced, and counting it would refuse every site there is.
    """
    alive = liveness.live(body)
    after = set(alive.live_out[block.at])
    for op in reversed(block.ops):
        if lo <= op.at <= hi:
            for one in after:
                if one is not made and body.origin.get(one) is target:
                    return False
        after = (after - set(op.defines)) | set(op.uses)
    return True


def _target_survives(body: MirBody, block: mir.MirBlock, lo: int, hi: int, target: Register_) -> bool:
    """Whether `target` still holds what it held at `lo` when `hi` is reached.

    The deletion's whole argument is that the round trip hands the value
    back to a register that already has it, so removing it changes nothing.
    That argument is only true while nothing writes that register in
    between -- and absorption emits code that does. In one generated
    program BC's `x MOD y MOD z` became

        push bx / push cx        the halves of ecx, to be rejoined later
        ...                      a whole absorbed MOD, ending
        pop ecx                  which is the first divide's own divisor
        idiv ecx
        pop ecx                  the rejoin
        idiv ecx                 and this one wanted the value above

    where source and target are both ecx, so the trip looked free and all
    three ops were deleted. The second idiv then divided by the first
    divide's divisor. The answer was 25375 where it should have been 8734.

    Liveness is the wrong question here and _target_is_free asks it: a
    write whose value is dead still destroys ours, and ours is excluded
    from that check by construction. So this asks the plain one -- does
    anything write it.
    """
    return not any(
        lo < op.at < hi and any(body.origin.get(one) is target for one in op.defines) for op in block.ops
    )


def round_trips(body: MirBody) -> tuple[RoundTrip, ...]:
    """Every split-and-rejoin in this body that computes nothing.

    Block-scoped, because a stack slot is a depth measured from the top of
    its own block and means something else in any other -- the same rule
    avail.py holds, and for the same reason.
    """
    found: list[RoundTrip] = []

    for block in body.blocks:
        halves: dict[Value, Value] = {}  # a value whose low 16 bits are another's high 16
        live: dict[int, tuple[int, Value | None, int]] = {}  # slot -> (width, value, op address)

        for op in block.ops:
            if (pair := _high_half_of(op)) is not None:
                halves[pair[0]] = pair[1]

            if (out := _popped(op)) is not None:
                where, width, value = out
                low, high = live.get(where), live.get(where + HALF)
                if (
                    width == WHOLE
                    and low is not None
                    and high is not None
                    and low[1] is not None
                    and high[1] is not None
                    and low[0] == HALF
                    and high[0] == HALF
                    and halves.get(high[1]) == low[1]
                ):
                    # The pop rebuilds exactly what the restore split. Safe
                    # to delete only while both ends name one register: the
                    # deletion leaves that register holding the value, and
                    # nothing downstream is rewritten.
                    was, now = body.origin.get(low[1]), body.origin.get(value)
                    at = tuple(sorted((low[2], high[2], op.at)))
                    if (
                        was is not None
                        and now is not None
                        and _target_survives(body, block, at[0], at[-1], now)
                        and (was is now or _target_is_free(body, block, at[0], at[-1], now, value))
                    ):
                        # Where the value already is, and where the pop puts
                        # it. Equal is a plain deletion. Different needs a
                        # move -- emitted where the PUSHES are, never where
                        # the pop is: by then whatever ran in between may
                        # have overwritten the source, and in nbody's own
                        # two sites a `pop eax` does exactly that.
                        found.append(
                            RoundTrip(
                                at,
                                low[1],
                                was,
                                now,
                                (low[2], high[2]),
                            )
                        )
                live.pop(where, None)
                live.pop(where + HALF, None)
                continue

            if (into := _pushed(op)) is not None:  # noqa: SIM102
                where, width, value = into
                live = {slot: held for slot, held in live.items() if slot + held[0] <= where or where + width <= slot}
                live[where] = (width, value, op.at)
                continue

            # Anything else that touches the stack, or a call, and the
            # slots stop being known. Conservative and cheap: a round trip
            # with real work between its halves is not one this deletes.
            if any(_slot(one) is not None for one in op.loads + op.stores) or op.barrier:
                live = {}

    return tuple(found)
