"""A counted loop that stores one value into consecutive cells is one fill.

LLVM's `LoopIdiomRecognize`, which makes such a loop a `memset`. The loop is
tested before its body, as the raise lays it out, and the test stays as the
guard: the body runs once, fills from the cell the first pass would have
stored, and leaves for the exit.

The shape is narrow on purpose: a header holding only its exit test, and a
body holding only the store and the counters' steps. The counter the test
reads steps by one, so the count is the bound less the counter, and one more
where the bound itself runs; the cell's index steps by the cell's width. No
value the loop computes may be read after it -- `loopexit` has rewritten the
ones it could by then.
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
    if not isinstance(store.results[0], mir.Cell) or store.stores != (store.results[0].ref,):
        return None
    ref, value = store.results[0].ref, store.args[0]
    if (
        ref.width not in (1, 2, 4)
        or ref.addr is None
        or ref.base is None
        or ref.segment is not None
        or ref.pointer
        or ref.symbolic is not None
        or ref.allocation is not None
        or ref.base_width != 2
        or ref.addr.space is Space.FRAME
    ):
        return None
    index = counters.get(ref.base.id)
    if index is None or not _stepping(index, ref.width):
        return None
    if not isinstance(value, (mir.Const, mir.Held)) or value.width != ref.width:
        return None
    if isinstance(value, mir.Held) and value.value in defined:
        return None
    # Nothing after the loop may read what it computed.
    for block in body.blocks:
        if block.at in inside:
            continue
        if any(value in defined for op in block.ops for value in op.uses):
            return None
        if any(value in defined for phi in block.phis for value in phi.incoming.values()):
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
    first = mir.Symbol(ref.addr.space, ref.addr.index, ref.addr.disp, 2)
    address = emit(mir.Kind.ADD, ir.Operation.BINARY, (mir.Held(ref.base, 2), first), 2)
    args = (value, count, address)
    fill = replace(
        store,
        op=ir.Operation.FILL,
        name=mir.Kind.FILL.value,
        kind=mir.Kind.FILL,
        args=args,
        results=(),
        defines=(),
        uses=tuple(arg.value for arg in args if isinstance(arg, mir.Held)),
        # Cells nothing names one by one: an effect that reaches anything.
        stores=(mir.MemRef(None, ref.width),),
        node=None,
        made=None,
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
            phis = tuple(replace(phi, incoming={**phi.incoming, latch.at: phi.incoming[header.at]}) for phi in block.phis)
            block = replace(block, phis=phis)
        blocks.append(block)
    return replace(body, blocks=tuple(blocks))


def _stepping(counter: induction.Affine, by: int) -> bool:
    return isinstance(counter.step, mir.Const) and counter.step.n == by


def _steps(header, latch, ops: list[mir.Op]) -> bool:
    """Whether `ops` are each header counter's own step and nothing else."""
    if len(ops) != len(header.phis):
        return False
    stepped = set()
    for op in ops:
        if op.kind is not mir.Kind.ADD or op.loads or op.stores or op.barrier or len(op.args) != 2 or len(op.results) != 1:
            return False
        source, step = op.args
        if not isinstance(source, mir.Held) or not isinstance(step, mir.Const) or not isinstance(op.results[0], mir.Held):
            return False
        phi = next((phi for phi in header.phis if phi.result == source.value), None)
        if phi is None or phi.incoming.get(latch.at) != op.results[0].value:
            return False
        stepped.add(phi.result)
    return len(stepped) == len(header.phis)


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
