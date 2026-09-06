"""Giving a required register a value of its own.

Some operands an instruction requires in one particular register and names
nowhere: `imul`'s product is dx:ax, `cwd` extends ax into dx, a shift by a
variable counts from cl. While the operands were the registers BC had, that
was satisfied by not moving anything. Lowering hands the allocator values
instead, and then the requirement is a claim about *this* instruction --
the same value may be anywhere at all one instruction later.

So the value the instruction requires is a value of its own, live across
that instruction and nothing else, with the original copied into it and out
of it. That is what `splitkit` does at a loop edge and `spiller` does around
a store, in the one place a fixed register asks for it.

The moves are ordinary: they belong to no parallel copy, and the allocator
places them like any other. What comes back beside the body is the pins for
the fresh values -- a hardware requirement, not a preference, which is why
this hands them over rather than writing them into the body's own pins.
"""

from dataclasses import replace

from iced_x86 import Register_

from qbopt import ir
from qbopt import lir
from qbopt import target


class Impossible(Exception):
    """One value an instruction requires in two different registers."""


def _already_there(pinned: "dict[int, Register_]", value: int, register: "Register_", width: int) -> bool:
    """Whether this value is pinned to the register the instruction wants.

    A call result is pinned to its 32-bit root and an ABI requirement names
    the 16-bit half of that same root -- one physical register, said two
    ways. Splitting then inserts `fresh <- value` with both ends pinned to
    it, and neither can be placed: procs-p-g2 reported `value#11 at width 2
    has no register`. One helper, because "is this the same register" is
    one question and the roots are how it is answered.
    """
    had = pinned.get(value)
    if had is None:
        return False
    if ir.ROOT.get(had, had) is not ir.ROOT.get(register, register):
        return False
    # Same root is not enough: the occurrence must be as wide as the
    # register the instruction reads it in, or reusing the value would
    # hand over more or fewer bytes than the requirement asked for.
    return target.width_of(register) == width


def constrained(
    body: lir.LirBody, pinned: "dict[int, Register_] | None" = None
) -> "tuple[lir.LirBody, dict[int, Register_]]":
    """`body` with a value per required occurrence, and where each must live.

    `pinned` is where the caller has already fixed values -- the raise's
    own answers for a call's results, which reach the allocator beside the
    body rather than inside it. Given here rather than read from
    `body.pins`, so that the map deciding whether a value is already in
    the register an instruction wants is the same one the allocation will
    honour.
    """
    pinned = {**body.pins, **(pinned or {})}
    fresh = _next_value(body)
    pins: "dict[int, Register_]" = {}
    blocks = []
    for block in body.blocks:
        insns: list[lir.Insn] = []
        for one in block.insns:
            wanted = {
                value: got
                for value, got in _wanted(one).items()
                if not _already_there(pinned, value, got[0], _width(one, value))
            }
            if not wanted:
                insns.append(one)
                continue
            before, after = [], []
            what = one.what
            dests: "list[object]" = list(what.dests) if what is not None else []
            sources: "list[object]" = list(what.sources) if what is not None else []
            defines, uses = list(one.defines), list(one.uses)
            widths = {held.value: held.width for held, _r in one.requires}
            for value, (register, where) in sorted(wanted.items()):
                held = ir.Held(fresh, widths.get(value) or _width(one, value))
                if not where:
                    # No occurrence to rewrite: the instruction reads this
                    # in a register it names nowhere. The copy still goes
                    # in, so what is pinned is a value that lives from the
                    # copy to the call and nowhere else -- the original
                    # stays free to live wherever the allocation likes.
                    before.append(_move(one, held, ir.Held(value, held.width)))
                    pins[fresh] = register
                    uses = [held.value if v == value else v for v in uses]
                    fresh += 1
                    continue
                pins[fresh] = register
                if any(side == "source" for side, _ in where):
                    before.append(_move(one, held, ir.Held(value, held.width)))
                if any(side == "dest" for side, _ in where):
                    after.append(_move(one, ir.Held(value, held.width), held))
                for side, index in where:
                    if side == "dest":
                        dests[index] = held
                        defines = [held.value if v == value else v for v in defines]
                    else:
                        sources[index] = held
                        uses = [held.value if v == value else v for v in uses]
                fresh += 1
            insns += before
            insns.append(
                replace(
                    one,
                    what=what
                    if what is None
                    else ir.Semantics(what.op, what.name, tuple(dests), tuple(sources), what.target),
                    defines=tuple(defines),
                    uses=tuple(uses),
                    # Consumed: the copy is in and the fresh value is
                    # pinned, so asking again would split the split.
                    requires=(),
                )
            )
            insns += after
        blocks.append(replace(block, insns=tuple(insns)))
    return replace(body, blocks=tuple(blocks)), pins


