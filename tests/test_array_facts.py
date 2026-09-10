"""Unknown branches must not erase independently proven constant array extents."""

from dataclasses import replace
from pathlib import Path

import pytest

from qbopt.frontend import arrayfacts
from qbopt.model import ir, mir
from qbopt.objectfile.module import Addr, Space


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
    path = Path(f"fixtures/regressions/arrphi-{tag}.obj")
    states = [body for _, body in mir.bodies(corpus.loaded(path), corpus.partitioned(path))]
    pointers = [ref for body in states for block in body.blocks for op in block.ops
                for ref in (*op.loads, *op.stores) if ref.pointer]
    assert len(pointers) == 6
    assert all(ref.allocation is not None for ref in pointers)
