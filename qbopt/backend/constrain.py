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

from qbopt.model import ir
from qbopt.model import lir
from qbopt.backend import target


class Impossible(Exception):
    """One value an instruction requires in two different registers."""


def _same_register(one: "Register_", other: "Register_", width: int) -> bool:
    """Whether two requirement names mean the same register at `width`.

    Target requirements name allocation roots while ABI requirements name
    the bytes the instruction reads. AX and EAX are consequently one answer
    for a word, but AH and EAX are not: EAX's byte name is AL.
    """
    if one == other:
        return True
    if target.width_of(one) == width:
        return one == target.named(other, width)
    if target.width_of(other) == width:
        return other == target.named(one, width)
    return ir.ROOT.get(one, one) == ir.ROOT.get(other, other)


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
    from qbopt.backend import spiller

    constants = spiller._constants(body, frozenset(value for one in body.insns for value in one.defines))

    def source(value, width):
        constant = constants.get(value)
        return constant if constant is not None and constant.width == width else ir.Held(value, width)

    fresh = max(_next_value(body), max((getattr(value, "id", value) for value in pinned), default=0) + 1)
    pins: dict[int, Register_] = {}
    blocks = []
    for block in body.blocks:
        insns: list[lir.Insn] = []
        for one in block.insns:
            # CSE may feed several ABI slots from one value. Give each
            # additional slot an independent lifetime before keying by value.
            inputs, slots, extra = [], {}, []
            for held, register in one.requires:
                key = (held.value, held.width, register)
                if key in slots:
                    inputs.append((slots[key], register))
                    continue
                if any(value == held.value for value, _width, _register in slots):
                    distinct = ir.Held(fresh, held.width)
                    insns.append(_move(one, distinct, source(held.value, held.width)))
                    pins[fresh] = pinned[fresh] = register
                    extra.append(fresh)
                    fresh += 1
                else:
                    distinct = held
                slots[key] = distinct
                inputs.append((distinct, register))
            if extra:
                one = replace(one, requires=tuple(inputs), uses=(*one.uses, *extra))
            # The declared width first. An operation that names no operand
            # has none for `_width` to read, so the requirement's own is
            # the only statement of how wide the value is: asked at a word
            # instead, the restore's answer looked unlike the eax it was
            # already pinned to, and a copy went in that could not be
            # placed -- pinned to eax beside the value it copied -- and
            # was spilled and reloaded for nothing.
            widths = {held.value: held.width for held, _r in one.requires + one.delivers}
            wanted = {
                value: got
                for value, got in _wanted(one).items()
                if not _already_there(pinned, value, got[0], widths.get(value) or _width(one, value))
            }
            given = {
                value: register
                for value, register in _delivered(one).items()
                if not _already_there(pinned, value, register, widths.get(value) or _width(one, value))
            }
            if not wanted and not given:
                insns.append(one)
                continue
            before, after, swap = [], [], {}
            input_values, output_values = {}, {}
            what = one.what
            dests: list[object] = list(what.dests) if what is not None else []
            sources: list[object] = list(what.sources) if what is not None else []
            defines, uses = list(one.defines), list(one.uses)
            for value, (register, where) in sorted(wanted.items()):
                held = ir.Held(fresh, widths.get(value) or _width(one, value))
                if not where:
                    # No occurrence to rewrite: the instruction reads this
                    # in a register it names nowhere. The copy still goes
                    # in, so what is pinned is a value that lives from the
                    # copy to the call and nowhere else -- the original
                    # stays free to live wherever the allocation likes.
                    before.append(_move(one, held, source(value, held.width)))
                    pins[fresh] = register
                    uses = [held.value if v == value else v for v in uses]
                    swap[value] = held.value
                    input_values[value] = held.value
                    fresh += 1
                    continue
                pins[fresh] = register
                swap[value] = held.value
                if any(side == "source" for side, _ in where):
                    before.append(_move(one, held, source(value, held.width)))
                if any(side == "dest" for side, _ in where):
                    after.append(_move(one, ir.Held(value, held.width), held))
                for side, index in where:
                    if side == "dest":
                        dests[index] = held
                        defines = [held.value if v == value else v for v in defines]
                        output_values[value] = held.value
                    else:
                        sources[index] = held
                        uses = [held.value if v == value else v for v in uses]
                        input_values[value] = held.value
                fresh += 1
            for value, register in sorted(given.items()):
                # Behind the instruction, not in front of it: the value is
                # what the idiom leaves in that register, so the copy
                # carries it out to wherever the allocation put it.
                held = ir.Held(fresh, widths.get(value) or _width(one, value))
                after.append(_move(one, ir.Held(value, held.width), held))
                pins[fresh] = register
                defines = [held.value if v == value else v for v in defines]
                swap[value] = held.value
                output_values[value] = held.value
                fresh += 1
            insns += before

            def address(operand):
                if not isinstance(operand, ir.Mem):
                    return operand
                return ir.mapped(operand, lambda value: ir.Held(swap.get(value.value, value.value), value.width))

            insns.append(
                replace(
                    one,
                    what=what
                    if what is None
                    else ir.Semantics(
                        what.op, what.name, tuple(map(address, dests)), tuple(map(address, sources)), what.target
                    ),
                    defines=tuple(defines),
                    uses=tuple(uses),
                    # Rewritten onto the fresh values rather than
                    # dropped. `_already_there` makes asking again a
                    # no-op, and the requirement has to survive because
                    # allocate.py re-derives it every round -- a
                    # requirement is about the instruction, not about
                    # whichever value happens to be feeding it, and the
                    # spiller replaces that value between rounds. Dropped,
                    # the restore's reload arrived with no pin at all and
                    # the idiom pushed whatever eax held.
                    requires=tuple(
                        (ir.Held(input_values.get(held.value, held.value), held.width), register)
                        for held, register in one.requires
                    ),
                    delivers=tuple(
                        (ir.Held(output_values.get(held.value, held.value), held.width), register)
                        for held, register in one.delivers
                    ),
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
    out: dict[int, Register_] = {}
    for block in body.blocks:
        for one in block.insns:
            for value, (register, _where) in _wanted(one).items():
                out[value] = register
            out.update(_delivered(one))
    return out


def _delivered(one: lir.Insn) -> dict:
    """Each value this instruction writes in a register it names nowhere.

    The other half of `_wanted`. The restore idiom is three instructions
    behind one node and names no operand at all, so nothing in its
    semantics says it leaves the two halves in ax and dx -- the allocation
    put one of them in cx and the idiom went on popping into dx, and
    lngmix stored the divisor where its accumulator's high half belonged.
    """
    return {held.value: register for held, register in one.delivers}


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
    out: dict[int, tuple[object, list]] = {}
    for held, register in one.requires:
        if held.value in out and not _same_register(out[held.value][0], register, held.width):
            raise Impossible(f"{one.at:#06x}: unsplit input value#{held.value} requires two registers")
        out[held.value] = (register, [])
    if what is None:
        return out
    for index, operand in enumerate(what.sources):
        if isinstance(operand, ir.Held) and operand.value in out:
            out[operand.value][1].append(("source", index))
    for where, register in target.requirements(what).items():
        side = what.dests if where.side == "dest" else what.sources
        if where.index >= len(side):
            continue
        operand = side[where.index]
        if not isinstance(operand, ir.Held):
            continue
        held, places = out.get(operand.value, (register, []))
        if not _same_register(held, register, operand.width):
            raise Impossible(f"{one.at:#06x}: value#{operand.value} is required in two registers at once")
        if (where.side, where.index) not in places:
            places.append((where.side, where.index))
        out[operand.value] = (register, places)
    return out


def _move(beside: lir.Insn, into: "ir.Held", out_of: "ir.Held | ir.Imm") -> lir.Insn:
    """One copy, claiming none of the instruction's own bytes."""
    return lir.Insn(
        at=beside.at,
        covers=(beside.at, beside.at),
        what=ir.Semantics(ir.Operation.MOVE, "mov", (into,), (out_of,)),
        defines=(into.value,),
        uses=(out_of.value,) if isinstance(out_of, ir.Held) else (),
        # The operation it stands beside, the way spiller.py's store and
        # phielim.py's copy do: omfwrite re-derives MIR from LIR and reads
        # `op` for every instruction, so `None` there is not a valid
        # instruction -- `replace() should be called on dataclass
        # instances`, with the op it was rebuilding set to None.
        op=beside.op,
    )


def _width(one: lir.Insn, value: int) -> int:
    for named, width in one.widths:
        if named == value:
            return width
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
