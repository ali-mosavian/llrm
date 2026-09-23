"""NDMAX stored 11, then 22 six bytes away, but reloaded the first element for PRINT."""

from dataclasses import replace

import pytest

from qbopt.analysis import consts
from qbopt.model import ir, mir
from qbopt.objectfile.module import Space
from qbopt.optimize import transform


def body_with_store(offset=6, *, owned=True):
    pointer, advanced, result = mir.Value(1, 0), mir.Value(2, 1), mir.Value(3, 4)
    allocation = mir.Symbol(Space.SEGMENT, 1, 0, 2) if owned else None
    first = mir.MemRef(None, 2, base=pointer, pointer=True, allocation=allocation)
    second = replace(first, base=advanced)
    store = mir.Op(0, ir.Operation.MOVE, "", (), (pointer,), kind=mir.Kind.STORE,
                   args=(mir.Const(11, 2),), results=(mir.Cell(first),), stores=(first,))
    advance = mir.Op(1, ir.Operation.BINARY, "", (advanced,), (pointer,), kind=mir.Kind.PTR_OFFSET,
                     args=(mir.Held(pointer, 4), mir.Const(offset, 4)), results=(mir.Held(advanced, 4),))
    other = mir.Op(2, ir.Operation.MOVE, "", (), (advanced,), kind=mir.Kind.STORE,
                   args=(mir.Const(22, 2),), results=(mir.Cell(second),), stores=(second,))
    load = mir.Op(4, ir.Operation.MOVE, "", (result,), (pointer,), kind=mir.Kind.LOAD,
                  args=(mir.Cell(first),), results=(mir.Held(result, 2),), loads=(first,))
    return mir.MirBody(0, (mir.MirBlock(0, (), (store, advance, other, load), ()),))


@pytest.mark.parametrize("offset", [2, 6, -2])
def test_disjoint_pointer_store_preserves_the_constant(offset):
    body = body_with_store(offset)
    result = body.blocks[0].ops[-1].defines[0]
    assert consts.known(body, frozenset(), {})[result] == consts.Known(11, 2)
    load = transform.folded(body, frozenset(), {}).blocks[0].ops[-1]
    assert load.args == (mir.Const(11, 2),)
    assert not load.loads


@pytest.mark.parametrize("offset", [-1, 1])
def test_partially_overlapping_pointer_store_keeps_the_load(offset):
    body = body_with_store(offset)
    assert transform.folded(body, frozenset(), {}).blocks[0].ops[-1].loads


def test_unbounded_pointer_offsets_are_not_a_disjointness_proof():
    body = body_with_store(owned=False)
    assert transform.folded(body, frozenset(), {}).blocks[0].ops[-1].loads


def test_unknown_call_invalidates_a_pointer_constant():
    body = body_with_store()
    block = body.blocks[0]
    call = mir.Op(3, ir.Operation.CALL, "", (), (), kind=mir.Kind.CALL)
    body = replace(body, blocks=(replace(block, ops=(*block.ops[:-1], call, block.ops[-1])),))
    assert transform.folded(body, frozenset(), {}).blocks[0].ops[-1].loads


def test_unknown_pointer_root_may_alias_the_first_store():
    body = body_with_store()
    block = body.blocks[0]
    advance = replace(block.ops[1], args=(mir.Held(mir.Value(9, 0), 4), mir.Const(6, 4)))
    body = replace(body, blocks=(replace(block, ops=(block.ops[0], advance, *block.ops[2:])),))
    assert transform.folded(body, frozenset(), {}).blocks[0].ops[-1].loads


def test_pointer_store_value_is_forwarded_without_being_constant():
    body = body_with_store()
    block = body.blocks[0]
    value = mir.Value(7, 0)
    store = replace(block.ops[0], args=(mir.Held(value, 2),), uses=(*block.ops[0].uses, value))
    body = replace(body, blocks=(replace(block, ops=(store, *block.ops[1:])),))
    load = transform.forwarded(body, frozenset(), {}).blocks[0].ops[-1]
    assert not load.loads
    assert load.args == (mir.Held(value, 2),)


def test_pointer_store_on_only_one_branch_cannot_supply_a_join():
    body = body_with_store()
    store, _, _, load = body.blocks[0].ops
    body = replace(body, blocks=(mir.MirBlock(0, (), (), (1, 2)),
                                 mir.MirBlock(1, (), (store,), (2,)),
                                 mir.MirBlock(2, (), (load,), ())))
    assert transform.folded(body, frozenset(), {}).blocks[-1].ops[0].loads


def test_copy_of_a_pointer_retains_the_relative_offset():
    from qbopt.analysis import pointerfacts
    body = body_with_store()
    block = body.blocks[0]
    copied = mir.Value(5, 3)
    original = block.ops[1].results[0]
    copy = mir.Op(3, ir.Operation.MOVE, "", (copied,), (original.value,), kind=mir.Kind.COPY,
                  args=(original,), results=(mir.Held(copied, 4),))
    body = replace(body, blocks=(replace(block, ops=(*block.ops[:2], copy, *block.ops[2:])),))
    first, second = block.ops[0].stores[0], replace(block.ops[2].stores[0], base=copied)
    assert pointerfacts.offsets(body).disjoint(first, second)
    narrow = replace(copy, results=(mir.Held(copied, 2),))
    body = replace(body, blocks=(replace(block, ops=(*block.ops[:2], narrow, *block.ops[2:])),))
    assert not pointerfacts.offsets(body).disjoint(first, second)


@pytest.mark.parametrize("tag", ["p-g2", "q-O", "v-g3"])
def test_ndmax_print_uses_the_stored_constant(tag):
    from pathlib import Path
    import corpus
    path = Path(f"fixtures/regressions/ndmax-{tag}.obj".lower())
    found = corpus.loaded(path)
    blocks = corpus.partitioned(path)
    body = mir.bodies(found, blocks, bounds_checks=True)[0][1]
    body = transform.applied(body, found.dgroup, found.calls, blocks=blocks, found=found)
    first_print = min(at for at, name in found.calls.items() if name == "B$PEI2")
    argument = [op for block in body.blocks for op in block.ops
                if op.kind is mir.Kind.ARG and op.at < first_print][-1]
    assert argument.args == (mir.Const(11, 2),)
