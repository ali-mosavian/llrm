"""Unknown branches must not erase independently proven constant array extents."""

from dataclasses import replace
from pathlib import Path

import pytest

from qbopt.frontend import arrayfacts
from qbopt.model import ir, mir
from qbopt.objectfile.module import Addr, Space


@pytest.mark.parametrize("case", ["bounded", "unbounded", "overlap", "clobber"])
def test_guarded_record_stores_only_exclude_proven_disjoint_statics(monkeypatch, case):
    """UDTRNG recomputed SLOT*8 and reloaded both steps on every record update."""
    from qbopt.frontend import blocks
    from qbopt.objectfile import module, omf
    with monkeypatch.context() as context:
        context.setattr(arrayfacts, "proven", lambda body: body)
        found = module.of(omf.parse(Path("fixtures/regressions/udtrng-p-g2.obj").read_bytes()))
        body = mir.bodies(found, blocks.partition(found, blocks.code_map(found)))[0][1]
    if case in ("unbounded", "overlap"):
        updated = []
        for block in body.blocks:
            ops = tuple(replace(op, args=(op.args[0], mir.Const(10, 2)))
                        if case == "overlap" and op.at == 0x60 and op.op is ir.Operation.COMPARE else op
                        for op in block.ops)
            if case == "unbounded" and block.at == 0x60:
                ops = tuple(replace(op, kind=mir.Kind.NOTHING) if op.kind is mir.Kind.BRANCH else op for op in ops)
            updated.append(replace(block, ops=ops))
        body = replace(body, blocks=tuple(updated))
    if case == "clobber":
        block = body.block(0x9a)
        barrier = mir.Op(0x9a, ir.Operation.CALL, "", (), (), kind=mir.Kind.CALL)
        body = replace(body, blocks=tuple(replace(item, ops=(barrier, *item.ops))
                       if item.at == block.at else item for item in body.blocks))
    result = arrayfacts.proven(body)
    store = next(op.stores[0] for op in result.block(0x9a).ops if op.at == 0xdd)
    slot = next(op.loads[0] for op in result.block(0x9a).ops if op.at == 0x9a and op.loads)
    assert mir.overlapping(store, slot, frozenset({5})) is (case != "bounded")


@pytest.mark.parametrize("low,high,width", [(-7, 0, 4), (0, 65530, 4), (0, 2, 0)])
def test_near_region_requires_nonwrapping_complete_access(low, high, width):
    from qbopt.frontend.addressfacts import region
    from qbopt.analysis.ranges import Interval
    assert region(Addr(Space.SEGMENT, 6, 5), Interval(low, high, 2), width) is None


def diamond():
    descriptor = mir.Symbol(Space.SEGMENT, 5, 16, 2)
    header = mir.MemRef(Addr(Space.SEGMENT, 16, 5), 4)
    request = mir.ArrayRequest(descriptor, 2, ((1, 4),))
    allocation = mir.Op(0, ir.Operation.CALL, "", (), (), kind=mir.Kind.CALL, array=request,
                        memory_values=((replace(header, addr=header.addr.plus(14), width=2), mir.Const(4, 2)),))
    pointer, offset = mir.Value(1, 1), mir.Value(2, 2)
    base = mir.Op(1, ir.Operation.MOVE, "", (pointer,), (), kind=mir.Kind.LOAD,
                  args=(mir.Cell(header),), results=(mir.Held(pointer, 4),), loads=(header,))
    advance = mir.Op(2, ir.Operation.BINARY, "", (offset,), (pointer,), kind=mir.Kind.PTR_OFFSET,
                     args=(mir.Held(pointer, 4), mir.Const(2, 4)), results=(mir.Held(offset, 4),))
    cell = mir.MemRef(None, 2, base=offset, pointer=True)
    def store(at):
        return mir.Op(at, ir.Operation.MOVE, "", (), (offset,), kind=mir.Kind.STORE,
                      args=(mir.Const(at, 2),), results=(mir.Cell(cell),), stores=(cell,))
    value = mir.Value(3, 30)
    read = mir.Op(30, ir.Operation.MOVE, "", (value,), (offset,), kind=mir.Kind.LOAD,
                  args=(mir.Cell(cell),), results=(mir.Held(value, 2),), loads=(cell,))
    return mir.MirBody(0, (mir.MirBlock(0, (), (allocation, base, advance), (10, 20)),
                           mir.MirBlock(10, (), (store(10),), (30,)),
                           mir.MirBlock(20, (), (store(20),), (30,)),
                           mir.MirBlock(30, (), (read,), ())))


@pytest.mark.parametrize("reverse", [False, True])
def test_unknown_branch_preserves_a_bounded_pointer(reverse):
    body = diamond()
    if reverse:
        body = replace(body, blocks=tuple(reversed(body.blocks)))
    after = arrayfacts.proven(body)
    pointers = [ref for block in after.blocks for op in block.ops for ref in (*op.loads, *op.stores) if ref.pointer]
    assert len(pointers) == 3 and all(ref.allocation for ref in pointers)


@pytest.mark.parametrize("offset", [-2, 7, 8, 0x80000000])
def test_outside_the_extent_is_not_owned(offset):
    body = diamond()
    block = body.blocks[0]
    advance = replace(block.ops[-1], args=(block.ops[-1].args[0], mir.Const(offset, 4)))
    body = replace(body, blocks=(replace(block, ops=(*block.ops[:-1], advance)), *body.blocks[1:]))
    assert arrayfacts.proven(body) == body


def test_one_path_invalidating_allocation_prevents_join_proof():
    body = diamond()
    right = body.blocks[2]
    call = mir.Op(21, ir.Operation.CALL, "", (), (), kind=mir.Kind.CALL)
    body = replace(body, blocks=(*body.blocks[:2], replace(right, ops=(*right.ops, call)), body.blocks[-1]))
    after = arrayfacts.proven(body)
    assert not after.blocks[-1].ops[0].loads[0].allocation


