"""MIR to machine form.

The one place a value becomes a register operand. Everything above this is
MIR -- values, constants and cells -- and everything below is x86, which is
the boundary rule 5 draws.

What comes out names no register either: an ir.Held says *which value*, and
select.py resolves it through the allocation. Naming one here would only
move the pass's mistake down a layer.
"""

from collections import Counter
from dataclasses import replace

from iced_x86 import Register
from iced_x86 import RegisterExt
from iced_x86 import InstructionInfoFactory

from qbopt import ir
from qbopt import mir
from qbopt import target
from qbopt import runtime
from qbopt import liveness
from qbopt.module import Addr


def operand(arg: mir.Arg) -> ir.Loc:
    """One MIR operand as the machine's."""
    if isinstance(arg, mir.Held):
        return ir.Held(value=arg.value.id, width=arg.width)
    if isinstance(arg, mir.Const):
        return ir.Imm(value=arg.n, width=arg.width)
    if isinstance(arg, mir.Symbol):
        return ir.Imm(arg.addend, arg.width, Addr(arg.space, arg.offset, arg.index))
    if isinstance(arg, mir.Cell):
        return arg.ref
    return arg.what  # the x87 stack, which has no MIR form


# What the machine calls each operation a pass can invent. Everything else
# keeps the operation it was raised with, which lowering reads off the node
# -- so this is only for the shapes MIR creates: a copy and a jump.
_MACHINE: dict[mir.Kind, tuple[ir.Operation, str]] = {
    # The two the machine spells differently and MIR does not: an addition
    # that takes the carry in is `adc`, and a subtraction that takes the
    # borrow is `sbb`. Named here and nowhere above -- MIR says only that
    # the operation reads the flags the one before it left.
    mir.Kind.ADD_CARRY: (ir.Operation.BINARY, "adc"),
    mir.Kind.SUB_BORROW: (ir.Operation.BINARY, "sbb"),
    # One operand, in the opcode. MIR says what they compute; that they
    # are written `inc` and `dec` is only true here.
    mir.Kind.INCREMENT: (ir.Operation.UNARY, "inc"),
    mir.Kind.DECREMENT: (ir.Operation.UNARY, "dec"),
    mir.Kind.COPY: (ir.Operation.MOVE, "mov"),
    mir.Kind.STORE: (ir.Operation.MOVE, "mov"),
    mir.Kind.JUMP: (ir.Operation.JUMP, "jmp"),
    # Strength reduction writes both: one multiply in a preheader and one
    # add at a latch, where BC recomputed the product every iteration. The
    # multiply is the two-operand `imul r,r/m`, which writes its
    # destination and nothing else -- the widening form writes dx:ax, and
    # an operation MIR invented defines one value.
    mir.Kind.MUL: (ir.Operation.MULTIPLY, "imul"),
    mir.Kind.ADD: (ir.Operation.BINARY, "add"),
}


def semantics(op: mir.Op, was: ir.Semantics | None = None, place=None) -> ir.Semantics | None:
    """What this operation computes, in machine form, or None for verbatim.

    None means nothing rewrote it: `was` is still what it says, and layout
    emits the original bytes rather than re-encoding them. A re-encode that
    lands on a longer form for the same instruction is how a rebuild starts
    growing without anything having been optimised.
    """
    same_target = was is None or op.target == was.target
    if place is None and op.raised is not None and (op.args, op.results) == op.raised and same_target:
        # Nothing rewrote it, so the MIR emitter carries its own bytes.
        # A caller naming a resolver is lowering for a path where the
        # allocation reaches every instruction, and there "unchanged"
        # still means "an operation over values": answering with the
        # original instruction hands back BC's registers, which is the
        # machine leaking into what a value is allowed to live in.
        return None
    if op.kind is mir.Kind.NOTHING and op.name == "":
        return ir.Semantics(ir.Operation.NOTHING, "", (), ())
    if not op.args and not op.results and op.raised is None:
        return None  # nothing to build one from
    # A value resolves to the register the original instruction had in the
    # same position where there is one. Emitting ir.Held instead hands the
    # choice to the allocation, and the allocation is applied to the ops it
    # re-encodes and not to the ones emitted from their own bytes -- so the
    # two disagree, and lngmix printed 110 for 142900 with the dividend
    # deleted. Where there is no such operand -- an operation a pass
    # invented -- ir.Held is the only honest answer and select resolves it.
    was_op, name = _MACHINE.get(op.kind, (op.op, op.name))
    if op.kind is mir.Kind.NOTHING and op.op is not ir.Operation.NOTHING:
        was_op, name = ir.Operation.NOTHING, "nop"
    args = op.args
    if (
        place is as_a_value
        and op.kind in (mir.Kind.ADD, mir.Kind.AND, mir.Kind.OR, mir.Kind.XOR)
        and len(args) == 2
        and isinstance(args[0], mir.Const)
        and isinstance(args[1], mir.Held)
    ):
        args = (args[1], args[0])
    if (
        place is as_a_value
        and op.kind is mir.Kind.MUL
        and len(op.results) == 1
        and len(args) == 2
        and isinstance(args[0], mir.Held)
        and isinstance(args[1], mir.Const)
    ):
        args = (args[0], args[0], args[1])
    place = place or _place
    return ir.Semantics(
        was_op,
        name,
        dests=tuple(place(one, was.dests if was else (), i) for i, one in enumerate(op.results)),
        sources=tuple(place(one, was.sources if was else (), i) for i, one in enumerate(args)),
        target=_target(op, was),
    )


