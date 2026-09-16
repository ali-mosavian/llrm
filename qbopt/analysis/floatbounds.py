"""Exception-free floating work over bounded, not necessarily known, integers."""

from dataclasses import replace

from qbopt.analysis import consts, floatfacts, ranges
from qbopt.model import mir
from qbopt.model.floating import Format, Precision

type Bounds = tuple[int, int]

_SIGNED = {Format.SIGNED16: 16, Format.SIGNED32: 32, Format.SIGNED64: 64}
_UNSIGNED = {Format.UNSIGNED64: 64}


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
    if rule.result in _UNSIGNED:
        limit = 1 << _UNSIGNED[rule.result]
        return result if 0 <= result[0] <= result[1] < limit else None
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


def _memory(arg, format, memory, scoped, definitions):
    """Bound every possible aligned element using bytes proved at this read."""
    if not isinstance(arg, mir.Cell):
        return None
    ref = mir._symbolic_ref(arg.ref)
    widths = {Format.BINARY32: 4, Format.BINARY64: 8}
    if ref.width != widths.get(format):
        return None
    offsets = (0,)
    if ref.base is not None:
        interval = scoped.get(ref.base)
        covered = ranges.covering(ref, scoped)
        if interval is None or covered.base is not None:
            return None
        stride = 1
        producer = definitions.get(ref.base)
        if (producer is not None and producer.kind is mir.Kind.SHL
            and producer.results == (mir.Held(ref.base, ref.base_width),)
            and len(producer.args) == 2 and isinstance(producer.args[1], mir.Const)
            and 0 <= producer.args[1].n < ref.base_width * 8):
            stride = 1 << producer.args[1].n
        start = -(-interval.low // stride) * stride
        offsets = range(start, interval.high + 1, stride)
        if not offsets or len(offsets) > 64:
            return None
    if ref.addr is None or ref.segment is not None:
        return None
    if ref.base is None and ref.addr.base:
        return None
    values = []
    for offset in offsets:
        cell = replace(ref, addr=replace(ref.addr, disp=ref.addr.disp + offset, base=0), base=None)
        bits = consts._cell(memory, cell)
        value = floatfacts.decoded(bits.n, format) if bits is not None else None
        if value is None or value.value.denominator != 1:
            return None
        values.append(int(value.value))
    return min(values), max(values)


def exact(body: mir.MirBody, constants: dict, dgroup: frozenset[int] = frozenset()) -> set[int]:
    """Operation identities proven numerically exact; pending checks remain separate.

    These bounds do not assert the sign of zero or any particular value.
    SSA value bounds survive control flow and calls; memory contents and
    the floating environment do not. Cyclic phis without independently
    bounded inputs remain unknown.
    """
    if not any(op.floating is not None for block in body.blocks for op in block.ops):
        return set()
    safe = set()
    shadow = replace(body, blocks=tuple(replace(block, ops=tuple(
        replace(op, stores=(mir.MemRef(None, 0, None, None),))
        if op.barrier or op.kind is mir.Kind.OPAQUE or (op.kind is mir.Kind.CALL and not op.stores)
        else op for op in block.ops
    )) for block in body.blocks))
    memory = floatfacts.cells(shadow, dgroup, {})
    scoped = ranges.bounded(body)
    definitions = {value: op for block in body.blocks for op in block.ops for value in op.defines}
    values: dict[mir.Value, Bounds] = {}
    pending = [(block.at, index, op) for block in body.blocks for index, op in enumerate(block.ops)
               if op.floating is not None and not op.barrier and op.kind not in (mir.Kind.CALL, mir.Kind.OPAQUE)]
    phis = [phi for block in body.blocks for phi in block.phis if not phi.result.flags]
    changed = True
    while changed:
        changed = False
        for phi in phis:
            if phi.result in values or not phi.incoming or not all(value in values for value in phi.incoming.values()):
                continue
            bounds = [values[value] for value in phi.incoming.values()]
            values[phi.result] = min(low for low, _ in bounds), max(high for _, high in bounds)
            changed = True
        remaining = []
        for at, index, op in pending:
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
                    bounds = _memory(arg, format, memory.get((at, index), {}),
                                     scoped.get(at, {}), definitions)
                if bounds is None:
                    break
                inputs.append(bounds)
            result = evaluated(op.kind, op.floating, tuple(inputs))
            if result is None:
                remaining.append((at, index, op))
                continue
            safe.add(id(op))
            for arg in op.results:
                if isinstance(arg, mir.Held):
                    values[arg.value] = result
            changed = True
        pending = remaining
    return safe
