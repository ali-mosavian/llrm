"""A counted loop that stores one value into consecutive cells is one fill.

LLVM's `LoopIdiomRecognize`, which makes such a loop a `memset`. The loop is
tested before its body, as the raise lays it out, and the test stays as the
guard: the body runs once, fills from the cell the first pass would have
stored, and leaves for the exit.

The shape is narrow on purpose: a header holding only its exit test, and a
body holding the store, the counters' steps and work with no effect. The
counter the test reads steps by one, so the count is the bound less the
counter, and one more where the bound itself runs; the cell is reached through
a counter stepping by the cell's width, or such a counter plus something the
loop does not change. A far cell's selector goes with it. No value the loop
computes may be read after it but a counter the exit takes from the header,
which leaves the body as its start plus the count of its steps.
"""

from dataclasses import replace

from qbopt.model import ir
from qbopt.model import mir
from qbopt.analysis import loops as loopy
from qbopt.analysis import induction
from qbopt.objectfile.module import Space
from qbopt.model.passes import MIRTransform

# The test that keeps the loop going, by whether the bound itself runs.
_INCLUSIVE = {mir.Kind.LT: False, mir.Kind.BELOW: False, mir.Kind.LE: True, mir.Kind.BELOW_EQ: True}
_INVERSE = {
    mir.Kind.GE: mir.Kind.LT,
    mir.Kind.ABOVE_EQ: mir.Kind.BELOW,
    mir.Kind.GT: mir.Kind.LE,
    mir.Kind.ABOVE: mir.Kind.BELOW_EQ,
}


class Fill(MIRTransform):
    name = "fill"

    def transform(self, body: mir.MirBody) -> mir.MirBody:
        return filled(body)


def filled(body: mir.MirBody) -> mir.MirBody:
    """`body` with every such loop's body made one fill."""
    for loop in loopy.loops(body.blocks, body.entry):
        made = _filled(body, loop)
        if made is not None:
            return filled(made)
    return body


def _work(ops) -> list[mir.Op]:
    return [op for op in ops if op.kind is not mir.Kind.NOTHING]