def as_a_value(arg: mir.Arg, had: tuple, index: int) -> ir.Loc:
    """One operand, left as the value it is.

    For a path where the allocation reaches every instruction. `_place`
    below resolves a value to the register BC had, which leaves nothing
    for the allocator to rewrite: it assigned cx to pressx's constant and
    the bytes still said ax, so the load after it killed the constant and
    the program printed R= 0 for 7500.
    """
    return operand(arg)


def _valueized(what: "ir.Semantics", op: "mir.Op") -> "ir.Semantics":
    """`what` with every register the operation names as a value replaced by it.

    Operand for operand and by position: `made`'s destinations line up
    with the operation's results and its sources with its arguments,
    which is how the pass that wrote it built the two. An idiom with no
    operands has nothing to line up and comes back exactly as it is --
    the restore's pair is the node's, not an operand's.
    """

    def named(side: tuple, mine: tuple) -> tuple:
        out = []
        for index, one in enumerate(side):
            was = mine[index] if index < len(mine) else None
            if isinstance(one, ir.Reg) and isinstance(was, mir.Held) and not was.value.flags:
                out.append(ir.Held(was.value.id, one.width))
            else:
                out.append(one)
        return tuple(out)

    return ir.Semantics(
        what.op,
        what.name,
        named(what.dests, op.results),
        named(what.sources, op.args),
        what.target,
    )


def _place(arg: mir.Arg, had: tuple, index: int) -> ir.Loc:
    """One operand, keeping the register the instruction already had."""
    got = operand(arg)
    if not isinstance(got, ir.Held) or index >= len(had):
        return got
    was = had[index]
    return was if isinstance(was, ir.Reg) and was.width == got.width else got


def rewritten(op, place=None) -> "ir.Semantics | None":
    """What a pass made of this operation, in machine form, or None.

    None means nothing rewrote it, which is a different answer from what
    it computes -- `current` collapses the two and a caller asking whether
    an operand *went* cannot.

    _located here and not in the callers. `semantics` builds from MIR's
    own operands, so a cell comes back as mir.MemRef, and nothing below
    this line has an encoding for one: two callers converted and two did
    not, which cost fifteen objects their rebuild (`add is not one
    select.py can emit`) and stride its `t` (`add [t],ax` emitted with no
    fixup, accumulating into offset zero, T= 0 for 210).
    """
    was = getattr(op.node, "semantics", None)
    if op.made is not None:
        if place is None:
            return op.made
        # Valueized in place. `pairs.widened` writes machine form -- one
        # 32-bit `and eax,...` for two 16-bit halves and a carry -- and
        # returning it untouched made a widened operation the only one
        # still naming BC's own registers while its neighbours had become
        # values, with nothing for the allocator to rewrite.
        #
        # Its own operands, not rebuilt from the operation: passing it
        # back through `semantics` gave the restore idiom -- which has
        # none, and whose pair the node names -- three operands it never
        # had, and select emitted nothing for it.
        return _valueized(op.made, op)
    return _located(semantics(op, was, place), was)


def current(op, place=None) -> "ir.Semantics | None":
    """What this operation computes now, in machine form.

    The one answer to the question every consumer used to ask as
    `op.made if op.made is not None else op.node.semantics` -- which read
    the *original* instruction for an operation a pass had rewritten in
    MIR's own operands, and so told twenty callers the fold had not
    happened.
    """
    return rewritten(op, place) or getattr(op.node, "semantics", None)


def _target(op: mir.Op, was: ir.Semantics | None) -> int | None:
    """A branch's destination, which is a block address and not an operand."""
    if op.target is not None:
        return op.target
    return was.target if was is not None else None


__all__ = ["operand", "semantics", "rewritten", "current"]


class Unlowered(Exception):
    """An operand nothing here can turn into a machine location."""


