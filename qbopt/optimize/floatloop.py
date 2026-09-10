"""Specialize exact finite recurrences to one checked final iteration.

The first invariant floating load stays first, with the original memory and
counter state. It still checks pending exceptions. After it, the proven
pre-final memory state replaces the intermediate iterations. All remaining
floating operations execute in their original order with their final inputs.
No reassociation, conversion deletion or assumption about rounding is needed.
"""

from dataclasses import replace

from qbopt.analysis import consts, floatfacts, induction, loops, ssa
from qbopt.model import mir
from qbopt.optimize import strength, transform


def specialized(body: mir.MirBody, dgroup: frozenset[int], calls: dict) -> mir.MirBody:
    proofs = {proof.header: proof for proof in floatfacts.loop_exits(body, dgroup, calls) if proof.count > 1}
    if not proofs:
        return body
    blocks = {block.at: block for block in body.blocks}
    predecessors = loops.predecessors(body.blocks)
    facts = consts.known(body, dgroup, calls)
    memory = floatfacts.cells(body, dgroup, calls)
    for loop in loops.loops(body.blocks, body.entry):
        if (proof := proofs.get(loop.header)) is None:
            continue
        header = blocks[loop.header]
        latch = blocks[next(iter(loop.latches))]
        exit_at, = [at for at in header.succ if at not in loop.body]
        if set(predecessors[exit_at]) != {header.at} or blocks[exit_at].phis:
            continue
        counters = induction.basics(body, loop)
        live = transform.live(body)
        phis = [phi for phi in header.phis if phi.result in live]
        if len(phis) != 1 or phis[0].result.id not in counters:
            continue
        phi = phis[0]
        counter = counters[phi.result.id]
        width = counter.start.width
        start = induction._signed(counter.start, facts, width)
        step = induction._signed(counter.step, facts, width)
        if start is None or step is None:
            continue
        active = [op for op in latch.ops if op.kind is not mir.Kind.NOTHING]
        if not active or active[0].kind is not mir.Kind.FLOAD or active[0].floating is None:
            continue
        checkpoint = active[0]
        if any(mir.overlapping(read, written, dgroup)
               for read in checkpoint.loads for written, _ in proof.stores):
            continue
        update = phi.incoming[latch.at]
        if any(op.floating is None and not floatfacts.checkpoint(op)
               and (update not in op.defines or op.loads or op.stores)
               for op in active[1:]):
            continue
        if any(phi.result in op.uses for op in active if op.floating is not None):
            continue
        if any(op.stores and op.args != (mir.Held(phi.result, width),) for op in header.ops):
            continue
        internal = {value for block in (header, latch) for op in block.ops for value in op.defines}
        if (internal | {phi.result}) & transform._leaving(body):
            continue
        outside = [block for block in body.blocks if block.at not in loop.body]
        if any(value in internal for block in outside for op in block.ops for value in op.uses):
            continue
        if any(value in internal or value == phi.result for block in outside
               for join in block.phis for value in join.incoming.values()):
            continue
        entry_at, = [at for at in predecessors[header.at] if at not in loop.body]
        entry = blocks[entry_at]
        initial = consts._kills(memory[entry_at, len(entry.ops) - 1], entry.ops[-1], facts, dgroup, calls)
        before_last = floatfacts.repeated(latch.ops, proof.count - 1, initial, dgroup, facts)
        if before_last is None:
            continue
        seeds = []
        for ref, _ in proof.stores:
            if not _carried(ref, latch.ops, dgroup):
                continue
            fact = consts._cell(before_last, ref)
            if fact is None:
                break
            owner = next(op for op in latch.ops if ref in op.stores)
            seed = _store(owner, ref, mir.Const(fact.n, fact.width))
            seeds.append(replace(seed, at=checkpoint.at, covers=(checkpoint.at, checkpoint.at)))
        else:
            final = mir.Const(consts.masked(start + step * proof.count, width), width)
            return _rewritten(body, header, latch, exit_at, checkpoint, seeds, phi.result, final)
    return body


def _carried(ref, ops, dgroup):
    for op in ops:
        if any(mir.overlapping(ref, read, dgroup) for read in op.loads):
            return True
        if any(mir.same_bytes(ref, written) for written in op.stores):
            return False
    return False


def _store(beside, ref, value):
    return mir.Op(at=beside.at, op=beside.op, name="", defines=(), uses=(),
                  loads=(), stores=(ref,), kind=mir.Kind.STORE, args=(value,), results=(mir.Cell(ref),),
                  covers=(beside.at, beside.at), id=beside.id, symbol=True)


def _jump(beside, destination):
    return replace(beside, kind=mir.Kind.JUMP, name="", args=(), results=(),
                   uses=(), defines=(), loads=(), stores=(), merges={}, node=None,
                   made=None, raised=((), ()), target=destination, test=None)


def _rewritten(body, header, latch, exit_at, checkpoint, seeds, counter, final):
    exit_block = next(block for block in body.blocks if block.at == exit_at)
    values = tuple(ssa.values(body))
    result = mir.Value(max(value.id for value in values) + 1, exit_at,
                       variable=max(value.variable for value in values) + 1)
    copy = strength._made(mir.Kind.COPY, "", result, (final,), exit_at, exit_block.ops[0])
    final_stores = tuple(replace(_store(op, ref, final), at=exit_at, covers=(exit_at, exit_at))
                         for op in header.ops for ref in op.stores)
    dominators = loops.dominators(body.blocks, body.entry)
    following = {block.at for block in body.blocks if exit_at in dominators.get(block.at, ())}
    out = []
    for block in body.blocks:
        if block.at == header.at:
            block = replace(block, succ=(latch.at,), ops=(*block.ops[:-1], _jump(block.ops[-1], latch.at)))
        elif block.at == latch.at:
            ops = []
            for op in block.ops:
                ops.append(op)
                if op is checkpoint:
                    ops.extend(seeds)
            end = block.ops[-1].at
            jump = _jump(header.ops[-1], exit_at)
            block = replace(block, succ=(exit_at,), ops=(*ops, replace(jump, at=end, covers=(end, end))))
        elif block.at in following:
            ops = tuple(ssa.substituted(op, {counter.id: result}) for op in block.ops)
            block = replace(block, ops=(copy, *final_stores, *ops) if block.at == exit_at else ops)
        out.append(block)
    return transform._trivial_phis(replace(body, blocks=tuple(out)))
