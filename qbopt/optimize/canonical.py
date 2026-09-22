"""Compares with the constant on the right, as LLVM's InstCombine puts them.

Folding makes `1 sub v` whenever a compare's left operand becomes known, and
no machine compares an immediate against a register in that order. The
compare is swapped and every test reading its flags mirrored; a reader that
is not a test gets the constant as a copied value instead.
"""

from dataclasses import replace

from qbopt.model import ir
from qbopt.model import mir
from qbopt.analysis import ssa


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