def _located(what: "ir.Semantics | None", was: "ir.Semantics | None") -> "ir.Semantics | None":
    """`what` with every MIR operand in it replaced by a machine one.

    A cell is the one that needs help. mir.MemRef says which bytes and what
    its address depends on -- the alias question -- and ir.Mem says how to
    encode it: which register reaches it, and how wide the displacement
    field was, which is not how wide the number needs to be. Neither is
    derivable from the other, so the encoding half comes from the operand
    the same instruction had in the same position before a pass rewrote it.

    A cell in a position the original had none is an error rather than a
    guess. Encoding a displacement at the wrong width is `mov ax,[bx]`
    emitted as `8b 07` -- the right instruction reading the wrong address.
    """
    if what is None:
        return None
    dests = tuple(_machine(one, was.dests if was else (), index) for index, one in enumerate(what.dests))
    sources = tuple(_machine(one, was.sources if was else (), index) for index, one in enumerate(what.sources))
    if dests == what.dests and sources == what.sources:
        return what
    return ir.Semantics(what.op, what.name, dests, sources, what.target)


def _machine(one, had: tuple, index: int):
    if not isinstance(one, mir.MemRef):
        return one
    before = had[index] if index < len(had) else None
    if not isinstance(before, ir.Mem):
        before = next((x for x in had if isinstance(x, ir.Mem)), None)
    base = ir.Held(one.base.id, 2) if one.base is not None else None
    if base is not None:
        # NONE until something places it. Keeping BC's register here makes
        # an unallocated operand indistinguishable from a placed one, and
        # arrprm's first store passed by luck exactly that way.
        made = (
            ir.Mem(one.addr, one.width, Register.NONE, before.offset, before.disp_width)
            if before is not None
            else replace(_addressed(one), through=Register.NONE)
        )
        return replace(made, base=base)
    if before is not None:
        return ir.Mem(one.addr, one.width, before.through, before.offset, before.disp_width)
    return _addressed(one)


def _addressed(one: "mir.MemRef") -> "ir.Mem":
    """A cell the original instruction had no memory operand for.

    A pass put it there -- a fold that turned a register read back into the
    read of the cell it came from -- so there is no encoding to copy and it
    has to come from the address itself. Only for the two spaces whose
    encoding the address fully determines: a frame slot is reached through
    bp and a segment-relative cell through no register at all, both with a
    two-byte displacement, which is what BC emits and what a fixup expects.

    A segment-relative cell may be indexed, and then `addr.base` is the
    register that reaches it -- part of the address's own identity, since
    two elements at the same displacement are not the same address unless
    that register agrees. So that is written down rather than guessed.

    Anything else -- a far pointer, the stack -- is refused by name.
    """
    from qbopt.module import Space

    addr = one.addr
    if addr is None:
        raise Unlowered("a cell with no address cannot be encoded: nothing says which register reaches it")
    if addr.space is Space.FRAME:
        return ir.Mem(addr, one.width, Register.BP, 0, 2)
    if addr.space is Space.SEGMENT:
        # An indexed element says which register reaches it: `addr.base` is
        # part of the address's own identity, because two elements at the
        # same displacement are not the same address unless that register
        # agrees. So the encoding is not a guess -- it is written down.
        return ir.Mem(addr, one.width, addr.base, 0, 2)
    raise Unlowered(f"a cell at {addr} in {addr.space} has no encoding this can derive")


