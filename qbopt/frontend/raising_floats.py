"""Translate decoded floating shapes into explicit evaluation semantics."""

from dataclasses import replace

from qbopt.model import ir
from qbopt.model import mir
from qbopt.model.floating import Format
from qbopt.model.floating import Rounding
from qbopt.model.floating import Precision
from qbopt.model.floating import Semantics


def _format(width: int, integer: bool) -> Format | None:
    if integer:
        return {2: Format.SIGNED16, 4: Format.SIGNED32, 8: Format.SIGNED64}.get(width)
    return {4: Format.BINARY32, 8: Format.BINARY64, 10: Format.EXTENDED80}.get(width)


def semantics(op: mir.Op) -> Semantics | None:
    match op.op:
        case ir.Operation.FLOAT_LOAD:
            if (
                op.name == "fild"
                and not op.loads
                and not op.stores
                and len(op.args) == 1
                and isinstance(op.args[0], (mir.Held, mir.Const))
            ):
                source = _format(op.args[0].width, True)
                if source is not None:
                    return Semantics((source,), Format.EXTENDED80, Precision.EXACT, Rounding.NONE)
            if op.name not in ("fld", "fild") or len(op.loads) != 1 or op.stores:
                return None
            source = _format(op.loads[0].width, op.name == "fild")
            if source is None:
                return None
            return Semantics((source,), Format.EXTENDED80, Precision.EXACT, Rounding.NONE)
        case ir.Operation.FLOAT_STORE:
            if (
                op.name in ("fistp", "fisttp")
                and not op.stores
                and not op.loads
                and len(op.results) == 1
                and isinstance(op.results[0], mir.Held)
            ):
                target = _format(op.results[0].width, True)
                if target is not None:
                    rounding = Rounding.TOWARD_ZERO if op.name == "fisttp" else Rounding.DYNAMIC
                    return Semantics((Format.EXTENDED80,), target, Precision.DESTINATION, rounding)
            if op.name not in ("fstp", "fistp") or len(op.stores) != 1 or op.loads:
                return None
            target = _format(op.stores[0].width, op.name == "fistp")
            if target is None:
                return None
            rounding = Rounding.NONE if target is Format.EXTENDED80 else Rounding.DYNAMIC
            return Semantics((Format.EXTENDED80,), target, Precision.DESTINATION, rounding)
        case ir.Operation.FLOAT_ARITH:
            if (
                op.name in ("fadd", "fsub", "fmul", "fdiv")
                and not op.loads
                and not op.stores
                and len(op.args) == 2
                and len(op.results) == 1
                and all(
                    (isinstance(arg, mir.Opaque) and isinstance(arg.what, ir.St))
                    or (isinstance(arg, mir.Held) and arg.width == 10)
                    for arg in (*op.args, *op.results)
                )
            ):
                return Semantics(
                    (Format.EXTENDED80, Format.EXTENDED80), Format.EXTENDED80, Precision.DYNAMIC, Rounding.DYNAMIC
                )
            if op.name not in ("fadd", "fsub", "fmul", "fdiv", "fidiv", "fisub") or len(op.loads) != 1 or op.stores:
                return None
            source = _format(op.loads[0].width, op.name in ("fidiv", "fisub"))
            if source is None:
                return None
            return Semantics((Format.EXTENDED80, source), Format.EXTENDED80, Precision.DYNAMIC, Rounding.DYNAMIC)
        case ir.Operation.FLOAT_ARITH_POP:
            if op.name not in ("faddp", "fsubp", "fmulp", "fdivp") or op.loads or op.stores:
                return None
            return Semantics(
                (Format.EXTENDED80, Format.EXTENDED80), Format.EXTENDED80, Precision.DYNAMIC, Rounding.DYNAMIC
            )
        case ir.Operation.FLOAT_UNARY:
            if op.name not in ("fchs", "fabs", "fsqrt") or op.loads or op.stores:
                return None
            exact = op.name in ("fchs", "fabs")
            return Semantics(
                (Format.EXTENDED80,),
                Format.EXTENDED80,
                Precision.EXACT if exact else Precision.DYNAMIC,
                Rounding.NONE if exact else Rounding.DYNAMIC,
            )
    return None


def annotated(body: mir.MirBody) -> mir.MirBody:
    def annotated_op(op):
        if (
            op.op is ir.Operation.NOTHING
            and op.name in ("wait", "fwait")
            and not op.defines
            and not op.uses
            and not op.loads
            and not op.stores
        ):
            return replace(op, kind=mir.Kind.FCHECK, name="")
        return replace(op, floating=semantics(op))

    return replace(
        body, blocks=tuple(replace(block, ops=tuple(annotated_op(op) for op in block.ops)) for block in body.blocks)
    )
