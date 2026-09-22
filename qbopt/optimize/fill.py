"""A counted loop that stores one value into consecutive cells is one fill.

LLVM's `LoopIdiomRecognize`, which makes such a loop a `memset`. The loop is
tested before its body, as the raise lays it out, and the test stays as the
guard: the body runs once, fills from the cell the first pass would have
stored, and leaves for the exit.

The shape is narrow on purpose: a header holding only its exit test, and a
body holding the store, the counters' steps and work with no effect. The trip
count is `induction`'s proof; the cell is reached through a counter stepping
by the cell's width, or such a counter plus something the loop does not
change. A far cell's selector goes with it. No value the loop
computes may be read after it but a counter the exit takes from the header,
which leaves the body as its start plus the count of its steps.
"""

from dataclasses import replace

from qbopt.model import ir
from qbopt.model import mir
from qbopt.analysis import ssa
from qbopt.analysis import loops as loopy
from qbopt.analysis import induction
from qbopt.objectfile.module import Space
from qbopt.model.passes import MIRTransform


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
    header = at_of.get(loop.header)
    if header is None or len(loop.latches) != 1 or len(header.succ) != 2:
        return None
    # The body is one straight line back to the header, in whatever order its blocks lie.
    chain = _chain(at_of, header, inside)
    if chain is None:
        return None
    latch = chain[-1]
    exit_at = next(to for to in header.succ if to not in inside)
    if at_of.get(exit_at) is None:
        return None

    # How many trips is `induction`'s to prove, whatever the counter's step or test.
    tested = _work(header.ops)
    proofs = [
        proof
        for proof in induction.counted(body, loop)
        if not proof.posttested and tested[-2:] == [proof.compare, proof.branch]
    ]
    if not proofs or not all(_pure(op) for op in tested[:-2]):
        return None
    proof = proofs[0]
    counters = induction.basics(body, loop)
    if len(counters) != len(header.phis):
        return None

    defined = {phi.result for at in inside for phi in at_of[at].phis}
    defined |= {value for at in inside for op in at_of[at].ops for value in op.defines}
    work = [op for block in chain for op in _work(block.ops) if op.kind is not mir.Kind.JUMP]
    work.append(latch.ops[-1])
    if work[-1].kind is not mir.Kind.JUMP or work[-1].target != header.at:
        return None
    effects = [op for op in work[:-1] if op.kind in (mir.Kind.STORE, mir.Kind.FILL)]
    if len(effects) != 1 or not _steps(header, latch, [op for op in work[:-1] if op is not effects[0]]):
        return None
    effect = effects[0]
    made = {value.id: op for op in (*tested, *work) for value in op.defines}
    found = _stored(effect, defined) if effect.kind is mir.Kind.STORE else _fill(effect, defined)
    if found is None:
        return None
    value, cells, base, space, provenance, ref = found
    index = counters.get(base.value.id) or _offset(made.get(base.value.id), counters, defined, made)
    if index is None or not _stepping(index, cells * value.width):
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
    if any(
        not isinstance(counters[one.id].step, mir.Const) or counters[one.id].start.width != proof.width for one in left
    ):
        return None

    fresh = _Fresh(body)
    at = effect.at
    prefix: list[mir.Op] = []

    def emit(kind: mir.Kind, operation: ir.Operation, args: tuple, width: int, name: str = "") -> mir.Held:
        result = fresh.held(at, width)
        prefix.append(mir.Op(
            at, operation, name or kind.value, (result.value,),
            tuple(arg.value for arg in args if isinstance(arg, mir.Held)),
            kind=kind, args=args, results=(result,),
        ))  # fmt: skip
        return result

    trips = induction.trips(proof, lambda kind, args: emit(kind, ir.Operation.BINARY, args, args[0].width))
    if trips is None:
        return None
    width = trips.width
    count = trips if cells == 1 else emit(mir.Kind.MUL, ir.Operation.BINARY, (trips, mir.Const(cells, width)), width)
    finals = {}
    for one in left:
        step = counters[one.id].step
        moved = (
            trips if step.n == 1 else emit(mir.Kind.MUL, ir.Operation.BINARY, (trips, mir.Const(step.n, width)), width)
        )
        finals[one] = emit(mir.Kind.ADD, ir.Operation.BINARY, (mir.Held(one, width), moved), width).value
    if ref is None:
        # A fill's address is already its first cell's, as the first trip computes it.
        address, segment = base, effect.args[3:]
    else:
        if ref.addr.space in (Space.FAR, Space.LITERAL):
            first = mir.Const(ref.addr.disp, 2)
        elif ref.addr.space is Space.FRAME:
            # The frame is reached through bp, which no immediate names.
            cell = (mir.Cell(replace(ref, base=None, width=2)),)
            first = emit(mir.Kind.ADDRESS, ir.Operation.ADDRESS, cell, 2, "lea")
        else:
            first = mir.Symbol(ref.addr.space, ref.addr.index, ref.addr.disp, 2)
        address = emit(mir.Kind.ADD, ir.Operation.BINARY, (mir.Held(ref.base, 2), first), 2)
        segment = () if ref.segment is None else (mir.Held(ref.segment, 2),)
    args = (value, count, address, *segment)
    fill = replace(
        effect,
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
        stores=(mir.MemRef(None, value.width, space=space, provenance=provenance),),
        source_backed=False,
        raised=None,
    )
    lines = {}
    for block in chain:
        ops = []
        for op in block.ops:
            if op is effect:
                ops += [*prefix, fill]
            elif op is work[-1]:
                ops.append(replace(op, target=exit_at))
            else:
                ops.append(op)
        lines[block.at] = ops
    # A proven positive count means the header's test passes on entry: it guards nothing.
    entered = bool(proof.count)
    blocks = []
    for block in body.blocks:
        if block.at == latch.at:
            block = replace(block, ops=tuple(lines[block.at]), succ=(exit_at,))
        elif block.at in lines:
            block = replace(block, ops=tuple(lines[block.at]))
        elif block.at == header.at:
            phis = tuple(
                replace(phi, incoming={where: one for where, one in phi.incoming.items() if where != latch.at})
                for phi in block.phis
            )
            block = replace(block, phis=phis)
            if entered:
                first = chain[0].at
                block = replace(block, ops=(*block.ops[:-1], mir.jump(block.ops[-1], first)), succ=(first,))
        elif block.at == exit_at:
            phis = tuple(
                replace(
                    phi,
                    incoming={
                        **{where: one for where, one in phi.incoming.items() if not (entered and where == header.at)},
                        latch.at: finals.get(phi.incoming[header.at], phi.incoming[header.at]),
                    },
                )
                for phi in block.phis
            )
            block = replace(block, phis=phis)
        blocks.append(block)
    swap = {
        phi.result.id: next(iter(phi.incoming.values()))
        for phi in next(block for block in blocks if block.at == header.at).phis
        if len(phi.incoming) == 1
    }
    if swap:
        blocks = [
            replace(
                block,
                phis=tuple(
                    replace(phi, incoming={where: ssa.provider(one, swap) for where, one in phi.incoming.items()})
                    for phi in block.phis
                    if phi.result.id not in swap
                ),
                ops=tuple(ssa.substituted(op, swap) for op in block.ops),
            )
            for block in blocks
        ]
    return replace(body, blocks=tuple(blocks))