def lowered(
    name: str,
    body: "mir.MirBody",
    calls: dict[int, str] | None,
    absorbed: "set[int] | dict[int, tuple] | None",
    contracts: "dict[int, object]",
    coverage: "dict[int, tuple] | None" = None,
) -> "lir.LirBody":
    """One MIR body as machine instructions, and nothing else.

    The pass that ends the abstract half. Above this a value is a value and
    an operation says what it computes; below it every operand is a
    location and the only questions left are which register, where the
    bytes go and what a fixup names.

    One instruction per operation, in the order the blocks give. `what` is
    None where nothing rewrote the operation, which is layout's signal to
    carry the original bytes rather than re-encode them -- a re-encode that
    lands on a longer form for the same instruction is how a rebuild grows
    without anything having been optimised.
    """
    from qbopt import lir

    # An absorbed call site is emitted by select.absorbed, seventeen bytes
    # of mov and idiv, and not from any semantics this could give it.
    # Lowering it to the call it replaced put `made` on it, and layout then
    # asked whether *that* still had an operand a fixup could sit in -- a
    # `call` with no operands does not -- so the site's own relocation was
    # dropped. lngmix read the wrong address for `v` and printed 50 for
    # 142900.
    #
    # What anything reads, so a definition nothing reads can become what it
    # always was: a statement that the register is destroyed, which
    # `clobbers` makes without inventing a value to carry it.
    read = {one.id for block in body.blocks for op in block.ops for one in op.uses}
    read |= {value.id for block in body.blocks for phi in block.phis for value in phi.incoming.values()}
    calls = calls or {}
    making = Lowering(body, read, calls, absorbed or (), contracts, coverage)
    # Once: expanding twice would build two of every instruction, and the
    # question below is about the ones this body will actually hold.
    readers = Counter(value for block in body.blocks for op in block.ops for value in op.uses)
    readers.update(value for block in body.blocks for phi in block.phis for value in phi.incoming.values())
    live = liveness.live(body)
    scheduled = {block.at: _branch_condition(block, readers) for block in body.blocks}
    for block in body.blocks:
        _check_inserted_conditions(scheduled[block.at], live.live_out[block.at])
    made = {}
    for block in body.blocks:
        alive = {value for value in live.live_out[block.at] if value.flags}
        preserve = set()
        for op in reversed(scheduled[block.at]):
            if alive:
                preserve.add(id(op))
            alive.difference_update(op.defines)
            alive.update(value for value in op.uses if value.flags)
        made[block.at] = tuple(
            one for op in scheduled[block.at] for one in making.expand(op, preserve_flags=id(op) in preserve)
        )
    live = _phis_worth_keeping(body, made)
    return lir.LirBody(
        name=name,
        entry=body.entry,
        blocks=tuple(
            lir.LirBlock(
                at=block.at,
                insns=made[block.at],
                succ=block.succ,
                phis=tuple(
                    lir.Phi(
                        result=phi.result.id,
                        incoming=tuple((at, value.id) for at, value in phi.incoming.items()),
                    )
                    for phi in block.phis
                    if not phi.result.flags and phi.result.id in live
                ),
            )
            for block in body.blocks
        ),
        origin=dict(body.origin),
        pins=dict(getattr(body, "pins", {}) or {}),
    )


def _check_inserted_conditions(ops: tuple[mir.Op, ...], leaving: frozenset[mir.Value]) -> None:
    alive = {value for value in leaving if value.flags}
    for op in reversed(ops):
        preserved = alive - set(op.defines)
        if op.node is None and op.kind in (mir.Kind.ADD, mir.Kind.MUL) and preserved:
            raise Unlowered(f"inserted {op.kind} at {op.at:#x} crosses a live condition")
        alive.difference_update(op.defines)
        alive.update(value for value in op.uses if value.flags)


def _branch_condition(block: mir.MirBlock, readers: Counter[mir.Value]) -> tuple[mir.Op, ...]:
    """Keep a pure single-use comparison adjacent to its branch during selection.

    MIR's condition is a value. On x86 it lives in flags, so unrelated
    arithmetic between its definition and use cannot retain that ordering.
    Like SelectionDAG glue, adjacency is a backend requirement, not an
    obligation imposed on the optimization passes.
    """
    if not block.ops or block.ops[-1].kind is not mir.Kind.BRANCH:
        return block.ops
    branch = block.ops[-1]
    conditions = [value for value in branch.uses if value.flags]
    if len(conditions) != 1 or readers[conditions[0]] != 1:
        return block.ops
    for index, op in enumerate(block.ops[:-1]):
        if (
            op.op is ir.Operation.COMPARE
            and op.defines == (conditions[0],)
            and not (op.loads or op.stores or op.barrier)
            and all(isinstance(arg, (mir.Held, mir.Const, mir.Symbol)) for arg in op.args)
        ):
            return (*block.ops[:index], *block.ops[index + 1 : -1], op, branch)
    return block.ops


# A kind whose one operation is more than one instruction, and what it
# becomes. Intrinsic expansions belong here rather than in `expand` itself.
def _extract(op: mir.Op, lowering: "Lowering") -> tuple[ir.Semantics, ...]:
    match op.args, op.results:
        case (mir.Held(value=source, width=4), mir.Const(n=16)), (mir.Held(value=result, width=2),):
            discarded = ir.Held(lowering.fresh(), 2)
            return (
                ir.Semantics(ir.Operation.PUSH, "push", (), (ir.Held(source.id, 4),)),
                ir.Semantics(ir.Operation.POP, "pop", (discarded,), ()),
                ir.Semantics(ir.Operation.POP, "pop", (ir.Held(result.id, 2),), ()),
            )
    raise Unlowered(f"unsupported extraction at {op.at:#x}")