def _filled(body: mir.MirBody, loop) -> mir.MirBody | None:
    at_of = {block.at: block for block in body.blocks}
    inside = set(loop.body)
    if len(inside) != 2 or len(loop.latches) != 1:
        return None
    header, latch = at_of.get(loop.header), at_of.get(next(iter(loop.latches)))
    if header is None or latch is None or latch.at == header.at or latch.succ != (header.at,):
        return None
    if len(header.succ) != 2 or latch.at not in header.succ:
        return None
    exit_at = next(to for to in header.succ if to != latch.at)
    leaving = at_of.get(exit_at)
    if leaving is None or exit_at in inside:
        return None

    tested = _work(header.ops)
    if len(tested) != 2:
        return None
    compare, branch = tested
    if branch.kind is not mir.Kind.BRANCH or branch.target not in header.succ:
        return None
    test = branch.test if branch.target == latch.at else _INVERSE.get(branch.test)
    if test not in _INCLUSIVE:
        return None
    flags = [value for value in compare.defines if value.flags]
    if (
        compare.kind is not mir.Kind.SUB
        or compare.results
        or len(compare.args) != 2
        or compare.loads
        or compare.stores
        or compare.barrier
        or not flags
        or not set(flags) & set(branch.uses)
    ):
        return None
    counter, bound = compare.args
    counters = induction.basics(body, loop)
    if len(counters) != len(header.phis) or not isinstance(counter, mir.Held) or counter.value.id not in counters:
        return None
    if not _stepping(counters[counter.value.id], 1):
        return None

    defined = {phi.result for at in inside for phi in at_of[at].phis}
    defined |= {value for at in inside for op in at_of[at].ops for value in op.defines}
    if not (isinstance(bound, mir.Const) or isinstance(bound, mir.Held) and bound.value not in defined):
        return None
    if bound.width != counter.width:
        return None

    work = _work(latch.ops)
    if not work or work[-1].kind is not mir.Kind.JUMP or work[-1].target != header.at:
        return None
    stores = [op for op in work[:-1] if op.kind is mir.Kind.STORE]
    if len(stores) != 1 or not _steps(header, latch, [op for op in work[:-1] if op is not stores[0]]):
        return None
    store = stores[0]
    if store.loads or store.barrier or len(store.args) != 1 or len(store.results) != 1:
        return None
    # The effect may carry what it is known to miss; the cell is the same.
    if (
        not isinstance(store.results[0], mir.Cell)
        or len(store.stores) != 1
        or replace(store.stores[0], excludes=()) != replace(store.results[0].ref, excludes=())
    ):
        return None
    ref, value = store.results[0].ref, store.args[0]
    if (
        ref.width not in (1, 2, 4)
        or ref.addr is None
        or ref.base is None
        or (ref.segment is not None) != (ref.addr.space is Space.FAR)
        or ref.segment in defined
        or ref.pointer
        or ref.symbolic is not None
        or ref.allocation is not None
        or ref.base_width != 2
        or ref.addr.space is Space.FRAME
    ):
        return None
    made = {value.id: op for op in work for value in op.defines}
    index = counters.get(ref.base.id) or _offset(made.get(ref.base.id), counters, defined)
    if index is None or not _stepping(index, ref.width):
        return None
    if not isinstance(value, (mir.Const, mir.Held)) or value.width != ref.width:
        return None
    if isinstance(value, mir.Held) and value.value in defined:
        return None
    # Nothing after the loop may read what it computed, but a counter the exit's phis take from the header.
    left = set()
    for block in body.blocks:
        if block.at in inside:
            continue
        if any(value in defined for op in block.ops for value in op.uses):
            return None
        for phi in block.phis:
            for where, one in phi.incoming.items():
                if one not in defined:
                    continue
                if block.at != exit_at or where != header.at or one.id not in counters:
                    return None
                left.add(one)
    if any(not isinstance(counters[one.id].step, mir.Const) or counters[one.id].start.width != counter.width for one in left):
        return None

    fresh = _Fresh(body)
    at = store.at
    prefix: list[mir.Op] = []

    def emit(kind: mir.Kind, operation: ir.Operation, args: tuple, width: int) -> mir.Held:
        result = fresh.held(at, width)
        prefix.append(mir.Op(
            at, operation, kind.value, (result.value,),
            tuple(arg.value for arg in args if isinstance(arg, mir.Held)),
            kind=kind, args=args, results=(result,), covers=(at, at),
        ))  # fmt: skip
        return result

    width = counter.width
    inclusive = _INCLUSIVE[test]
    if isinstance(bound, mir.Const):
        negated = emit(mir.Kind.NEG, ir.Operation.UNARY, (counter,), width)
        count = emit(mir.Kind.ADD, ir.Operation.BINARY, (negated, mir.Const(bound.n + inclusive, width)), width)
    else:
        count = emit(mir.Kind.SUB, ir.Operation.BINARY, (bound, counter), width)
        if inclusive:
            count = emit(mir.Kind.ADD, ir.Operation.BINARY, (count, mir.Const(1, width)), width)
    finals = {}
    for one in left:
        step = counters[one.id].step
        moved = count if step.n == 1 else emit(mir.Kind.MUL, ir.Operation.BINARY, (count, mir.Const(step.n, width)), width)
        finals[one] = emit(mir.Kind.ADD, ir.Operation.BINARY, (mir.Held(one, width), moved), width).value
    if ref.addr.space in (Space.FAR, Space.LITERAL):
        first = mir.Const(ref.addr.disp, 2)
    else:
        first = mir.Symbol(ref.addr.space, ref.addr.index, ref.addr.disp, 2)
    address = emit(mir.Kind.ADD, ir.Operation.BINARY, (mir.Held(ref.base, 2), first), 2)
    args = (value, count, address) + (() if ref.segment is None else (mir.Held(ref.segment, 2),))
    fill = replace(
        store,
        op=ir.Operation.FILL,
        name=mir.Kind.FILL.value,
        kind=mir.Kind.FILL,
        args=args,
        results=(),
        defines=(),
        uses=tuple(arg.value for arg in args if isinstance(arg, mir.Held)),
        # The exact cells are no longer named one by one, but the storage
        # class remains semantic.  Lowering needs it to select SS rather
        # than DS for a fill reached through a frame-derived near pointer.
        stores=(mir.MemRef(None, ref.width, space=ref.space, provenance=ref.provenance),),
        node=None,
        raised=None,
    )
    ops = []
    for op in latch.ops:
        if op is store:
            ops += [*prefix, fill]
        elif op is work[-1]:
            ops.append(replace(op, target=exit_at))
        else:
            ops.append(op)
    blocks = []
    for block in body.blocks:
        if block.at == latch.at:
            block = replace(block, ops=tuple(ops), succ=(exit_at,))
        elif block.at == header.at:
            phis = tuple(
                replace(phi, incoming={where: one for where, one in phi.incoming.items() if where != latch.at})
                for phi in block.phis
            )
            block = replace(block, phis=phis)
        elif block.at == exit_at:
            phis = tuple(
                replace(phi, incoming={**phi.incoming, latch.at: finals.get(phi.incoming[header.at], phi.incoming[header.at])})
                for phi in block.phis
            )
            block = replace(block, phis=phis)
        blocks.append(block)
    return replace(body, blocks=tuple(blocks))


