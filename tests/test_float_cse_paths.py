"""Exact floating reuse must follow every intervening control-flow path."""

from dataclasses import replace

from qbopt.model import ir, mir
from qbopt.model.floating import Format, Precision, Rounding, Semantics
from qbopt.optimize import transform


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