def _word_division(op: mir.Op, lowering: "Lowering") -> tuple[ir.Semantics, ...] | None:
    if len(op.args) != 2 or len(op.results) != 2:
        return None
    if not all(isinstance(arg, mir.Held) and arg.width == 2 for arg in (*op.args, *op.results)):
        return None
    dividend, divisor = map(operand, op.args)
    high = ir.Held(lowering.fresh(), 2)
    return (
        ir.Semantics(ir.Operation.EXTEND, "cwd", (high,), (dividend,)),
        ir.Semantics(ir.Operation.DIVIDE, "idiv", tuple(map(operand, op.results)), (high, dividend, divisor)),
    )


_EXPANDS: dict = {mir.Kind.EXTRACT: _extract, mir.Kind.DIVMOD: _word_division}


class Lowering:
    """One body being lowered, and the values the expansion invents.

    A MIR operation is usually one machine instruction. Where it is not --
    an intrinsic that becomes a load, a multiply and a shift -- the extra
    instructions need values nothing raised and ids no raised value has,
    and something has to own both. That is per body, because an id is only
    unique within one.

    The first instruction an operation becomes is its leader and carries
    the operation: its address, the bytes it stands for, and the operation
    itself, which is where its id lives. Every one after it is an insertion and says so the way
    phielim's and twoaddr's do -- no bytes of its own, no id, no op -- so
    layout's accounting still adds up.
    """

    def _widths(self, op: "mir.Op") -> tuple:
        """How wide each value an operand-less operation names is.

        The raise wrote it down: a folded site's answers are four bytes
        because the routine returns a long, and nothing about the `call`
        it replaced says so. Every consumer used to default to a word.
        """
        out: dict[int, int] = {}
        for one in (*op.results, *op.args):
            value, width = getattr(one, "value", None), getattr(one, "width", None)
            if value is not None and width:
                out[value.id] = width
        return tuple(sorted(out.items()))

    def _idiom(self, op: "mir.Op") -> tuple:
        """Where an operation that names no operand leaves what it writes.

        Only the restore idiom: three instructions behind one node, whose
        semantics have no destination to carry a value. The registers are
        the pair's own -- `push eax / pop ax / pop dx` puts the low half
        in ax and the high half in dx -- and which value is which is the
        register the raise saw it in, which for these is always BC's own
        because the idiom is BC's own.
        """
        if op.kind is mir.Kind.OPAQUE:
            return self._implicit_values(op, op.defines)
        if op.kind is mir.Kind.DIVMOD:
            # A folded site is a sequence too, and it leaves its answers in
            # registers its operands name nowhere: the one the routine
            # returned in and the one calls.py keeps the other in. Said
            # here so the allocator knows where they arrive -- until it
            # did, the only way to emit the site was the sequence frozen
            # at the raise, which is why a hoisted divide could not be
            # emitted at all.
            from qbopt import calls as machine

            folded = self._sites.get(op.id)
            if folded is None or len(op.results) != 2:
                return ()
            site = folded[0]
            other = machine.other_result(site)
            if other is None:
                return ()
            # By role, the way the raise ordered them: the visible answer
            # is the one the routine's own name promises.
            visible, kept = machine.RESULT, other
            first, second = (kept, visible) if site.name.upper() == machine.REMAINDER else (visible, kept)
            return tuple(
                (ir.Held(one.value.id, one.width), target.named(where, one.width))
                for one, where in zip(op.results, (first, second))
                if getattr(one, "value", None) is not None and one.value.id in self._read
            )
        if not isinstance(op.node, ir.Restore):
            return ()
        out = []
        for one in op.defines:
            if one.flags or one.id not in self._read:
                continue
            root = ir.ROOT.get(self._origin.get(one, -1), -1)
            if root not in mir.RESTORE_PAIR[op.node.pair]:
                raise Unlowered(f"{op.at:#06x}: the restore's {one} is in no register the idiom writes")
            out.append((ir.Held(one.id, 2), target.named(root, 2)))  # a half, and the idiom pops one word into each
        return tuple(out)

    def _abi(self, op: "mir.Op") -> tuple:
        """Which registers a call reads its arguments in.

        `op.args` are the arguments the routine's contract declares, in the
        order `runtime.slots` puts them, and this pairs each with its
        slot's register. The value is whichever one reaches the call -- a
        pass may have computed it anywhere -- and the register is the
        routine's, not wherever BC happened to keep it.

        Empty where nothing is established: the read set a call carries is
        every tracked register until a contract narrows it, and that is a
        liveness dependency rather than an argument list.
        """
        if op.kind is mir.Kind.OPAQUE:
            return self._implicit_values(op, op.uses)
        if op.id in self._sites:
            # These selected multi-instruction sequences still encode their
            # original addressing registers. Make that constraint explicit
            # so allocation supplies the current address value there.
            return tuple(
                (ir.Held(ref.base.id, 2), target.named(ref.addr.base, 2))
                for ref in op.loads
                if ref.base is not None and ref.addr is not None
            )
        if isinstance(op.node, ir.Restore):
            # The idiom reads the widened value in the pair's own register
            # -- `push eax` -- and names it nowhere, so without this the
            # allocation left the answer in ebx and the idiom pushed
            # whatever eax happened to hold. Only that one: the other use
            # is the previous contents of the half it writes, which is a
            # merge and not an input.
            source, _into = mir.RESTORE_PAIR[op.node.pair]
            return tuple(
                (ir.Held(one.id, 4), target.named(source, 4))
                for one in op.uses
                if not one.flags and one not in op.merges and ir.ROOT.get(self._origin.get(one, -1), -1) is source
            )
        if op.kind is not mir.Kind.CALL:
            return ()
        # The raise's own answer first: `args_known` is false for a call
        # whose contract declares nothing and for one to the program's own
        # code, which has no runtime contract at all -- not for a routine
        # established to read no register, which is a fact and says so.
        # Where nothing says what the callee reads, nothing may be moved
        # out from under it.
        if not op.args_known:
            raise Unlowered(f"{op.at:#06x}: {self._calls.get(op.at) or 'this call'}'s interface is not established")
        routine = self._contracts.get(op.at)
        if routine is None:
            raise Unlowered(f"{op.at:#06x}: no contract for {self._calls.get(op.at)}")
        if not runtime.established_inputs(routine):
            # Nothing is known about what it reads, and the raise reads
            # every tracked register for it. Emitting it unconstrained lets
            # the allocation move whatever it was handed; refusing keeps
            # BC's own layout, which is the answer that still works.
            raise Unlowered(f"{op.at:#06x}: {self._calls.get(op.at)} has no established inputs")
        where = runtime.slots(routine)
        if not where:
            return ()
        # A declared contract whose arguments do not answer it: emitting the
        # call unconstrained would say the routine reads nothing, which is
        # the one thing known to be false about it.
        if len(where) != len(op.args):
            raise Unlowered(f"{op.at:#06x}: {len(op.args)} arguments for {len(where)} declared inputs")
        made = []
        for one, slot in zip(op.args, where):
            if not isinstance(one, mir.Held) or one.value.id is None or one.value.flags:
                raise Unlowered(f"{op.at:#06x}: {one!r} is not a value a register can hold")
            made.append((ir.Held(one.value.id, one.width), mir.AS_NAMED[slot]))
        return tuple(made)

    def _implicit_values(self, op: mir.Op, values: tuple[mir.Value, ...]) -> tuple:
        """Unencoded operands of an opaque instruction still have machine locations."""
        if not isinstance(op.node, ir.Opaque):
            return ()
        registers = {
            ir.ROOT.get(one.register, one.register): one.register
            for one in InstructionInfoFactory().info(op.node.insn.insn).used_registers()
        }
        return tuple(
            (ir.Held(value.id, RegisterExt.size(register)), register)
            for value in values
            if not value.flags
            if (register := registers.get(self._origin.get(value))) is not None
        )

    def __init__(self, body: "mir.MirBody", read: set, calls: dict, absorbed, contracts=None, coverage=None) -> None:
        self._read = read
        self._coverage = coverage or {}
        self._origin = body.origin
        self._calls = calls
        # The same answer the raise used, per call site. Looked up here
        # only where the caller had none to give.
        # The map the caller chose, or one built here for a caller with
        # none to give -- a tool, a test. `raise_body` does the same, from
        # the same function, so the two boundaries cannot differ; the
        # emission path passes one object to both and never reaches this.
        # An explicitly empty map is a map: it says this call site was
        # decided about and the answer was nothing.
        # The caller's, always. Built here from `calls` alone it would
        # lack the family and the module's own PUBDEF names, and the raise
        # -- which has both -- would establish a contract this did not.
        self._contracts = contracts
        self._absorbed = absorbed
        # Where a folded site's answers arrive is in the record, so a
        # caller with only the set of absorbed ids says the site is folded
        # and nothing more.
        self._sites = absorbed if isinstance(absorbed, dict) else {}
        every = [
            one.id for block in body.blocks for op in block.ops for one in (*op.defines, *op.uses) if one.id is not None
        ]
        every += [phi.result.id for block in body.blocks for phi in block.phis]
        self._next = max(every, default=0) + 1

    def fresh(self) -> int:
        """A value id nothing in this body already uses."""
        self._next += 1
        return self._next - 1

    def expand(self, op: "mir.Op", *, preserve_flags: bool = True) -> "tuple[lir.Insn, ...]":
        """Every instruction this operation becomes, the leader first."""
        from qbopt import lir

        made = _EXPANDS.get(op.kind)
        parts = made(op, self) if made is not None else None
        if op.kind is mir.Kind.CONVERT and not preserve_flags:
            parts = _sign_word(op) or parts
        if op.kind is mir.Kind.MUL and not preserve_flags:
            parts = _scaled(op) or parts
        if not parts:
            # The site's own sequence emits it, so there is nothing for
            # this to say -- while it is still that operation. A pass may
            # rewrite a folded site into something else and keep its id,
            # which is how the bytes it stood for go on being accounted
            # for: the reused divide becomes a copy of the answer the
            # divide before it computed. Identified by id alone that copy
            # came out with nothing to emit, and every body holding one
            # left the route that can spill. An operation with no node has
            # no site's bytes to be emitted from.
            folded = op.id in self._absorbed and op.node is not None
            what = None if folded else current(op, as_a_value)
            # An instruction's dataflow is what its own operands name. The
            # two used to be separate -- `defines` from the operation and
            # the operands from BC's registers -- and once the operands
            # became values a destination nothing reads was a Held the
            # allocator had never heard of: `value#91 at width 2 has no
            # register`.
            # Roles decide wherever there are operands. An operation that
            # names none -- a call, whose results the runtime hands back in
            # registers it mentions nowhere -- keeps what it always said,
            # or a spilled call result gets reloads and no store.
            speaks = what is not None and (what.dests or what.sources)
            made = tuple(_written(what.dests)) if speaks else ()
            read = tuple(_read(what)) if speaks else ()
            return (
                lir.Insn(
                    at=op.at,
                    covers=op.covers,
                    what=what,
                    defines=made
                    if speaks and op.kind is not mir.Kind.CALL
                    else tuple(one.id for one in op.defines if not one.flags and one.id in self._read),
                    uses=read if speaks else tuple(one.id for one in op.uses if not one.flags),
                    requires=self._abi(op),
                    clobbers=_clobbers(op, self._calls),
                    spread=(op.covers, *op.extra_covers) if op.extra_covers else self._coverage.get(op.id, ()),
                    delivers=self._idiom(op),
                    widths=self._widths(op) if what is None else (),
                    op=op,
                    symbol=op.symbol,
                ),
            )
        # The leader keeps the operation's identity -- its address, the
        # bytes it stands for, the operation itself -- and nothing else.
        # What it reads and writes is its own first step's, the way every
        # instruction after it is its own: a leader claiming the whole
        # operation's operands would say the product is live from the load
        # that starts the run. The effect the operation had belongs to the
        # run, not to any one instruction in it.
        return (
            lir.Insn(
                at=op.at,
                covers=op.covers,
                what=parts[0],
                defines=tuple(_written(parts[0].dests)),
                uses=tuple(_read(parts[0])),
                clobbers=frozenset(),
                op=op,
            ),
            *(_follows(op, one) for one in parts[1:]),
        )


