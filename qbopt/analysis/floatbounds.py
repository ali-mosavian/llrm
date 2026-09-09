"""Exception-free floating work over bounded, not necessarily known, integers."""

from qbopt.model import mir
from qbopt.model.floating import Format, Precision

type Bounds = tuple[int, int]

_SIGNED = {Format.SIGNED16: 16, Format.SIGNED32: 32, Format.SIGNED64: 64}


def evaluated(kind, rule, inputs: tuple[Bounds, ...]) -> Bounds | None:
    if len(inputs) != len(rule.inputs):
        return None
    match kind, inputs:
        case mir.Kind.FLOAD | mir.Kind.FSTORE, (value,):
            result = value
        case mir.Kind.FNEG, ((low, high),):
            result = -high, -low
        case mir.Kind.FABS, ((low, high),):
            result = (0 if low <= 0 <= high else min(abs(low), abs(high)), max(abs(low), abs(high)))
        case mir.Kind.FADD, ((low, high), (other_low, other_high)):
            result = low + other_low, high + other_high
        case mir.Kind.FSUB, ((low, high), (other_low, other_high)):
            result = low - other_high, high - other_low
        case mir.Kind.FMUL, ((low, high), (other_low, other_high)):
            products = low * other_low, low * other_high, high * other_low, high * other_high
            result = min(products), max(products)
        case _:
            return None
    if rule.result in _SIGNED:
        limit = 1 << (_SIGNED[rule.result] - 1)
        return result if -limit <= result[0] <= result[1] < limit else None
    match rule.result:
        case Format.BINARY32:
            precision = 24
        case Format.BINARY64:
            precision = 53
        case Format.EXTENDED80:
            precision = 24 if rule.precision is Precision.DYNAMIC else 64
        case _:
            return None
    limit = 1 << precision
    return result if -limit <= result[0] <= result[1] <= limit else None


def exact(body: mir.MirBody, constants: dict) -> set[int]:
    """Operation identities proven numerically exact; pending checks remain separate.

    These bounds do not assert the sign of zero or any particular value.
    No memory contents or floating environment are assumed across calls.
    """
    safe = set()
    for block in body.blocks:
        values = {}
        for op in block.ops:
            if op.barrier or op.kind in (mir.Kind.CALL, mir.Kind.OPAQUE):
                values.clear()
                continue
            if op.floating is None:
                continue
            inputs = []
            for arg, format in zip(op.args, op.floating.inputs):
                if format in _SIGNED:
                    limit = 1 << (_SIGNED[format] - 1)
                    bounds = -limit, limit - 1
                elif isinstance(arg, mir.Held):
                    bounds = values.get(arg.value)
                    fact = constants.get(arg.value)
                    if fact is not None and fact.value.denominator == 1:
                        bounds = int(fact.value), int(fact.value)
                else:
                    bounds = None
                if bounds is None:
                    break
                inputs.append(bounds)
            result = evaluated(op.kind, op.floating, tuple(inputs))
            if result is None:
                continue
            safe.add(id(op))
            for arg in op.results:
                if isinstance(arg, mir.Held):
                    values[arg.value] = result
    return safe
