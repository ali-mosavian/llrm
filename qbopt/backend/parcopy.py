"""Scheduling a parallel copy: moves that happen at once, written in an order.

A phi says several values arrive together on one edge. `phielim` turns each
into a move and marks them one group; they are simultaneous, and writing
them down in the order the phis happened to be listed is right only while
none reads a location an earlier one overwrote. pressx-v-evt emitted
`r24 <- [bp-8]` and then `r27 <- r24`, so one arm of the phi carried the
value the reload had just put there and R came out 6460 for 7500.

After allocation, deliberately: which moves conflict is a question about
locations, and until the allocator has chosen them there is nothing to ask.
LLVM does the same thing in the same place -- `VirtRegRewriter` leaves
copies and the machine-copy passes below it order them.

Only the safe half. A move may go once nothing left in the group reads what
it writes; a group where nothing qualifies is a cycle, which needs a
temporary or an exchange to break, and this refuses it by name instead --
`Tangled` -- so the caller falls back rather than emitting an order that is
wrong.
"""

from dataclasses import replace

from iced_x86 import Register

from qbopt.model import ir
from qbopt.model import lir
from qbopt.model.passes import LIRTransform


class Tangled(Exception):
    """A parallel copy whose moves all read each other's destinations."""


class Malformed(Exception):
    """Something in a copy group that is not a move of one place to another."""


class ParallelCopy(LIRTransform):
    name = "parcopy"

    def transform(self, body: lir.LirBody) -> lir.LirBody:
        return scheduled(body)


def scheduled(body: lir.LirBody) -> lir.LirBody:
    """`body` with every copy group written in an order that computes it."""
    if not any(one.group is not None for block in body.blocks for one in block.insns):
        return body
    blocks = []
    for block in body.blocks:
        out: list[lir.Insn] = []
        run: list[lir.Insn] = []
        for one in (*block.insns, None):
            group = one.group if one is not None else None
            if run and (group != run[0].group):
                out += [part for move in _ordered(run) for part in _expanded(move)]
                run = []
            if one is None:
                break
            if group is None:
                out.append(one)
                continue
            run.append(one)
        blocks.append(replace(block, insns=tuple(out)))
    return replace(body, blocks=tuple(blocks))


def _expanded(one: lir.Insn) -> tuple[lir.Insn, ...]:
    """An ordered frame copy needs no scratch register and preserves flags."""
    if one.what is not None and one.what.op is not ir.Operation.MOVE:
        return (one,)
    into, source = one.what.dests[0], one.what.sources[0]
    if not isinstance(into, ir.Mem) or not isinstance(source, ir.Mem):
        return (one,)
    if (
        into.width != source.width
        or into.width not in (2, 4)
        or any(cell.through != Register.BP for cell in (into, source))
    ):
        raise Malformed("memory parallel copy needs equal-width frame slots")
    return (
        replace(one, what=ir.Semantics(ir.Operation.PUSH, "push", (), (source,)), defines=(), uses=()),
        replace(
            one,
            what=ir.Semantics(ir.Operation.POP, "pop", (into,), ()),
            covers=(one.at, one.at),
            op=None,
            defines=(),
            uses=(),
            spread=(),
        ),
    )


def _ordered(moves: list[lir.Insn]) -> list[lir.Insn]:
    """One group, in an order where no move reads what an earlier one wrote."""
    identities = [one for one in moves if _into(one) == _outof(one)]
    left = [one for one in moves if _into(one) != _outof(one)]
    # An identity emits no machine instruction.  It still defines the
    # virtual value represented by its destination, which later LIR safety
    # analyses read from ``defines``/``uses``.  Retain that fact as a
    # zero-cost marker rather than deleting the whole instruction.
    out: list[lir.Insn] = [lir.anchor(one) for one in identities]
    while left:
        # Free where nothing still to come reads the place it writes.
        wanted = {_outof(one) for one in left}
        ready = [one for one in left if _into(one) not in wanted]
        if not ready:
            rotated = _rotated(left)
            if rotated is None:
                raise Tangled(
                    "these moves all read each other's destinations and need a temporary: "
                    + ", ".join(f"{_into(one)} <- {_outof(one)}" for one in left)
                )
            made, used = rotated
            out += made
            for one in used:
                left.remove(one)
            continue
        for one in ready:
            out.append(replace(one, group=None))
            left.remove(one)
    return out


