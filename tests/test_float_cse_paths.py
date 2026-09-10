"""Exact floating reuse must follow every intervening control-flow path."""

from dataclasses import replace
import pytest

from qbopt.model import ir, mir
from qbopt.model.floating import Format, Precision, Rounding, Semantics
from qbopt.optimize import transform
from qbopt.analysis import floatbounds
from qbopt.objectfile.module import Addr, Space


def body_with_path(barrier: bool = False) -> mir.MirBody:
    source, first, second = (mir.Value(n, n, variable=n) for n in (1, 2, 3))
    load = mir.Op(0, ir.Operation.FLOAT_LOAD, "fild", (source,), (), kind=mir.Kind.FLOAD,
                  args=(mir.Const(7, 2),), results=(mir.Held(source, 10),),
                  floating=Semantics((Format.SIGNED16,), Format.EXTENDED80, Precision.EXACT, Rounding.NONE))

    def add(at: int, result: mir.Value) -> mir.Op:
        return mir.Op(at, ir.Operation.FLOAT_ARITH, "fadd", (result,), (source,), kind=mir.Kind.FADD,
                      args=(mir.Held(source, 10), mir.Held(source, 10)), results=(mir.Held(result, 10),),
                      floating=Semantics((Format.EXTENDED80, Format.EXTENDED80), Format.EXTENDED80,
                                         Precision.DYNAMIC, Rounding.DYNAMIC))

    middle = (mir.Op(4, ir.Operation.BARRIER, "", (), (), kind=mir.Kind.OPAQUE),) if barrier else ()
    use = mir.Op(8, ir.Operation.NOTHING, "", (), (second,), kind=mir.Kind.ARG,
                 args=(mir.Held(second, 10),))
    return mir.MirBody(0, (
        mir.MirBlock(0, (), (load, add(2, first)), (4, 5)),
        mir.MirBlock(4, (), middle, (6,)), mir.MirBlock(5, (), (), (6,)),
        mir.MirBlock(6, (), (add(6, second), use), ()),
    ))


def test_exact_float_add_is_reused_after_a_diamond() -> None:
    body = body_with_path()
    after = transform.subexpressions(body)
    assert sum(op.kind is mir.Kind.FADD for block in after.blocks for op in block.ops) == 1
    assert after.blocks[-1].ops[-1].args == body.blocks[0].ops[-1].results


def test_one_opaque_path_blocks_float_reuse() -> None:
    after = transform.subexpressions(body_with_path(barrier=True))
    assert sum(op.kind is mir.Kind.FADD for block in after.blocks for op in block.ops) == 2


def test_cycle_between_candidates_requires_an_environment_invariant() -> None:
    body = body_with_path()
    body = replace(body, blocks=tuple(
        replace(block, succ=(4, 6)) if block.at == 4 else block for block in body.blocks
    ))
    after = transform.subexpressions(body)
    assert sum(op.kind is mir.Kind.FADD for block in after.blocks for op in block.ops) == 2


def test_runtime_integer_bounds_enable_cross_block_float_reuse() -> None:
    body = body_with_path()
    cell = mir.MemRef(Addr(Space.SEGMENT, 0, 1), 2)
    load = replace(body.blocks[0].ops[0], args=(mir.Cell(cell),), loads=(cell,))
    body = replace(body, blocks=(replace(body.blocks[0], ops=(load, body.blocks[0].ops[1])), *body.blocks[1:]))
    after = transform.subexpressions(body)
    assert sum(op.kind is mir.Kind.FADD for block in after.blocks for op in block.ops) == 1


def test_bounds_do_not_depend_on_block_listing_order() -> None:
    body = body_with_path()
    cell = mir.MemRef(Addr(Space.SEGMENT, 0, 1), 2)
    load = replace(body.blocks[0].ops[0], args=(mir.Cell(cell),), loads=(cell,))
    entry = replace(body.blocks[0], ops=(load, body.blocks[0].ops[1]))
    body = replace(body, blocks=(*body.blocks[1:], entry))
    safe = floatbounds.exact(body, {})
    assert id(body.block(6).ops[0]) in safe


@pytest.mark.parametrize("unknown", [False, True])
def test_phi_bounds_require_every_incoming_value(unknown: bool) -> None:
    body = body_with_path()
    load = body.blocks[0].ops[0]
    left, right, joined = (mir.Value(n, n, variable=n) for n in (10, 11, 12))
    cell = mir.MemRef(Addr(Space.SEGMENT, 0, 1), 2)
    def reading(at: int, value: mir.Value) -> mir.Op:
        return replace(load, at=at, defines=(value,), results=(mir.Held(value, 10),),
                       args=(mir.Cell(cell),), loads=(cell,))
    add = replace(body.block(6).ops[0], uses=(joined,), args=(mir.Held(joined, 10), mir.Held(joined, 10)))
    body = mir.MirBody(0, (
        mir.MirBlock(0, (), (), (4, 5)),
        mir.MirBlock(4, (), (reading(4, left),), (6,)),
        mir.MirBlock(5, (), () if unknown else (reading(5, right),), (6,)),
        mir.MirBlock(6, (mir.Phi(joined, {4: left, 5: right}),), (add,), ()),
    ))
    assert (id(add) in floatbounds.exact(body, {})) is not unknown


def test_cyclic_phi_cannot_assume_its_seed_bounds_hold_forever() -> None:
    body = body_with_path()
    seed = body.blocks[0].ops[0]
    carried = mir.Value(20, 6, variable=20)
    add = replace(body.block(6).ops[0], uses=(carried,),
                  args=(mir.Held(carried, 10), mir.Held(carried, 10)))
    body = mir.MirBody(0, (
        mir.MirBlock(0, (), (seed,), (6,)),
        mir.MirBlock(6, (mir.Phi(carried, {0: seed.defines[0], 6: add.defines[0]}),), (add,), (6,)),
    ))
    assert id(add) not in floatbounds.exact(body, {})
