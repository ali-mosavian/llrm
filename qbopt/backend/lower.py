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

from qbopt.model import ir
from qbopt.model import mir
from qbopt.abi import runtime
from qbopt.backend import target
from qbopt.backend import division
from qbopt.analysis import liveness
from qbopt.backend import arithmetic
from qbopt.model.floating import Format
from qbopt.backend import cpu as targets
from qbopt.objectfile.module import Addr
from qbopt.model.floating import Rounding


def operand(arg: mir.Arg) -> ir.Loc:
    """One MIR operand as the machine's."""
    if isinstance(arg, mir.Held):
        return ir.Held(value=arg.value.id, width=arg.width)
    if isinstance(arg, mir.Const):
        return ir.Imm(value=arg.n, width=arg.width)
    if isinstance(arg, mir.Symbol):
        return ir.Imm(arg.addend, arg.width, Addr(arg.space, arg.offset, arg.index))
    if isinstance(arg, mir.FrameAddress):
        return ir.Address(
            Addr(mir.Space.FRAME, arg.offset),
            Register.BP,
            offset=arg.offset,
            disp_width=1 if -128 <= arg.offset <= 127 else 2,
        )
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
    mir.Kind.SIGN_EXTEND: (ir.Operation.EXTEND, "movsx"),
    mir.Kind.ZERO_EXTEND: (ir.Operation.EXTEND, "movzx"),
    mir.Kind.LOAD: (ir.Operation.MOVE, "mov"),
    mir.Kind.STORE: (ir.Operation.MOVE, "mov"),
    mir.Kind.JUMP: (ir.Operation.JUMP, "jmp"),
    # Strength reduction writes both: one multiply in a preheader and one
    # add at a latch, where BC recomputed the product every iteration. The
    # multiply is the two-operand `imul r,r/m`, which writes its
    # destination and nothing else -- the widening form writes dx:ax, and
    # an operation MIR invented defines one value.
    mir.Kind.MUL: (ir.Operation.MULTIPLY, "imul"),
    mir.Kind.ADD: (ir.Operation.BINARY, "add"),
    mir.Kind.SHL: (ir.Operation.BINARY, "shl"),
    mir.Kind.SHR: (ir.Operation.BINARY, "shr"),
    mir.Kind.SAR: (ir.Operation.BINARY, "sar"),
}


_BRANCHES = {
    mir.Kind.EQ: "je",
    mir.Kind.NE: "jne",
    mir.Kind.LT: "jl",
    mir.Kind.LE: "jle",
    mir.Kind.GT: "jg",
    mir.Kind.GE: "jge",
    mir.Kind.BELOW: "jb",
    mir.Kind.BELOW_EQ: "jbe",
    mir.Kind.ABOVE: "ja",
    mir.Kind.ABOVE_EQ: "jae",
}


# The instruction each kind is, for MIR that says only what it computes.
_NAMED: dict[mir.Kind, tuple[ir.Operation, str]] = {
    **_MACHINE,
    mir.Kind.SUB: (ir.Operation.BINARY, "sub"),
    mir.Kind.AND: (ir.Operation.BINARY, "and"),
    mir.Kind.OR: (ir.Operation.BINARY, "or"),
    mir.Kind.XOR: (ir.Operation.BINARY, "xor"),
    mir.Kind.NEG: (ir.Operation.UNARY, "neg"),
    mir.Kind.NOT: (ir.Operation.UNARY, "not"),
    mir.Kind.DIVMOD: (ir.Operation.DIVIDE, "idiv"),
    mir.Kind.UDIVMOD: (ir.Operation.DIVIDE, "div"),
    mir.Kind.ADDRESS: (ir.Operation.ADDRESS, "lea"),
    mir.Kind.ARG: (ir.Operation.PUSH, "push"),
    mir.Kind.CONCAT: (ir.Operation.MOVE, ""),
    mir.Kind.BRANCH: (ir.Operation.BRANCH, ""),
    mir.Kind.FADD: (ir.Operation.FLOAT_ARITH, "fadd"),
    mir.Kind.FSUB: (ir.Operation.FLOAT_ARITH, "fsub"),
    mir.Kind.FMUL: (ir.Operation.FLOAT_ARITH, "fmul"),
    mir.Kind.FDIV: (ir.Operation.FLOAT_ARITH, "fdiv"),
    mir.Kind.FNEG: (ir.Operation.FLOAT_UNARY, "fchs"),
    mir.Kind.FABS: (ir.Operation.FLOAT_UNARY, "fabs"),
    mir.Kind.FSQRT: (ir.Operation.FLOAT_UNARY, "fsqrt"),
    # FloatAlloc picks fcom, fcomp or fcompp by what dies.
    mir.Kind.FCOMPARE: (ir.Operation.COMPARE, "fcom"),
    mir.Kind.FCHECK: (ir.Operation.NOTHING, "fwait"),
    mir.Kind.CALL: (ir.Operation.CALL, "call"),
    mir.Kind.RETURN: (ir.Operation.RETURN, ""),
}
# An x87 compare's answer reaches the flags through sahf, C0 into CF and C3
# into ZF, where an unsigned compare leaves it.
_UNORDERED = {
    mir.Kind.LT: mir.Kind.BELOW,
    mir.Kind.LE: mir.Kind.BELOW_EQ,
    mir.Kind.GT: mir.Kind.ABOVE,
    mir.Kind.GE: mir.Kind.ABOVE_EQ,
}
_INTEGERS = frozenset({Format.SIGNED16, Format.SIGNED32, Format.SIGNED64})
# Where an operation with no node of its own returns values and a call
# delivers them, by position: every x86 C convention's AX, then DX.
_RETURNED = (Register.EAX, Register.EDX)


def _instruction(op: mir.Op) -> tuple[ir.Operation, str] | None:
    if op.kind is mir.Kind.SUB and not op.results:
        return ir.Operation.COMPARE, "cmp"
    if op.kind is mir.Kind.FLOAD and op.floating is not None and op.floating.inputs[0] is Format.UNSIGNED64:
        raise Unlowered("unsigned 64-bit floating load needs target-specific expansion")
    if op.kind is mir.Kind.FSTORE and op.floating is not None and op.floating.result is Format.UNSIGNED64:
        raise Unlowered("unsigned 64-bit floating store needs target-specific expansion")
    if op.kind is mir.Kind.FLOAD and op.floating is not None:
        return ir.Operation.FLOAT_LOAD, "fild" if op.floating.inputs[0] in _INTEGERS else "fld"
    if op.kind is mir.Kind.FSTORE and op.floating is not None:
        if op.floating.result not in _INTEGERS:
            return ir.Operation.FLOAT_STORE, "fstp"
        # Toward zero is fisttp, which a 387 lacks: FloatAlloc spells it for one.
        return ir.Operation.FLOAT_STORE, "fisttp" if op.floating.rounding is Rounding.TOWARD_ZERO else "fistp"
    return _NAMED.get(op.kind)