def _rotated(left: list[lir.Insn]) -> "tuple[list[lir.Insn], list[lir.Insn]] | None":
    """One cycle out of `left`, using exchanges or the machine stack.

    A cycle `p1 <- p2 <- ... <- pn <- p1` is `xchg p1,p2` then `xchg p2,p3`
    and so on: n-1 instructions, no scratch register, and xchg writes no
    flags. When adjacent places are both spilled, save the first on the
    machine stack, perform the remaining moves, then pop into the last.
    Every intermediate memory copy is itself a balanced push/pop, so the
    saved value stays below it and no spare register is needed.
    """
    writes = {_into(one): one for one in left}
    start = left[0]
    cycle = [start]
    place = _outof(start)
    while place != _into(start):
        one = writes.get(place)
        if one is None or one in cycle:
            return None  # not a cycle this walk closes
        cycle.append(one)
        place = _outof(one)

    operands = [one.what.dests[0] for one in cycle]
    pairs = list(zip(operands, operands[1:]))
    from qbopt.backend import target

    if any(isinstance(one, ir.Reg) and one.register in target.SEGMENTS for one in operands):
        return None  # no `xchg` names a segment register
    # One width across the whole chain. `_named` keys a register by its
    # root, which is right for the ordering -- writing ax really does
    # clobber eax's low half, so the dependency is real -- but it makes
    # `mov ax,bx` and `mov ebx,eax` walk as one clean 2-cycle, and the
    # exchange built from their own operands is `xchg ax,ebx`: either no
    # encoding at all or the low halves swapped and ebx's upper half
    # quietly wrong. Mem has the same hole, its width being no part of the
    # key either.
    if len({one.width for one in operands}) != 1:
        return None
    last = cycle[-1]
    if not any(isinstance(a, ir.Mem) and isinstance(b, ir.Mem) for a, b in pairs):
        # The last move contributes no instruction, so it must stand for no
        # bytes -- every copy a group holds is inserted, and `lir.without`
        # relies on the same fact.
        if last.covers and last.covers[0] != last.covers[1]:
            return None
        made = [
            replace(
                one,
                what=ir.Semantics(ir.Operation.EXCHANGE, "xchg", (a, b), (b, a)),
                group=None,
                defines=(),
                uses=(),
            )
            for one, (a, b) in zip(cycle, pairs)
        ]
        return made, cycle

    if operands[0].width not in (2, 4):
        return None
    saved = operands[0]
    made = [
        replace(
            start,
            what=ir.Semantics(ir.Operation.PUSH, "push", (), (saved,)),
            group=None,
            defines=(),
            uses=(),
        ),
        *(replace(one, group=None) for one in cycle[:-1]),
        replace(
            last,
            what=ir.Semantics(ir.Operation.POP, "pop", (operands[-1],), ()),
            group=None,
            defines=(),
            uses=(),
        ),
    ]
    return made, cycle


def _into(one: lir.Insn) -> str:
    return _place(one, one.what.dests if one.what else ())


def _outof(one: lir.Insn) -> str:
    return _place(one, one.what.sources if one.what else ())


def _place(one: lir.Insn, where: tuple) -> str:
    """The location an operand names, as one comparable thing."""
    what = one.what
    if what is None or what.op is not ir.Operation.MOVE or len(what.dests) != 1 or len(what.sources) != 1:
        raise Malformed(f"{one.at:#06x} is in a copy group and is not a move")
    if len(where) != 1:
        raise Malformed(f"{one.at:#06x} names {len(where)} places on one side")
    return _named(where[0])


def _named(one) -> str:
    if isinstance(one, ir.Reg):
        return f"r{ir.ROOT.get(one.register, one.register)}"
    if isinstance(one, ir.Mem):
        return f"m{one.addr}:{one.through}:{one.offset}"
    if isinstance(one, ir.Imm):
        return f"i{one.value}"
    raise Malformed(f"a copy group names {one!r}, which is not a place")
