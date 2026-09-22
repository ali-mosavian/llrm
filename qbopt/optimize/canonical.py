"""Canonical forms, as LLVM's InstCombine puts them: compares with the constant on the right, no neutral terms.

Folding makes `1 sub v` whenever a compare's left operand becomes known, and
no machine compares an immediate against a register in that order. The
compare is swapped and every test reading its flags mirrored; a reader that
is not a test gets the constant as a copied value instead.
"""

from dataclasses import replace

from qbopt.model import ir
from qbopt.model import mir
from qbopt.analysis import ssa
from qbopt.analysis import consts


def compares(body: mir.MirBody) -> mir.MirBody:
    readers: dict[mir.Value, list[mir.Op]] = {}
    for block in body.blocks:
        for op in block.ops:
            for value in op.uses:
                readers.setdefault(value, []).append(op)
    swapped: set[mir.Value] = set()
    copied: set[int] = set()
    for block in body.blocks:
        for op in block.ops:
            if not _constant_left(op):
                continue
            tests = all(
                one.kind is mir.Kind.BRANCH and one.test in mir.MIRRORED for one in readers.get(op.defines[0], ())
            )
            if tests and not isinstance(op.args[1], mir.Const):
                swapped.add(op.defines[0])
            else:
                copied.add(id(op))
    if not swapped and not copied:
        return body

    values = tuple(ssa.values(body))
    serial = max((value.id for value in values), default=0)
    variable = max((value.variable for value in values), default=0)
    blocks = []
    for block in body.blocks:
        ops = []
        for op in block.ops:
            if _constant_left(op) and op.defines[0] in swapped:
                ops.append(replace(op, args=(op.args[1], op.args[0])))
            elif id(op) in copied:
                serial += 1
                variable += 1
                held = mir.Value(serial, op.at, variable=variable, version=1)
                constant = op.args[0]
                ops.append(
                    mir.Op(
                        op.at,
                        ir.Operation.MOVE,
                        "",
                        (held,),
                        (),
                        kind=mir.Kind.COPY,
                        args=(constant,),
                        results=(mir.Held(held, constant.width),),
                    )
                )
                ops.append(replace(op, uses=(held, *op.uses), args=(mir.Held(held, constant.width), *op.args[1:])))
            elif op.kind is mir.Kind.BRANCH and swapped.intersection(op.uses):
                ops.append(replace(op, test=mir.MIRRORED[op.test]))
            else:
                ops.append(op)
        blocks.append(replace(block, ops=tuple(ops)))
    return replace(body, blocks=tuple(blocks))


def _constant_left(op: mir.Op) -> bool:
    return (
        op.kind is mir.Kind.SUB
        and not op.results
        and len(op.defines) == 1
        and len(op.args) == 2
        and isinstance(op.args[0], mir.Const)
    )


def identities(body: mir.MirBody) -> mir.MirBody:
    """`x + 0`, `x - 0` and `x * 1` are `x`, and a test `x <=u 0` is `x == 0`.

    A rewrite states what it computes from a proof in full -- rotation's
    trip count is `bound - start + inclusive` for any start -- and the
    neutral terms go here, so no rewrite folds its own.
    """
    read = {value for block in body.blocks for op in block.ops for value in op.uses}
    read |= {value for block in body.blocks for phi in block.phis for value in phi.incoming.values()}
    swap: dict[int, mir.Value] = {}
    copies: dict[int, mir.Op] = {}
    zero_tests: set[mir.Value] = set()
    for block in body.blocks:
        for op in block.ops:
            if _zero_test(op):
                zero_tests.add(op.defines[0])
            kept = _neutral(op)
            if kept is None or any(value.flags and value in read for value in op.defines):
                continue
            (result,) = op.results
            if isinstance(kept, mir.Held):
                swap[result.value.id] = kept.value
            else:
                copies[id(op)] = replace(
                    op,
                    kind=mir.Kind.COPY,
                    defines=(result.value,),
                    uses=(),
                    args=(kept,),
                    source_backed=False,
                    raised=None,
                    symbol=False,
                )
    tests = {mir.Kind.BELOW_EQ: mir.Kind.EQ, mir.Kind.ABOVE: mir.Kind.NE}
    renamed = any(
        op.kind is mir.Kind.BRANCH and op.test in tests and zero_tests.intersection(op.uses)
        for block in body.blocks
        for op in block.ops
    )
    if not swap and not copies and not renamed:
        return body
    blocks = []
    for block in body.blocks:
        ops = []
        for op in block.ops:
            if op.results and isinstance(op.results[0], mir.Held) and op.results[0].value.id in swap:
                if op.id is not None or op.source_backed or op.absorbed:
                    ops.append(mir.cleared(op))  # its bytes stay owned
                continue
            op = ssa.substituted(copies.get(id(op), op), swap)
            if op.kind is mir.Kind.BRANCH and op.test in tests and zero_tests.intersection(op.uses):
                op = replace(op, test=tests[op.test])  # nothing is below zero
            ops.append(op)
        phis = tuple(
            replace(phi, incoming={at: ssa.provider(value, swap) for at, value in phi.incoming.items()})
            for phi in block.phis
        )
        blocks.append(replace(block, phis=phis, ops=tuple(ops)))
    return replace(body, blocks=tuple(blocks))


def _neutral(op: mir.Op) -> mir.Held | mir.Const | None:
    """The operand a pure `x + 0`, `x - 0` or `x * 1` passes through unchanged."""
    if (
        len(op.args) != 2
        or len(op.results) != 1
        or not isinstance(op.results[0], mir.Held)
        or op.loads
        or op.stores
        or op.merges
        or op.barrier
        or op.floating is not None
    ):
        return None
    width = op.results[0].width
    identity = {mir.Kind.ADD: 0, mir.Kind.SUB: 0, mir.Kind.MUL: 1}.get(op.kind)
    if identity is None:
        return None
    left, right = op.args
    commutes = op.kind is not mir.Kind.SUB
    for kept, other in ((left, right), (right, left))[: 1 + commutes]:
        if (
            isinstance(other, mir.Const)
            and consts.masked(other.n, width) == identity
            and isinstance(kept, (mir.Held, mir.Const))
            and kept.width == width
        ):
            return kept
    return None


def _zero_test(op: mir.Op) -> bool:
    return (
        op.kind is mir.Kind.SUB
        and not op.results
        and len(op.defines) == 1
        and len(op.args) == 2
        and isinstance(op.args[1], mir.Const)
        and consts.masked(op.args[1].n, op.args[1].width) == 0
    )