def _stepping(counter: induction.Affine, by: int) -> bool:
    return isinstance(counter.step, mir.Const) and counter.step.n == by


def _chain(at_of: dict[int, mir.MirBlock], header: mir.MirBlock, inside: set[int]) -> list[mir.MirBlock] | None:
    """The loop's blocks after its header, when each has one way in and one out and the last goes back."""
    chain: list[mir.MirBlock] = []
    at = next((to for to in header.succ if to in inside), None)
    while at is not None and at != header.at:
        block = at_of.get(at)
        if block is None or block in chain or len(block.succ) != 1 or block.phis:
            return None
        chain.append(block)
        at = block.succ[0]
    return chain if chain and len(chain) + 1 == len(inside) else None


def _stored(store: mir.Op, defined: set) -> tuple | None:
    """A one-cell store's value, cells, base, storage class, provenance and cell."""
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
        or not _unchanged(value, defined)
        or value.width != ref.width
    ):
        return None
    return value, 1, mir.Held(ref.base, 2), ref.space, ref.provenance, ref


def _fill(fill: mir.Op, defined: set) -> tuple | None:
    """The same of a fill of a constant number of cells: a loop of them is one fill."""
    value, count, address, *segment = fill.args
    if (
        fill.loads
        or fill.barrier
        or len(fill.stores) != 1
        or not isinstance(count, mir.Const)
        or not isinstance(address, mir.Held)
        or not _unchanged(value, defined)
        or not all(_unchanged(one, defined) for one in segment)
    ):
        return None
    return value, count.n, address, fill.stores[0].space, fill.stores[0].provenance, None


def _unchanged(arg: mir.Arg, defined: set) -> bool:
    return isinstance(arg, mir.Const) or isinstance(arg, mir.Held) and arg.value not in defined


def _offset(
    op: mir.Op | None, counters: dict[int, induction.Affine], defined: set, made: dict[int, mir.Op]
) -> induction.Affine | None:
    """The counter `op` adds things the loop does not change to, stepping as it does."""
    if op is None or op.kind is not mir.Kind.ADD or op.loads or op.stores or op.barrier or len(op.args) != 2:
        return None
    for one, other in (op.args, op.args[::-1]):
        if not isinstance(one, mir.Held) or not (isinstance(other, mir.Symbol) or _unchanged(other, defined)):
            continue
        found = counters.get(one.value.id) or _offset(made.get(one.value.id), counters, defined, made)
        if found is not None:
            return found
    return None


def _pure(op: mir.Op) -> bool:
    """Work that stores nothing, reads nothing and cannot trap."""
    return not (
        op.loads
        or op.stores
        or op.barrier
        or op.floating is not None
        or op.kind in (mir.Kind.CALL, mir.Kind.ESCAPE, mir.Kind.OPAQUE, mir.Kind.DIVMOD, mir.Kind.UDIVMOD)
        or not all(isinstance(result, mir.Held) for result in op.results)
    )


def _steps(header, latch, ops: list[mir.Op]) -> bool:
    """Whether each header counter steps once in `ops`, and the rest compute without effect.

    Done once instead of every time round, work that stores nothing, reads
    nothing and cannot trap leaves only values nothing after the loop reads."""
    stepped = set()
    for op in ops:
        phi = _stepped(header, latch, op)
        if phi is not None and phi.result not in stepped:
            stepped.add(phi.result)
        elif not _pure(op):
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
        values |= {
            value for block in body.blocks for phi in block.phis for value in (phi.result, *phi.incoming.values())
        }
        self.serial = max((value.id for value in values), default=0)
        self.variable = max((value.variable for value in values), default=0)

    def held(self, at: int, width: int) -> mir.Held:
        self.serial += 1
        self.variable += 1
        return mir.Held(mir.Value(self.serial, at, variable=self.variable, version=1), width)