def test_budget_exhaustion_adds_no_facts():
    body = diamond()
    assert arrayfacts.proven(body, limit=1) is body


def test_interval_cannot_acquire_unwritten_high_bits():
    from qbopt.analysis.ranges import Interval
    assert arrayfacts._fitted(Interval(-1, 0, 2), 4) is None


def test_copy_does_not_implicitly_extend_an_offset():
    source, target = mir.Value(50, 0), mir.Value(51, 1)
    op = mir.Op(1, ir.Operation.MOVE, "", (target,), (source,), kind=mir.Kind.COPY,
                args=(mir.Held(source, 2),), results=(mir.Held(target, 4),))
    assert arrayfacts._result(op, [65535]) is None


def test_narrow_constant_does_not_prove_a_wider_offset():
    """A low word of 2 does not bound an offset whose high word is unknown."""
    body = diamond()
    entry = body.blocks[0]
    delta = mir.Value(9, 2)
    constant = mir.Op(2, ir.Operation.MOVE, "", (delta,), (), kind=mir.Kind.COPY,
                      args=(mir.Const(2, 2),), results=(mir.Held(delta, 2),))
    advance = replace(entry.ops[-1], args=(entry.ops[-1].args[0], mir.Held(delta, 4)))
    body = replace(body, blocks=(replace(entry, ops=(*entry.ops[:-1], constant, advance)), *body.blocks[1:]))
    assert arrayfacts.proven(body) == body


def test_new_allocation_does_not_revive_a_stale_pointer():
    """Reusing a descriptor cannot make a pointer into its previous allocation valid."""
    body = diamond()
    entry = body.blocks[0]
    second = replace(entry.ops[0], at=3)
    body = replace(body, blocks=(replace(entry, ops=(*entry.ops, second)), *body.blocks[1:]))
    assert arrayfacts.proven(body) == body


@pytest.mark.parametrize("tag", ["p-g2", "q-O", "v-g3"])
def test_arrphi_proves_both_unknown_branches_and_the_join(tag):
    """ARRPHI prints 10,9; its six bounded accesses used to lose all allocation facts."""
    import corpus
    path = Path(f"fixtures/regressions/arrphi-{tag}.obj".lower())
    states = [body for _, body in mir.bodies(corpus.loaded(path), corpus.partitioned(path))]
    pointers = [ref for body in states for block in body.blocks for op in block.ops
                for ref in (*op.loads, *op.stores) if ref.pointer]
    assert len(pointers) == 6
    assert all(ref.allocation is not None for ref in pointers)


@pytest.mark.parametrize("tag", ["p-g2", "q-O", "v-g3"])
def test_hugerg_has_an_inductive_extent_proof_with_a_small_budget(tag, monkeypatch):
    """HUGERG's 456,457,789,790 output needs 197 iterations; exact walking exhausted 10,000 operations."""
    import corpus
    from qbopt.frontend import raising_array_bounds
    with monkeypatch.context() as context:
        context.setattr(raising_array_bounds, "proven", lambda body: body)
        path = Path(f"fixtures/regressions/hugerg-{tag}.obj".lower())
        body = mir.bodies(corpus.loaded(path), corpus.partitioned(path))[0][1]
    after = arrayfacts.proven(body, limit=1000)
    stores = [ref for block in after.blocks for op in block.ops for ref in op.stores if ref.pointer]
    assert len(stores) == 2 and all(ref.allocation is not None for ref in stores)
    from qbopt import wholeseg
    from qbopt.analysis import loops
    states = []
    def watch(stage, name, state):
        if stage == "mir-widen":
            states.append(state)
    result = wholeseg.emitted(path.read_bytes(), watch=watch)
    assert result.outcome is wholeseg.Emission.LIR, result.reason
    optimized, = states
    inside = {at for loop in loops.loops(optimized.blocks, optimized.entry) for at in loop.body}
    assert inside
    assert not any(ref.addr is not None and ref.addr.space is Space.SEGMENT
                   and ref.addr.index == corpus.loaded(path).program_data and 6 <= ref.addr.disp < 28
                   for block in optimized.blocks if block.at in inside for op in block.ops for ref in op.loads)


@pytest.mark.parametrize("failure", ["out_of_bounds", "unknown_guard", "call"])
def test_inductive_proof_must_preserve_its_own_preconditions(failure, monkeypatch):
    import corpus
    from qbopt.frontend import raising_array_bounds
    with monkeypatch.context() as context:
        context.setattr(raising_array_bounds, "proven", lambda body: body)
        path = Path("fixtures/regressions/hugerg-p-g2.obj")
        body = mir.bodies(corpus.loaded(path), corpus.partitioned(path))[0][1]
    def changed(op):
        if op.kind is mir.Kind.PTR_OFFSET and failure == "out_of_bounds":
            return replace(op, args=(op.args[0], mir.Const(80802, 4)))
        if op.kind is mir.Kind.BRANCH and failure == "unknown_guard":
            return replace(op, uses=(mir.Value(9999, 0, flags=True),))
        if op.kind is mir.Kind.STORE and any(ref.pointer for ref in op.stores) and failure == "call":
            return mir.Op(op.at, ir.Operation.CALL, "", (), (), kind=mir.Kind.CALL)
        return op
    body = replace(body, blocks=tuple(replace(block, ops=tuple(map(changed, block.ops))) for block in body.blocks))
    after = arrayfacts.proven(body)
    assert not any(ref.allocation for block in after.blocks for op in block.ops for ref in op.stores)