def required(body: lir.LirBody) -> "dict[int, Register_]":
    """Where each value the body's instructions require has to live.

    A scan, not a rewrite: `constrained` above splits an occurrence into a
    value of its own once, and this reads what the body says *now*. The
    spiller substitutes a reload value at an instruction in a later round,
    and that value inherits nothing -- so `imul` at 0xd0 in pressx came
    out with its halves in cx and bx, and the product was wrong. Asked
    again before every attempt, the new value is pinned where the old one
    was.
    """
    out: "dict[int, Register_]" = {}
    for block in body.blocks:
        for one in block.insns:
            for value, (register, _where) in _wanted(one).items():
                out[value] = register
    return out


def _wanted(one: lir.Insn) -> dict:
    """Each value this instruction requires somewhere, and where it sits.

    Keyed by the value, because a tie -- `imul`'s low half is both its
    first source and its first destination -- is one value in two
    occurrences and has to become one fresh value in both, or the two
    copies would name different registers and the tie would be gone.
    """
    what = one.what
    # A value read in a register the instruction names nowhere: a runtime
    # routine's arguments. No occurrence to split, so no places.
    out: dict[int, tuple[object, list]] = {held.value: (register, []) for held, register in one.requires}
    if what is None:
        return out
    for where, register in target.requirements(what).items():
        side = what.dests if where.side == "dest" else what.sources
        if where.index >= len(side):
            continue
        operand = side[where.index]
        if not isinstance(operand, ir.Held):
            continue
        held, places = out.get(operand.value, (register, []))
        if held != register:
            raise Impossible(f"{one.at:#06x}: value#{operand.value} is required in two registers at once")
        places.append((where.side, where.index))
        out[operand.value] = (register, places)
    return out


def _move(beside: lir.Insn, into: "ir.Held", out_of: "ir.Held") -> lir.Insn:
    """One copy, claiming none of the instruction's own bytes."""
    return lir.Insn(
        at=beside.at,
        covers=(beside.at, beside.at),
        what=ir.Semantics(ir.Operation.MOVE, "mov", (into,), (out_of,)),
        defines=(into.value,),
        uses=(out_of.value,),
        # The operation it stands beside, the way spiller.py's store and
        # phielim.py's copy do: objwrite re-derives MIR from LIR and reads
        # `op` for every instruction, so `None` there is not a valid
        # instruction -- `replace() should be called on dataclass
        # instances`, with the op it was rebuilding set to None.
        op=beside.op,
    )


def _width(one: lir.Insn, value: int) -> int:
    what = one.what
    if what is None:
        return 2
    for operand in (*what.dests, *what.sources):
        if isinstance(operand, ir.Held) and operand.value == value:
            return operand.width
    return 2


def _next_value(body: lir.LirBody) -> int:
    every = [v for block in body.blocks for one in block.insns for v in (*one.defines, *one.uses)]
    every += [phi.result for block in body.blocks for phi in block.phis]
    return max(every, default=0) + 1