def named(body: "mir.MirBody") -> "mir.MirBody":
    """Every operation with no machine form given one from what it computes.

    An operation a raise states only as a kind reaches here with an empty
    `op` and `name`; the rest of lowering reads those, so they are filled
    first, and only here, where naming the machine is this layer's job.
    """
    from dataclasses import replace

    compared = {
        value for block in body.blocks for op in block.ops if op.kind is mir.Kind.FCOMPARE for value in op.defines
    }

    def one(op: mir.Op) -> mir.Op:
        if op.kind is mir.Kind.BRANCH and op.test in _UNORDERED and compared.intersection(op.uses):
            op = replace(op, test=_UNORDERED[op.test])
        if op.op is not ir.Operation.NOTHING or op.name or op.kind is mir.Kind.NOTHING or op.made is not None:
            return op
        found = _instruction(op)
        if found is None:
            raise Unlowered(f"{op.at:#06x}: no instruction for {op.kind}")
        return replace(op, op=found[0], name=found[1])

    return replace(body, blocks=tuple(replace(block, ops=tuple(map(one, block.ops))) for block in body.blocks))


def _indirect_call(op: mir.Op) -> bool:
    """Whether a generated CALL's sole encoded operand is its target.

    The frontend records this control-flow fact explicitly.  Inferring it
    from `symbol` confused a call whose relocation moved with an indirect
    call; inferring it from the argument count confused B$PSSD's implicit DI
    input with a target.  Stack arguments are preceding ARG operations, so
    the one argument here is solely the encoded destination.
    """
    if not op.indirect:
        return False
    if op.kind is not mir.Kind.CALL or len(op.args) != 1 or not isinstance(op.args[0], mir.Held):
        raise Unlowered(f"{op.at:#06x}: indirect call needs exactly one value target")
    return True


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
    if op.kind is mir.Kind.FCHECK:
        return ir.Semantics(ir.Operation.NOTHING, "wait", (), ())
    if op.kind is mir.Kind.JUMP and op.target is not None:
        return ir.Semantics(ir.Operation.JUMP, "jmp", (), (), op.target)
    if (
        op.node is None
        and op.kind in (mir.Kind.CALL, mir.Kind.RETURN)
        and op.op in (ir.Operation.CALL, ir.Operation.RETURN)
    ):
        # Results arrive and return values leave in registers the ABI says,
        # rather than encoded operands. An indirect call's target is the one
        # exception: it is the r/m16 source named by the call instruction.
        located = place or _place
        sources = tuple(located(one, (), i) for i, one in enumerate(op.args)) if _indirect_call(op) else ()
        return ir.Semantics(op.op, op.name, (), sources, indirect=op.indirect)
    if not op.args and not op.results and op.raised is None and op.kind is not mir.Kind.BRANCH:
        return None  # nothing to build one from
    # A value resolves to the register the original instruction had in the
    # same position where there is one. Emitting ir.Held instead hands the
    # choice to the allocation, and the allocation is applied to the ops it
    # re-encodes and not to the ones emitted from their own bytes -- so the
    # two disagree, and lngmix printed 110 for 142900 with the dividend
    # deleted. Where there is no such operand -- an operation a pass
    # invented -- ir.Held is the only honest answer and select resolves it.
    was_op, name = _MACHINE.get(op.kind, (op.op, op.name))
    if op.kind is mir.Kind.BRANCH and not name and op.test in _BRANCHES:
        was_op, name = ir.Operation.BRANCH, _BRANCHES[op.test]
    if op.kind is mir.Kind.NOTHING and op.op is not ir.Operation.NOTHING:
        was_op, name = ir.Operation.NOTHING, "nop"
    args = op.args
    if op.kind is mir.Kind.RETURN:
        # Returned values constrain allocation but RET only encodes stack cleanup.
        args = tuple(one for one, original in zip(args, was.sources if was else ()) if isinstance(original, ir.Imm))
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
            elif isinstance(one, ir.Mem) and isinstance(was, mir.Cell):
                base = ir.Held(was.ref.base.id, was.ref.base_width) if was.ref.base is not None else None
                out.append(replace(one, base=base, selector=_selector(was.ref)))
            else:
                out.append(one)
        return tuple(out)

    return replace(
        what,
        dests=named(what.dests, op.results),
        sources=named(what.sources, op.args),
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
        # Valueized in place. Older raised bodies may carry a rewritten
        # machine form, and returning it untouched would leave BC's original
        # registers among abstract values with nothing for the allocator to
        # rewrite.
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
    if place is None and op.made is None and any(ref.pointer for ref in (*op.loads, *op.stores)):
        return None
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
    return replace(what, dests=dests, sources=sources)


def _machine(one, had: tuple, index: int):
    if not isinstance(one, mir.MemRef):
        return one
    if one.pointer:
        raise Unlowered("whole-pointer memory operand escaped pointer materialization")
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
        return replace(made, base=base, selector=_selector(one))
    if before is not None:
        return ir.Mem(one.addr, one.width, before.through, before.offset, before.disp_width, selector=_selector(one))
    return replace(_addressed(one), selector=_selector(one))


def _selector(ref: "mir.MemRef") -> "ir.Held | None":
    """The value a far cell's segment is in, for allocation to place."""
    from qbopt.objectfile.module import Space

    if ref.segment is None or ref.addr is None or ref.addr.space is not Space.FAR:
        return None
    return ir.Held(ref.segment.id, 2)


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

    A literal displacement through an SSA base retains that base; allocation
    chooses an address register. Far pointers and the stack remain refused.
    """
    from qbopt.objectfile.module import Space

    addr = one.addr
    if addr is None:
        raise Unlowered("a cell with no address cannot be encoded: nothing says which register reaches it")
    if addr.space is Space.FRAME:
        return ir.Mem(addr, one.width, Register.BP, 0, 2)
    if addr.space is Space.LITERAL and one.base is not None:
        return ir.Mem(addr, one.width, Register.NONE, 0, 2)
    if addr.space is Space.FAR and one.base is not None and one.segment is not None:
        # MIR names both parts of the address as values. _machine() attaches
        # the offset value as Mem.base, allocation gives it an addressing
        # register, and Lowering._abi pins the selector value to the segment
        # register carried by addr. Nothing here has to guess either one.
        if addr.segment == Register.NONE:
            # MIR names the selector only as a value, which _abi pins to ES.
            addr = replace(addr, segment=Register.ES)
        return ir.Mem(addr, one.width, Register.NONE, addr.disp, 2)
    if addr.space in (Space.SEGMENT, Space.EXTERNAL):
        # An indexed element says which register reaches it: `addr.base` is
        # part of the address's own identity, because two elements at the
        # same displacement are not the same address unless that register
        # agrees. So the encoding is not a guess -- it is written down.
        #
        # EXTERNAL encodes identically. Which relocation namespace names the
        # cell -- EXTDEF against SEGDEF -- is the whole of the difference,
        # and the instruction is the same DS-relative displacement either
        # way. It is here because raising_defseg synthesizes a store to
        # b$seg, and nothing else in the corpus had ever built an EXTERNAL
        # operand rather than carrying BC's own bytes for one.
        return ir.Mem(addr, one.width, addr.base, 0, 2)
    raise Unlowered(f"a cell at {addr} in {addr.space} has no encoding this can derive")


def lowered(
    name: str,
    body: "mir.MirBody",
    calls: dict[int, str] | None,
    absorbed: "set[int] | dict[int, tuple] | None",
    contracts: "dict[int, object]",
    coverage: "dict[int, tuple] | None" = None,
    cpu: str | targets.Profile = "386",
    *,
    pointer_model=None,
    noreturn: bool = False,
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
    from qbopt.model import lir
    from qbopt.analysis import ssa
    from qbopt.backend import lower_floats
    from qbopt.backend import lower_switches

    # Width does not identify a type here: a folded DOUBLE store and C's
    # int64 are both eight bytes.  The C path runs lower_int64 before this
    # generic boundary; `_constant_store` below splits an eight-byte memory
    # bit pattern without pretending it is integer arithmetic.

    try:
        body = lower_switches.expanded(body)
    except ValueError as error:
        raise Unlowered(str(error)) from error
    body = named(body)
    lower_floats.checked(body)
    body = ssa.pruned_phis(body, {phi.result for block in body.blocks for phi in block.phis if not phi.result.flags})

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
    making = Lowering(body, read, calls, absorbed or (), contracts, coverage, cpu, pointer_model=pointer_model)
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
    uses = Counter(value for insns in made.values() for one in insns for value in one.uses)
    uses.update(
        held.value for insns in made.values() for one in insns for held, _ in one.requires if held.value not in one.uses
    )
    uses.update(value.id for block in body.blocks for phi in block.phis for value in phi.incoming.values())
    made = {at: _memory_arguments(insns, uses, making._exposed) for at, insns in made.items()}
    made = {at: _immediate_arguments(insns, uses) for at, insns in made.items()}
    made = {at: _rematerialized_arguments(insns, uses, making._exposed) for at, insns in made.items()}
    live = _phis_worth_keeping(body, made)
    return lir.LirBody(
        name=name,
        entry=body.entry,
        noreturn=noreturn,
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
        inputs=frozenset(value.id for value in liveness.entry_values(body) if not value.flags and value.id is not None),
        ordered=True,
        pins={**body.pins, **{value: Register.ES for value in body.values if body.origin.get(value) == Register.ES}},
    )


def _check_inserted_conditions(ops: tuple[mir.Op, ...], leaving: frozenset[mir.Value]) -> None:
    alive = {value for value in leaving if value.flags}
    for op in reversed(ops):
        preserved = alive - set(op.defines)
        if (
            op.node is None
            and op.kind in (mir.Kind.ADD, mir.Kind.MUL, mir.Kind.SMULHI, mir.Kind.PTR_OFFSET)
            and preserved
        ):
            raise Unlowered(f"inserted {op.kind} at {op.at:#x} crosses a live condition")
        alive.difference_update(op.defines)
        alive.update(value for value in op.uses if value.flags)


def _rematerialized_arguments(insns, uses, exposed):
    """Select immediate pushes without keeping literal addresses live across calls."""
    from qbopt.model import lir

    literals = {}
    consumed = Counter()
    out = []
    for one in insns:
        match one.what:
            case ir.Semantics(ir.Operation.PUSH, "push", (), (ir.Held(value, width),)):
                if (
                    value in literals
                    and not (one.clobbers or one.requires or one.delivers or one.spread)
                    and not one.defines
                    and one.uses == (value,)
                ):
                    definition, immediate = literals[value]
                    if width == immediate.width:
                        one = replace(
                            one,
                            what=replace(one.what, sources=(immediate,)),
                            uses=(),
                            op=definition.op,
                            symbol=True if immediate.address is not None else False,
                        )
                        consumed[value] += 1
        for value in one.defines:
            literals.pop(value, None)
        match one.what:
            case ir.Semantics(ir.Operation.MOVE, "mov", (ir.Held(value, width),), (ir.Imm() as immediate,)):
                if (
                    width == immediate.width
                    and width in (2, 4)
                    and one.defines == (value,)
                    and not (one.uses or one.clobbers or one.requires or one.delivers or one.spread)
                ):
                    literals[value] = (one, immediate)
        out.append(one)
    dead = {
        id(definition)
        for value, (definition, _) in literals.items()
        if consumed[value] == uses[value] and consumed[value] and value not in exposed
    }
    return tuple(lir.without(out, lambda one: id(one) in dead))


def _memory_arguments(insns, uses, exposed):
    """Fold a single-use load into the PUSH that consumes it.

    This is instruction selection, not memory forwarding: the read stays at
    the call site, and candidates survive only instructions that cannot write
    memory or a fixed address register.  Start with incoming frame arguments,
    whose storage and address are stable across the call setup.  Folding an
    arbitrary local can instead extend an address live range and increase
    pressure (FPDEEP spilled all nine float conversions that way).
    """
    from qbopt.model import lir

    loaded = {}
    consumed = Counter()
    out = []
    for one in insns:
        folded = False
        match one.what:
            case ir.Semantics(ir.Operation.PUSH, "push", (), (ir.Held(value, width),)) if value in loaded:
                definition, source = loaded[value]
                if (
                    width == source.width in (2, 4)
                    and uses[value] == 1
                    and value not in exposed
                    and not (one.defines or one.clobbers or one.requires or one.delivers or one.spread)
                ):
                    one = replace(
                        one,
                        what=replace(one.what, sources=(source,)),
                        uses=(),
                        op=definition.op,
                        symbol=definition.symbol,
                    )
                    consumed[value] += 1
                    folded = True

        what = one.what
        writes_memory = what is None or any(isinstance(dest, ir.Mem) for dest in what.dests)
        writes_memory = writes_memory or bool(getattr(one.op, "stores", ()))
        writes_fixed = what is None or any(isinstance(dest, ir.Reg) for dest in what.dests)
        barrier = what is None or what.op in (
            ir.Operation.BARRIER,
            ir.Operation.CALL,
            ir.Operation.RETURN,
        )
        if writes_memory or writes_fixed or barrier or one.clobbers:
            loaded.clear()

        for value in one.defines:
            loaded.pop(value, None)
        if not folded:
            match what:
                case ir.Semantics(ir.Operation.MOVE, "mov", (ir.Held(value, width),), (ir.Mem() as source,)):
                    if (
                        width == source.width in (2, 4)
                        and source.addr is not None
                        and source.addr.space is mir.Space.FRAME
                        and source.addr.disp >= 4
                        and ir.root(source.through) is not Register.ESP
                        and ir.root(source.index_through) is not Register.ESP
                        and not source.stack_argument
                        and one.defines == (value,)
                        and not (one.uses or one.clobbers or one.requires or one.delivers or one.spread)
                    ):
                        loaded[value] = (one, source)
        out.append(one)

    dead = {value for value in consumed if consumed[value] == uses[value] and consumed[value] and value not in exposed}
    previous = None
    while previous != tuple(id(one) for one in out):
        previous = tuple(id(one) for one in out)
        out = lir.without(out, lambda one: any(value in dead for value in one.defines))
    return tuple(out)


def _immediate_arguments(insns, uses):
    """Select a direct push for an adjacent, single-use immediate definition."""
    from qbopt.model import lir

    out = []
    index = 0
    while index < len(insns):
        pair = insns[index : index + 2]
        if len(pair) == 2 and all(not (one.clobbers or one.requires or one.delivers or one.spread) for one in pair):
            copy, push = pair
            match copy.what, push.what:
                case (
                    ir.Semantics(ir.Operation.MOVE, "mov", (ir.Held(value, width),), (ir.Imm() as immediate,)),
                    ir.Semantics(ir.Operation.PUSH, "push", (), (ir.Held(pushed, pushed_width),)),
                ):
                    if (
                        value == pushed
                        and width == pushed_width == immediate.width
                        and width in (2, 4)
                        and uses[value] == 1
                        and not copy.uses
                        and copy.defines == (value,)
                        and push.uses == (value,)
                        and not push.defines
                    ):
                        combined = replace(copy, what=replace(push.what, sources=(immediate,)), defines=(), uses=())
                        folded = lir.without((combined, push), lambda one: one is push)
                        if len(folded) == 1:
                            out.extend(folded)
                            index += 2
                            continue
        out.append(insns[index])
        index += 1
    return tuple(out)


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
        case (mir.Held(value=source, width=4), mir.Const(n=offset)), (mir.Held(value=result, width=2),) if offset in (
            0,
            16,
        ):
            # The halves of a sign extension are the word itself and its sign,
            # and x86 has an instruction for the second. Going through the
            # stack instead cost stride's loop four instructions for what
            # `cwd` does in one -- and the low half it popped was dead, the
            # divide already reading the word.
            word = lowering.sign_extended(source.id)
            if word is not None:
                kept = ir.Held(result.id, 2)
                if offset == 0:
                    return (ir.Semantics(ir.Operation.MOVE, "mov", (kept,), (operand(word),)),)
                # Only where a divide already wanted dx:ax. `cwd` pins its word
                # to ax and takes dx, which is free there and is a shuffle
                # anywhere else -- emitting it for every sign word cost
                # deedlines' actions3d 1.5% of its running time.
                if lowering.divides(result.id, word):
                    return (ir.Semantics(ir.Operation.EXTEND, "cwd", (kept,), (operand(word),)),)
            discarded = ir.Held(lowering.fresh(), 2)
            kept = ir.Held(result.id, 2)
            return (
                ir.Semantics(ir.Operation.PUSH, "push", (), (ir.Held(source.id, 4),)),
                ir.Semantics(ir.Operation.POP, "pop", (kept if offset == 0 else discarded,), ()),
                ir.Semantics(ir.Operation.POP, "pop", (discarded if offset == 0 else kept,), ()),
            )
    raise Unlowered(f"unsupported extraction at {op.at:#x}")


def _word_division(op: mir.Op, lowering: "Lowering") -> tuple[ir.Semantics, ...] | None:
    if len(op.args) != 2 or len(op.results) != 2:
        return None
    width = op.results[0].width if isinstance(op.results[0], mir.Held) else 0
    if width == 4 and op.node is not None:
        return None  # Legacy folded sites order results by their runtime entry point.
    if (
        width not in (2, 4)
        or not all(isinstance(arg, mir.Held) and arg.width == width for arg in (op.args[0], *op.results))
        or not isinstance(op.args[1], (mir.Held, mir.Const))
        or op.args[1].width != width
    ):
        return None
    dividend, divisor = map(operand, op.args)
    unsigned = op.kind is mir.Kind.UDIVMOD
    if isinstance(op.args[1], mir.Const) and not unsigned:
        reciprocal = division.reciprocal(
            dividend,
            op.args[1].n,
            tuple(map(operand, op.results)),
            lowering.fresh,
            lowering.cpu,
            remainder=op.results[1].value.id in lowering._read,
        )
        if reciprocal is not None:
            return reciprocal
    setup = ()
    if isinstance(op.args[1], mir.Const):
        held = ir.Held(lowering.fresh(), width)
        setup = (ir.Semantics(ir.Operation.MOVE, "mov", (held,), (divisor,)),)
        divisor = held
    high = ir.Held(lowering.fresh(), width)
    widened = (
        ir.Semantics(ir.Operation.MOVE, "mov", (high,), (ir.Imm(0, width),))
        if unsigned
        else ir.Semantics(ir.Operation.EXTEND, "cwd" if width == 2 else "cdq", (high,), (dividend,))
    )
    name = "div" if unsigned else "idiv"
    return (
        *setup,
        widened,
        ir.Semantics(ir.Operation.DIVIDE, name, tuple(map(operand, op.results)), (high, dividend, divisor)),
    )


def _concat(op: mir.Op, lowering: "Lowering") -> tuple[ir.Semantics, ...]:
    match op.args, op.results:
        case (mir.Held(width=2) | mir.Const(width=2), mir.Held(width=2) | mir.Const(width=2)), (mir.Held(width=4),):
            high, low = map(operand, op.args)
            return (
                ir.Semantics(ir.Operation.PUSH, "push", (), (high,)),
                ir.Semantics(ir.Operation.PUSH, "push", (), (low,)),
                ir.Semantics(ir.Operation.POP, "pop", (operand(op.results[0]),), ()),
            )
    raise Unlowered(f"unsupported concatenation at {op.at:#x}")


def _signed_high_product(op: mir.Op, lowering: "Lowering") -> tuple[ir.Semantics, ...]:
    if (
        len(op.args) != 2
        or len(op.results) != 1
        or not isinstance(result := op.results[0], mir.Held)
        or result.width not in (2, 4)
        or any(not isinstance(arg, (mir.Held, mir.Const)) or arg.width != result.width for arg in op.args)
    ):
        raise Unlowered(f"unsupported signed high product at {op.at:#x}")
    setup, sources = [], []
    for arg in op.args:
        source = operand(arg)
        if isinstance(arg, mir.Const):
            held = ir.Held(lowering.fresh(), result.width)
            setup.append(ir.Semantics(ir.Operation.MOVE, "mov", (held,), (source,)))
            source = held
        sources.append(source)
    low = ir.Held(lowering.fresh(), result.width)
    return (*setup, ir.Semantics(ir.Operation.MULTIPLY, "imul", (low, operand(result)), tuple(sources)))


def _pointer_offset(op: mir.Op, lowering: "Lowering") -> tuple[ir.Semantics, ...]:
    if lowering.pointer_model is None:
        raise Unlowered(f"pointer offset at {op.at:#x} needs an established pointer ABI")
    if len(op.args) != 2 or len(op.results) != 1:
        raise Unlowered(f"unsupported pointer offset at {op.at:#x}")
    try:
        return lowering.pointer_model.offset(*map(operand, op.args), operand(op.results[0]), lowering.fresh)
    except ValueError as error:
        raise Unlowered(str(error)) from error


def _pointer_access(op: mir.Op, lowering: "Lowering") -> tuple[ir.Semantics, ...] | None:
    references = {arg.ref for arg in (*op.args, *op.results) if isinstance(arg, mir.Cell) and arg.ref.pointer}
    if not references:
        return None
    if lowering.pointer_model is None:
        raise Unlowered(f"pointer access at {op.at:#x} needs an established pointer ABI")
    if len(references) != 1 or op.kind not in (mir.Kind.LOAD, mir.Kind.STORE):
        raise Unlowered(f"unsupported whole-pointer memory operation at {op.at:#x}")
    ref = next(iter(references))
    if ref.base is None or ref.base_width != 4 or ref.addr is not None or ref.segment is not None:
        raise Unlowered(f"whole-pointer access has an unnormalized address at {op.at:#x}")
    from qbopt.objectfile.module import Space

    offset = ir.Held(lowering.fresh(), 2)
    selector = ir.Reg(Register.ES, 2)
    cell = ir.Mem(Addr(Space.FAR, 0, segment=Register.ES), ref.width, base=offset)

    def place(arg, had, index):
        return cell if isinstance(arg, mir.Cell) and arg.ref == ref else as_a_value(arg, had, index)

    access = current(op, place)
    if access is None:
        raise Unlowered(f"whole-pointer access has no operation at {op.at:#x}")
    # Materialization is local to the memory instruction. Restore the segment
    # resource so other MIR addresses retain their address-space identity.
    return (
        ir.Semantics(ir.Operation.PUSH, "push", (), (selector,)),
        ir.Semantics(ir.Operation.PUSH, "push", (), (ir.Held(ref.base.id, 4),)),
        ir.Semantics(ir.Operation.POP, "pop", (offset,), ()),
        ir.Semantics(ir.Operation.POP, "pop", (selector,), ()),
        access,
        ir.Semantics(ir.Operation.POP, "pop", (selector,), ()),
    )


def _constant_store(op: mir.Op, lowering: "Lowering") -> tuple[ir.Semantics, ...] | None:
    match op.args, op.results:
        case (mir.Const(n=bits, width=8),), (mir.Cell(ref=ref),) if ref.width == 8:
            if ref.base is not None or ref.segment is not None or ref.addr is None:
                raise Unlowered("wide constant store needs a static address")
            return tuple(
                ir.Semantics(
                    ir.Operation.MOVE,
                    "mov",
                    (_addressed(replace(ref, width=4, addr=replace(ref.addr, disp=ref.addr.disp + offset))),),
                    (ir.Imm((bits >> (offset * 8)) & 0xFFFFFFFF, 4),),
                )
                for offset in (0, 4)
            )
    return _pointer_access(op, lowering)


def _fill(op: mir.Op, lowering: "Lowering") -> tuple[ir.Semantics, ...]:
    """`rep stos`: the count in cx, the value in the accumulator, the cells through es:di.

    A far cell's selector is an operand the allocation places in ES. Otherwise
    ES is set to DS for the fill and put back, so a selector living in it survives.
    """
    value, count, address, *selector = op.args
    name = {1: "stosb", 2: "stosw", 4: "stosd"}.get(value.width)
    if name is None:
        raise Unlowered(f"fill of {value.width}-byte cells at {op.at:#x}")
    setup = []

    def held(arg: mir.Arg, width: int) -> ir.Loc:
        if isinstance(arg, mir.Held):
            return operand(arg)
        into = ir.Held(lowering.fresh(), width)
        setup.append(ir.Semantics(ir.Operation.MOVE, "mov", (into,), (operand(arg),)))
        return into

    stored, counted, through = held(value, value.width), held(count, 2), held(address, 2)
    stepped, emptied = ir.Held(lowering.fresh(), 2), ir.Held(lowering.fresh(), 2)
    if selector:
        sources = (stored, counted, through, held(selector[0], 2))
        return (*setup, ir.Semantics(ir.Operation.FILL, name, (ir.Mem(None, 0), stepped, emptied), sources))
    extra = ir.Reg(Register.ES, 2)
    return (
        *setup,
        ir.Semantics(ir.Operation.PUSH, "push", (), (extra,)),
        ir.Semantics(ir.Operation.PUSH, "push", (), (ir.Reg(Register.DS, 2),)),
        ir.Semantics(ir.Operation.POP, "pop", (extra,), ()),
        ir.Semantics(ir.Operation.FILL, name, (ir.Mem(None, 0), stepped, emptied), (stored, counted, through, extra)),
        ir.Semantics(ir.Operation.POP, "pop", (extra,), ()),
    )


_EXPANDS: dict = {
    mir.Kind.FILL: _fill,
    mir.Kind.STORE: _constant_store,
    mir.Kind.EXTRACT: _extract,
    mir.Kind.DIVMOD: _word_division,
    mir.Kind.UDIVMOD: _word_division,
    mir.Kind.CONCAT: _concat,
    mir.Kind.SMULHI: _signed_high_product,
    mir.Kind.PTR_OFFSET: _pointer_offset,
}


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

    def _idiom(self, op: "mir.Op", speaks: bool = False) -> tuple:
        """Where an operation that names no operand leaves what it writes."""
        return tuple(dict.fromkeys((*self._selectors(op, speaks), *self._delivered(op))))

    def _selectors(self, op: "mir.Op", speaks: bool = False) -> tuple:
        """A selector is delivered in ES by whatever writes ES.

        The mirror of _abi's own rule for reading one. An operand carries the
        value where the operation has one -- `mov es,[d]` -- and where it has
        none the register is still written, so the value is still defined:
        a call clobbers ES and the next far access goes through whatever it
        left there. Saying so here is what lets the definition exist at all;
        without it lowering refuses a value no operand names.
        """
        carried = {one.value.id for one in op.results if isinstance(one, mir.Held)}
        # An operand that carries the selector is placed like any value; only
        # an operation with no operands still says ES.
        return tuple(
            (ir.Held(one.value.id, one.width), Register.ES)
            for one in op.results
            if not speaks and isinstance(one, mir.Held) and self._origin.get(one.value) == Register.ES
        ) + tuple(
            (ir.Held(one.id, 2), Register.ES)
            for one in op.defines
            if one.id not in carried and not one.flags and one.id in self._read and self._origin.get(one) == Register.ES
        )

    def _delivered(self, op: "mir.Op") -> tuple:
        """Where an operation that names no operand leaves what it writes.

        Only the restore idiom: three instructions behind one node, whose
        semantics have no destination to carry a value. The registers are
        the pair's own -- `push eax / pop ax / pop dx` puts the low half
        in ax and the high half in dx -- and which value is which is the
        register the raise saw it in, which for these is always BC's own
        because the idiom is BC's own.
        """
        if op.kind is mir.Kind.CALL:
            widths = dict(self._widths(op))
            where = dict(self._origin)
            if op.node is None:
                # A float result is on the x87, not in a register.
                integers = (one for one in op.defines if not one.flags and widths.get(one.id) != 10)
                where = {**dict(zip(integers, _RETURNED)), **where}
            return tuple(
                (ir.Held(value.id, widths.get(value.id, 2)), target.named(where[value], widths.get(value.id, 2)))
                for value in op.defines
                if not value.flags and value.id in self._read and value in where
            )
        if op.kind is mir.Kind.OPAQUE and not isinstance(op.node, ir.Restore):
            return self._implicit_values(op, op.defines)
        if op.kind is mir.Kind.DIVMOD:
            # A folded site is a sequence too, and it leaves its answers in
            # registers its operands name nowhere: the one the routine
            # returned in and the one calls.py keeps the other in. Said
            # here so the allocator knows where they arrive -- until it
            # did, the only way to emit the site was the sequence frozen
            # at the raise, which is why a hoisted divide could not be
            # emitted at all.
            from qbopt.legacy import calls as machine

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

    def _abi(self, op: "mir.Op", what: "ir.Semantics | None" = None) -> tuple:
        # A cell that names its selector value is placed by allocation; one
        # emitted without an operand for it still goes through ES.
        placed = {
            operand.selector.value
            for operand in ((*what.dests, *what.sources) if what is not None else ())
            if isinstance(operand, ir.Mem) and operand.selector is not None
        }
        selectors = (
            (ir.Held(ref.segment.id, 2), Register.ES)
            for ref in (*op.loads, *op.stores)
            if ref.segment is not None and ref.segment.id not in placed
        )
        return tuple(dict.fromkeys((*self._fixed_inputs(op), *selectors)))

    def _fixed_inputs(self, op: "mir.Op") -> tuple:
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
        if op.kind is mir.Kind.RETURN and op.node is None and op.args:
            # Placed by position, not by pinning the value: a pass may
            # replace the operand, and a pin stays with the value it named.
            returned = []
            integers = [arg for arg in op.args if not (isinstance(arg, mir.Held) and arg.width == 10)]
            for arg, register in zip(integers, _RETURNED):
                if not isinstance(arg, mir.Held):
                    raise Unlowered(f"{op.at:#06x}: return operand needs materialization")
                returned.append((ir.Held(arg.value.id, arg.width), target.named(register, arg.width)))
            return tuple(returned)
        if op.kind is mir.Kind.RETURN and op.node is not None:
            returned = []
            for arg, location in zip(op.args, op.node.semantics.sources):
                if isinstance(location, ir.Reg):
                    if not isinstance(arg, mir.Held):
                        raise Unlowered(f"{op.at:#06x}: return operand needs materialization")
                    returned.append((ir.Held(arg.value.id, arg.width), location.register))
            return tuple(returned)
        if op.kind is mir.Kind.OPAQUE and not isinstance(op.node, ir.Restore):
            return self._implicit_values(op, op.uses, inputs=True)
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
            return self._unencoded(op)
        if _indirect_call(op):
            # An indirect call names its target as an ordinary encoded source.
            # The stack arguments are separate ARG operations, so there are no
            # hidden register inputs for this list to constrain.
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

    def _implicit_values(self, op: mir.Op, values: tuple[mir.Value, ...], *, inputs: bool = False) -> tuple:
        """Unencoded operands of an opaque instruction still have machine locations."""
        if not isinstance(op.node, ir.Opaque):
            return ()
        registers = {
            ir.ROOT.get(one.register, one.register): one.register
            for one in InstructionInfoFactory().info(op.node.insn.insn).used_registers()
        }
        if inputs:
            # SSA holds the unshifted word; copying its low byte to AH is not an extraction.
            return self._positional(
                op,
                {
                    root: target.named(root, 2)
                    if register in (Register.AH, Register.BH, Register.CH, Register.DH)
                    else register
                    for root, register in registers.items()
                },
            )
        return tuple(
            (ir.Held(value.id, RegisterExt.size(register)), register)
            for value in values
            if not value.flags
            if (register := registers.get(self._origin.get(value))) is not None
        )

    def _unencoded(self, op: mir.Op) -> tuple:
        """An operand the raise left opaque is emitted in BC's registers, so what it reads must be there."""
        registers = {
            ir.ROOT.get(where, where): where
            for arg in op.args
            if isinstance(arg, mir.Opaque)
            for where in (
                getattr(arg.what, "through", None),
                getattr(arg.what, "index", None),
                getattr(arg.what, "index_through", None),
            )
            if isinstance(where, int) and where != Register.NONE
        }
        return self._positional(op, registers) if registers and op.node is not None else ()

    def _positional(self, op: mir.Op, registers: dict) -> tuple:
        # A use's register is its position, not its value's origin: the
        # raise listed them in variable order, and a pass that forwards a
        # copy into `out dx,al` hands it a value BC kept somewhere else.
        # By origin, UNWHITEFADE's third OUT pinned nothing to AL and
        # wrote 0x3C9's low byte as every blue.
        order = sorted(mir._touched(op.node)[1], key=lambda one: (one is not mir.FLAGS, one))
        if len(op.uses) < len(order):
            raise Unlowered(f"{op.at:#06x}: {len(op.uses)} uses for {len(order)} operand registers")
        return tuple(
            (ir.Held(value.id, RegisterExt.size(register)), register)
            for value, root in zip(op.uses, order)
            if not value.flags
            if (register := registers.get(root)) is not None
        )

    def __init__(
        self,
        body: "mir.MirBody",
        read: set,
        calls: dict,
        absorbed,
        contracts=None,
        coverage=None,
        cpu: str | targets.Profile = "386",
        *,
        pointer_model=None,
    ) -> None:
        self.cpu = targets.profile(cpu)
        self.pointer_model = pointer_model
        self._read = read
        # Which dword values are a word's sign extension, and which word.
        self._extended = {
            one.results[0].value.id: one.args[0]
            for block in body.blocks
            for one in block.ops
            if one.kind is mir.Kind.SIGN_EXTEND
            and len(one.args) == 1
            and len(one.results) == 1
            and isinstance(one.args[0], mir.Held)
            and one.args[0].width == 2
            and isinstance(one.results[0], mir.Held)
            and one.results[0].width == 4
        }
        # A value used once, as a divide's high half over the low half beside
        # it. Anything else reading it means `cwd` would be computing the sign
        # for something that did not ask for dx.
        readers: dict[int, int] = {}
        dividends: dict[int, int] = {}
        for block in body.blocks:
            for one in block.ops:
                for arg in one.args:
                    if isinstance(arg, mir.Held):
                        readers[arg.value.id] = readers.get(arg.value.id, 0) + 1
                if (
                    one.kind is mir.Kind.DIV
                    and len(one.args) == 3
                    and all(isinstance(arg, mir.Held) for arg in one.args[:2])
                ):
                    dividends[one.args[0].value.id] = one.args[1].value.id
        self._dividends = {high: low for high, low in dividends.items() if readers.get(high) == 1}
        from qbopt.frontend.raising_words import leaving

        self._exposed = {value.id for value in leaving(body)}
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
        from qbopt.backend import addressforms

        self._address_forms = addressforms.offsets(body)
        self._indexed, self._folded = addressforms.indexed(body, self._exposed)
        every = [
            one.id for block in body.blocks for op in block.ops for one in (*op.defines, *op.uses) if one.id is not None
        ]
        every += [phi.result.id for block in body.blocks for phi in block.phis]
        self._next = max(every, default=0) + 1

    def fresh(self) -> int:
        """A value id nothing in this body already uses."""
        self._next += 1
        return self._next - 1

    def sign_extended(self, value: int) -> "mir.Held | None":
        """The word this dword is the sign extension of, if it is one."""
        return self._extended.get(value)

    def divides(self, high: int, word: "mir.Held") -> bool:
        """Whether `high` is only ever a divide's high half over that word."""
        found = self._dividends.get(high)
        return found is not None and found == word.value.id

    def _caller_cleanup(self, op: "mir.Op") -> "tuple[lir.Insn, ...]":
        """`add sp` after a call whose contract leaves its arguments to the caller."""
        contract = self._contracts.get(op.at) if op.kind is mir.Kind.CALL and self._contracts else None
        count = getattr(contract, "caller_cleanup", 0)
        if not count:
            return ()
        sp = ir.Reg(Register.SP, 2)
        return (_follows(op, ir.Semantics(ir.Operation.BINARY, "add", (sp,), (sp, ir.Imm(count, 2)))),)

    def expand(self, op: "mir.Op", *, preserve_flags: bool = True) -> "tuple[lir.Insn, ...]":
        """Every instruction this operation becomes, the leader first."""
        from qbopt.model import lir
        from qbopt.backend import addressforms

        if any(one.id in self._folded for one in op.defines):
            # The address is its cells' base and index now; see addressforms.indexed.
            op = replace(
                op, kind=mir.Kind.NOTHING, name="", args=(), results=(), defines=(), uses=(), node=None, made=None
            )
        made = _EXPANDS.get(op.kind)
        parts = made(op, self) if made is not None else _pointer_access(op, self)
        parts = _flag_test(op, self) or parts
        if op.kind is mir.Kind.CONVERT and not preserve_flags:
            parts = _sign_word(op) or parts
        if op.kind is mir.Kind.MUL and not preserve_flags:
            parts = _scaled(op, self) or parts
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
            from qbopt.backend import addressforms

            what = addressforms.scaled(addressforms.selected(what, self._address_forms), self._indexed)
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
            requires = self._abi(op, what)
            delivers = self._idiom(op, bool(speaks))
            if speaks and op.kind is not mir.Kind.CALL:
                # A register operand MIR has no value for -- BH -- is emitted
                # as itself, and the value the operation defines through it
                # would reach its readers from nowhere: oimad's `xor bh,bh`
                # left `mov ax,bx` reading a spill slot nothing stored.
                given = {*made, *(held.value for held, _ in delivers)}
                lost = [one for one in op.defines if not one.flags and one.id in self._read and one.id not in given]
                if lost:
                    raise Unlowered(f"{op.at:#06x}: {what} defines {lost} through no operand")
            # A node-less call's float result and a return's float operand are
            # in st(0): said by an instruction beside it, which FloatAlloc reads.
            floats = (
                tuple(one.value.id for one in (*op.args, *op.results) if isinstance(one, mir.Held) and one.width == 10)
                if op.node is None and op.kind in (mir.Kind.CALL, mir.Kind.RETURN)
                else ()
            )
            before = tuple(
                _follows(op, ir.Semantics(ir.Operation.FLOAT_STORE, "", (), (ir.Held(one, 10),)))
                for one in floats
                if op.kind is mir.Kind.RETURN
            )
            after = tuple(
                _follows(op, ir.Semantics(ir.Operation.FLOAT_LOAD, "", (ir.Held(one, 10),), ()))
                if one in self._read
                else _follows(op, ir.Semantics(ir.Operation.FLOAT_STORE, "fstp", (ir.St(0),), (ir.St(0),)))
                for one in floats
                if op.kind is mir.Kind.CALL
            )
            inputs = read if speaks else tuple(one.id for one in op.uses if not one.flags and one.id not in floats)
            inputs = tuple(dict.fromkeys((*inputs, *(held.value for held, _ in requires))))
            return (
                *before,
                lir.Insn(
                    at=op.at,
                    covers=op.covers,
                    what=what,
                    defines=made
                    if speaks and op.kind is not mir.Kind.CALL
                    else tuple(
                        one.id for one in op.defines if not one.flags and one.id in self._read and one.id not in floats
                    ),
                    uses=inputs,
                    requires=requires,
                    clobbers=_clobbers(op, self._calls, self._contracts),
                    clobbers_high=_clobbered_high(op, self._calls, self._contracts),
                    spread=()
                    if op.inserted
                    else (op.covers, *op.extra_covers)
                    if op.extra_covers
                    else self._coverage.get(op.id, ()),
                    delivers=delivers,
                    widths=self._widths(op) if what is None else (),
                    op=op,
                    symbol=op.symbol,
                ),
                *self._caller_cleanup(op),
                *after,
            )
        # The leader keeps the operation's identity -- its address, the
        # bytes it stands for, the operation itself -- and nothing else.
        # What it reads and writes is its own first step's, the way every
        # instruction after it is its own: a leader claiming the whole
        # operation's operands would say the product is live from the load
        # that starts the run. The effect the operation had belongs to the
        # run, not to any one instruction in it.
        parts = tuple(addressforms.scaled(one, self._indexed) for one in parts)
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


def _flag_test(op: mir.Op, context: Lowering) -> tuple[ir.Semantics, ...] | None:
    """A dead AND destination needs flags, not a two-address temporary."""
    if (
        op.kind is not mir.Kind.AND
        or op.loads
        or op.stores
        or op.merges
        or op.barrier
        or len(op.results) != 1
        or len(op.args) != 2
        or not isinstance(op.results[0], mir.Held)
    ):
        return None
    result = op.results[0]
    if (
        result.value.id in context._read | context._exposed
        or result.width not in (2, 4)
        or any(not isinstance(arg, mir.Held) or arg.width != result.width for arg in op.args)
        or any(value != result.value and not value.flags for value in op.defines)
    ):
        return None
    return (ir.Semantics(ir.Operation.COMPARE, "test", (), tuple(map(operand, op.args))),)


def _scaled(op: mir.Op, context: Lowering) -> tuple[ir.Semantics, ...] | None:
    """Select a cheaper target-specific chain when multiply flags are unobserved."""
    if len(op.args) != 2 or len(op.results) != 1:
        return None
    source, scale = op.args
    if isinstance(source, mir.Const):
        source, scale = scale, source
    result = op.results[0]
    if not (
        isinstance(source, mir.Held)
        and isinstance(scale, mir.Const)
        and isinstance(result, mir.Held)
        and source.width == result.width
        and source.width in (2, 4)
        and scale.width == source.width
        and 1 < scale.n < 1 << (source.width * 8)
        and not op.loads
        and not op.stores
        and not op.merges
    ):
        return None
    chain = arithmetic.scale(scale.n, getattr(context, "cpu", "386"))
    if chain is None or any(count >= source.width * 8 for name, count in chain if name == "shl"):
        return None
    parts = []
    current = operand(source)
    for index, (name, count) in enumerate(chain):
        into = operand(result) if index == len(chain) - 1 else ir.Held(context.fresh(), source.width)
        other = ir.Imm(count, 1) if name == "shl" else operand(source)
        parts.append(ir.Semantics(ir.Operation.BINARY, name, (into,), (current, other)))
        current = into
    return tuple(parts)


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
    from qbopt.model import lir

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
    from qbopt.model import mir

    return _clobbers(op, {}) if op.kind is mir.Kind.DIVMOD else frozenset()


def _clobbers(op: "mir.Op", calls: dict[int, str], contracts: dict | None = None) -> "frozenset[Register_]":
    """Which registers this instruction destroys without naming them.

    For a call, from `runtime.py`'s own contract for the routine
    -- which is measured against the runtime's source, not assumed. An
    unestablished contract clobbers every register, and saying so is the
    safe direction: over-stating what a call destroys only keeps a value
    out of a register, while under-stating it puts a live value in one the
    call overwrites.
    """
    from qbopt.model import mir
    from qbopt.abi import runtime

    if isinstance(op.node, ir.Restore):
        # The source is unchanged by push-wide/pop-low. The second pop
        # overwrites the other register even when its result is dead.
        return frozenset({mir.RESTORE_PAIR[op.node.pair][1]})
    if op.kind is mir.Kind.DIVMOD:
        # An absorbed divide is emitted as a sequence, not as one
        # instruction, and it writes registers none of its operands name:
        # the dividend's, the divisor's, idiv's own edx, and wherever the
        # answer it was not asked for is kept. Declared here because this
        # is where a machine fact belongs, and because nothing else tells
        # the allocator -- a value living in ecx across the site was not
        # interfering with anything it could see.
        from qbopt.legacy import calls as machine

        return frozenset({machine.RESULT, machine.DIVISOR, Register.EDX, machine.OTHER} & set(target.AVAILABLE))
    if op.kind is not mir.Kind.CALL:
        return frozenset()
    contract = (contracts or {}).get(op.at) or runtime.contract(calls.get(op.at))
    if contract is None:
        return frozenset((*target.AVAILABLE, *target.SELECTORS))
    names = _names()
    disturbed = runtime.disturbs(contract)
    # A contract is about the 8086 and names no FS or GS; one reaching user
    # code, or written for the 386, runs code that may use them.
    unnamed = (
        frozenset(target.SELECTORS) - frozenset(names) if disturbed == runtime.EVERY or contract.i386 else frozenset()
    )
    return unnamed | frozenset(
        register for register in names for named in disturbed if named.value.lower() in names[register]
    )


def _clobbered_high(op: "mir.Op", calls: dict[int, str], contracts: dict | None = None) -> "frozenset[Register_]":
    """The registers a 386 callee keeps only the 16-bit half of."""
    if op.kind is not mir.Kind.CALL:
        return frozenset()
    contract = (contracts or {}).get(op.at) or runtime.contract(calls.get(op.at))
    if contract is None or not contract.i386:
        return frozenset()
    whole = {ir.ROOT.get(register, register) for register in _clobbers(op, calls, contracts)}
    return frozenset(register for register in target.AVAILABLE if ir.ROOT.get(register, register) not in whole)


# Each allocatable register by the names runtime.py's own Reg enum uses:
# `ax` for eax, since a contract is written about the 16-bit machine.
def _names() -> dict:
    return {
        register: {target.name_of(target.named(register, 2)), target.name_of(target.named(register, 4))}
        for register in (*target.AVAILABLE, Register.ES)
    }