def _scaled(op: mir.Op) -> tuple[ir.Semantics, ...] | None:
    """A low product by a power of two, with no observable multiply flags."""
    if len(op.args) != 2 or len(op.results) != 1:
        return None
    source, scale = op.args
    if isinstance(source, mir.Const):
        source, scale = scale, source
    result = op.results[0]
    if not (
        isinstance(source, mir.Held) and isinstance(scale, mir.Const) and isinstance(result, mir.Held)
        and source.width == result.width and source.width in (2, 4)
        and 1 < scale.n < 1 << (source.width * 8) and scale.n & (scale.n - 1) == 0
        and not op.loads and not op.stores and not op.merges
    ):
        return None
    return (
        ir.Semantics(
            ir.Operation.BINARY, "shl", (operand(result),),
            (operand(source), ir.Imm(scale.n.bit_length() - 1, 1)),
        ),
    )


def _sign_word(op: mir.Op) -> tuple[ir.Semantics, ...] | None:
    if (
        op.op is not ir.Operation.EXTEND
        or len(op.args) != 1
        or len(op.results) != 1
        or not isinstance(op.args[0], mir.Held)
        or not isinstance(op.results[0], mir.Held)
        or op.args[0].width != op.results[0].width
        or op.args[0].width not in (2, 4)
    ):
        return None
    source, result = operand(op.args[0]), operand(op.results[0])
    return (
        ir.Semantics(ir.Operation.MOVE, "mov", (result,), (source,)),
        ir.Semantics(ir.Operation.BINARY, "sar", (result,), (result, ir.Imm(source.width * 8 - 1, 1))),
    )