def _stepping(counter: induction.Affine, by: int) -> bool:
    return isinstance(counter.step, mir.Const) and counter.step.n == by


def _offset(op: mir.Op | None, counters: dict[int, induction.Affine], defined: set) -> induction.Affine | None:
    """The counter `op` adds something the loop does not change to, stepping as it does."""
    if op is None or op.kind is not mir.Kind.ADD or op.loads or op.stores or op.barrier or len(op.args) != 2:
        return None
    for one, other in (op.args, op.args[::-1]):
        unchanged = isinstance(other, (mir.Const, mir.Symbol)) or isinstance(other, mir.Held) and other.value not in defined
        if isinstance(one, mir.Held) and one.value.id in counters and unchanged:
            return counters[one.value.id]
    return None


def _steps(header, latch, ops: list[mir.Op]) -> bool:
    """Whether each header counter steps once in `ops`, and the rest compute without effect.

    Done once instead of every time round, work that stores nothing, reads
    nothing and cannot trap leaves only values nothing after the loop reads."""
    stepped = set()
    for op in ops:
        phi = _stepped(header, latch, op)
        if phi is not None and phi.result not in stepped:
            stepped.add(phi.result)
        elif (
            op.loads
            or op.stores
            or op.barrier
            or op.floating is not None
            or op.kind in (mir.Kind.CALL, mir.Kind.ESCAPE, mir.Kind.OPAQUE, mir.Kind.DIVMOD, mir.Kind.UDIVMOD)
            or not all(isinstance(result, mir.Held) for result in op.results)
        ):
            return False
    return len(stepped) == len(header.phis)


def _stepped(header, latch, op: mir.Op):
    """The header phi `op` steps by a constant, if it is one."""
    if op.kind is not mir.Kind.ADD or op.loads or op.stores or op.barrier or len(op.args) != 2 or len(op.results) != 1:
        return None
    source, step = op.args
    if not isinstance(source, mir.Held) or not isinstance(step, mir.Const) or not isinstance(op.results[0], mir.Held):
        return None
    phi = next((phi for phi in header.phis if phi.result == source.value), None)
    if phi is None or phi.incoming.get(latch.at) != op.results[0].value:
        return None
    return phi


class _Fresh:
    """Values no operation in the body names yet."""

    def __init__(self, body: mir.MirBody) -> None:
        values = {value for block in body.blocks for op in block.ops for value in (*op.defines, *op.uses)}
        values |= {value for block in body.blocks for phi in block.phis for value in (phi.result, *phi.incoming.values())}
        self.serial = max((value.id for value in values), default=0)
        self.variable = max((value.variable for value in values), default=0)

    def held(self, at: int, width: int) -> mir.Held:
        self.serial += 1
        self.variable += 1
        return mir.Held(mir.Value(self.serial, at, variable=self.variable, version=1), width)