def _follows(op: "mir.Op", what: "ir.Semantics") -> "lir.Insn":
    """One instruction an expansion inserted, beside the operation it came from.

    No bytes: `covers=(at, at)` is what an inserted instruction claims, and
    claiming the operation's own span twice is how layout came to say one
    byte was held by more than one op. Its values are the ones its own
    semantics name, because nothing raised it and `op.defines` describes
    the operation rather than this piece of it.
    """
    from qbopt import lir

    return lir.Insn(
        at=op.at,
        covers=(op.at, op.at),
        what=what,
        defines=tuple(_written(what.dests)),
        uses=tuple(_read(what)),
        clobbers=frozenset(),
        op=None,
    )


def _phis_worth_keeping(body: "mir.MirBody", made: dict) -> "frozenset[int]":
    """Which phi results this body still reads, transitively.

    The raise puts a phi on every register live around a loop, which is
    what SSA over machine state means. Once an operation says what it
    computes, most of those are read by nothing -- and `phielim` still
    materialises a copy on every edge for each, including edges whose
    value has no definition at all. lngmix's entry block copied an
    undefined ax into cx, clobbering the accumulator, and printed
    3419650 for 142900.

    Rooted in what the lowered instructions read, closed backwards
    through the phis' own inputs: a phi feeding a live phi is live, and
    an input read on its own account stays live through that use.
    """
    wanted = {value for insns in made.values() for one in insns for value in one.uses}
    phis = {
        phi.result.id: [value.id for value in phi.incoming.values()]
        for block in body.blocks
        for phi in block.phis
        if not phi.result.flags
    }
    changing = True
    while changing:
        changing = False
        for result, incoming in phis.items():
            if result in wanted and not set(incoming) <= wanted:
                wanted.update(incoming)
                changing = True
    return frozenset(wanted)


def _named(where: tuple) -> "list[int]":
    """The abstract values an operand list names, `ir.values` deciding."""
    return [one.value for operand in where for one in ir.values(operand)]


def _written(dests: tuple) -> "list[int]":
    """The values an instruction writes: a destination that *is* a value.

    Not every value a destination names. A cell names the value that
    computed its address, and `mov [es:bx],7` writes memory and no value
    at all -- taking every name here made the store define the pointer it
    stores through, so the address was born at the store and dead before
    it, and the allocator was free to put something else there.
    """
    return [one.value for one in dests if isinstance(one, ir.Held)]


def _read(what: "ir.Semantics") -> "list[int]":
    """The values an instruction reads: its sources, and the addresses its
    destinations are reached by -- a cell is written *through* a value."""
    return _named(what.sources) + [
        one.value for where in what.dests if not isinstance(where, ir.Held) for one in ir.values(where)
    ]


def clobbering(op: "mir.Op") -> "frozenset[Register_]":
    """What an absorbed divide destroys, for a caller with no call map.

    The allocation route asks this. A real call's contract is keyed by its
    address and needs that map, and answering "everything" without it
    would forbid every register to every value living across any call --
    over-stating in the safe direction, but a different question from the
    one this route is correcting.
    """
    from qbopt import mir

    return _clobbers(op, {}) if op.kind is mir.Kind.DIVMOD else frozenset()


def _clobbers(op: "mir.Op", calls: dict[int, str]) -> "frozenset[Register_]":
    """Which registers this instruction destroys without naming them.

    Only a call, and only from `runtime.py`'s own contract for the routine
    -- which is measured against the runtime's source, not assumed. An
    unestablished contract clobbers every register, and saying so is the
    safe direction: over-stating what a call destroys only keeps a value
    out of a register, while under-stating it puts a live value in one the
    call overwrites.
    """
    from qbopt import mir
    from qbopt import runtime

    if op.kind is mir.Kind.DIVMOD:
        # An absorbed divide is emitted as a sequence, not as one
        # instruction, and it writes registers none of its operands name:
        # the dividend's, the divisor's, idiv's own edx, and wherever the
        # answer it was not asked for is kept. Declared here because this
        # is where a machine fact belongs, and because nothing else tells
        # the allocator -- a value living in ecx across the site was not
        # interfering with anything it could see.
        from qbopt import calls as machine

        return frozenset({machine.RESULT, machine.DIVISOR, Register.EDX, machine.OTHER} & set(target.AVAILABLE))
    if op.kind is not mir.Kind.CALL:
        return frozenset()
    contract = runtime.contract(calls.get(op.at))
    if contract is None:
        return frozenset(target.AVAILABLE)
    names = _names()
    return frozenset(
        register
        for register in target.AVAILABLE
        for named in (contract.clobbers or ())
        if named.value.lower() in names.get(register, ())
    )


# Each allocatable register by the names runtime.py's own Reg enum uses:
# `ax` for eax, since a contract is written about the 16-bit machine.
def _names() -> dict:
    return {
        register: {target.name_of(target.named(register, 2)), target.name_of(target.named(register, 4))}
        for register in target.AVAILABLE
    }
