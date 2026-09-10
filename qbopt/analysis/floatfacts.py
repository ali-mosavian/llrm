"""Exact finite numeric facts; strict effects remain a separate obligation."""

from dataclasses import dataclass, replace
from fractions import Fraction

from qbopt.analysis import consts, induction, loops
from qbopt.model import mir
from qbopt.model.floating import Format, Precision, Semantics


@dataclass(frozen=True, slots=True)
class Finite:
    value: Fraction
    negative_zero: bool = False

    @property
    def negative(self) -> bool:
        return self.value < 0 or self.negative_zero


_BINARY = {Format.BINARY32: (24, 8, 127), Format.BINARY64: (53, 11, 1023)}
_INTEGER = {Format.SIGNED16: 16, Format.SIGNED32: 32, Format.SIGNED64: 64}


def decoded(bits: int, format: Format) -> Finite | None:
    if format in _INTEGER:
        width = _INTEGER[format]
        if not 0 <= bits < 1 << width:
            return None
        sign = 1 << (width - 1)
        return Finite(Fraction((bits ^ sign) - sign))
    if format not in _BINARY:
        return None
    precision, exponent_bits, bias = _BINARY[format]
    if not 0 <= bits < 1 << (precision + exponent_bits):
        return None
    fraction = bits & ((1 << (precision - 1)) - 1)
    exponent = (bits >> (precision - 1)) & ((1 << exponent_bits) - 1)
    negative = bool(bits >> (precision + exponent_bits - 1))
    if exponent == (1 << exponent_bits) - 1 or (exponent == 0 and fraction):
        return None
    if exponent == 0:
        return Finite(Fraction(0), negative)
    significand = (1 << (precision - 1)) | fraction
    shift = exponent - bias - precision + 1
    value = Fraction(significand << max(shift, 0), 1 << max(-shift, 0))
    return Finite(-value if negative else value)


def _fits(value: Fraction, precision: int, minimum: int, maximum: int) -> bool:
    if not value:
        return True
    numerator, denominator = abs(value.numerator), value.denominator
    if denominator & (denominator - 1):
        return False
    trailing = (numerator & -numerator).bit_length() - 1
    exponent = numerator.bit_length() - denominator.bit_length()
    return (numerator.bit_length() - trailing <= precision
            and minimum <= exponent <= maximum)


def evaluated(kind: mir.Kind, rule: Semantics, inputs: tuple[Finite, ...]) -> Finite | None:
    if len(inputs) != len(rule.inputs):
        return None
    match kind, inputs:
        case mir.Kind.FLOAD | mir.Kind.FSTORE, (value,):
            result = value
        case mir.Kind.FNEG, (value,):
            result = Finite(-value.value, not value.negative_zero if not value.value else False)
        case mir.Kind.FABS, (value,):
            result = Finite(abs(value.value))
        case mir.Kind.FADD | mir.Kind.FSUB, (left, right):
            right_value = right.value if kind is mir.Kind.FADD else -right.value
            value = left.value + right_value
            if not value:
                right_negative = right.negative ^ (kind is mir.Kind.FSUB)
                if left.value or right.value or left.negative != right_negative:
                    return None  # cancellation's zero sign depends on rounding
                result = Finite(value, left.negative)
            else:
                result = Finite(value)
        case mir.Kind.FMUL | mir.Kind.FDIV, (left, right):
            if kind is mir.Kind.FDIV and not right.value:
                return None
            value = left.value * right.value if kind is mir.Kind.FMUL else left.value / right.value
            result = Finite(value, not value and left.negative != right.negative)
        case _:
            return None
    if rule.result in _INTEGER:
        width = _INTEGER[rule.result]
        return result if result.value.denominator == 1 and -(1 << (width - 1)) <= result.value < 1 << (width - 1) else None
    if rule.result in _BINARY:
        precision, _, bias = _BINARY[rule.result]
        return result if _fits(result.value, precision, 1 - bias, bias) else None
    if rule.result is Format.EXTENDED80:
        precision = 24 if rule.precision is Precision.DYNAMIC else 64
        return result if _fits(result.value, precision, -16382, 16383) else None
    return None


def encoded(value: Finite, format: Format) -> int | None:
    if format not in _BINARY:
        return None
    precision, exponent_bits, bias = _BINARY[format]
    if not _fits(value.value, precision, 1 - bias, bias):
        return None
    sign = int(value.negative) << (precision + exponent_bits - 1)
    if not value.value:
        return sign
    magnitude = abs(value.value)
    exponent = magnitude.numerator.bit_length() - magnitude.denominator.bit_length()
    shift = precision - 1 - exponent
    significand = magnitude * Fraction(1 << max(shift, 0), 1 << max(-shift, 0))
    return sign | ((exponent + bias) << (precision - 1)) | (int(significand) - (1 << (precision - 1)))


def _inputs(op, integers, memory, facts):
    if op.floating is None or len(op.args) != len(op.floating.inputs):
        return None
    inputs = []
    for arg, format in zip(op.args, op.floating.inputs):
        if isinstance(arg, mir.Held) and arg.width == 10:
            fact = facts.get(arg.value)
        else:
            bits = consts._operand(op, arg, integers, memory)
            fact = decoded(bits.n, format) if bits is not None else None
        if fact is None:
            return None
        inputs.append(fact)
    return tuple(inputs)


def checkpoint(op: mir.Op) -> bool:
    """An explicit FP exception check with no additional value or memory effect."""
    return (op.kind is mir.Kind.FCHECK and not op.barrier and op.floating is None
            and not (op.defines or op.uses or op.loads or op.stores or op.merges)
            and op.stack is None)


def repeated(ops: tuple[mir.Op, ...], count: int, initial: consts.Cells,
             dgroup: frozenset[int], known: dict | None = None) -> consts.Cells | None:
    """Exact memory facts after a caller-proven repetition of a straight-line body.

    Each storage conversion is evaluated in order on every iteration. This
    proves numeric exits only, not that the loop's effects may be removed.
    Unknown values, inexact conversions and unmodelled control flow refuse.
    """
    if count < 0 or count * len(ops) > 100_000:
        return None
    internal = {value for op in ops for value in op.defines}
    invariant = {value: fact for value, fact in (known or {}).items() if value not in internal}
    memory = dict(initial)
    allowed = {mir.Kind.NOTHING, mir.Kind.COPY, mir.Kind.ADD, mir.Kind.SUB,
               mir.Kind.INCREMENT, mir.Kind.DECREMENT}
    for _ in range(count):
        integers = dict(invariant)
        floating = {}
        for op in ops:
            if checkpoint(op):
                continue  # exact operations add no pending exception; the check is retained by specialization
            if op.barrier or op.merges:
                return None
            if op.floating is None:
                if op.kind not in allowed or op.loads or op.stores or op.stack is not None:
                    return None
                result = consts._result(op, integers, here=memory)
                if result is not None:
                    for value in op.defines:
                        if not value.flags:
                            integers[value] = result
                continue
            inputs = _inputs(op, integers, memory, floating)
            result = evaluated(op.kind, op.floating, inputs) if inputs is not None else None
            if result is None:
                return None
            if op.kind is mir.Kind.FSTORE:
                if len(op.stores) != 1:
                    return None
                ref = mir._symbolic_ref(op.stores[0])
                bits = encoded(result, op.floating.result)
                if bits is None or ref.addr is None or ref.base is not None or ref.segment is not None:
                    return None
                store = replace(op, kind=mir.Kind.STORE, args=(mir.Const(bits, ref.width),),
                                stores=(ref,), uses=())
                memory = consts._kills(memory, store, integers, dgroup, {})
            else:
                if op.stores or len(op.results) != 1 or not isinstance(op.results[0], mir.Held):
                    return None
                floating[op.results[0].value] = result
    return memory


@dataclass(frozen=True, slots=True)
class LoopExit:
    header: int
    count: int
    stores: tuple[tuple[mir.MemRef, consts.Known], ...]


def loop_exits(body: mir.MirBody, dgroup: frozenset[int], calls: dict) -> tuple[LoopExit, ...]:
    """Proven numeric exits of canonical loops with storage-rounded FP state."""
    if not any(op.kind is mir.Kind.FSTORE for block in body.blocks for op in block.ops):
        return ()
    integers = consts.known(body, dgroup, calls)
    memory = cells(body, dgroup, calls)
    blocks = {block.at: block for block in body.blocks}
    predecessors = loops.predecessors(body.blocks)
    exits = []
    for loop in loops.loops(body.blocks, body.entry):
        if len(loop.body) != 2 or len(loop.latches) != 1:
            continue
        header = blocks[loop.header]
        latch = blocks[next(iter(loop.latches))]
        outside = set(predecessors.get(header.at, ())) - set(loop.body)
        if len(outside) != 1 or latch.phis:
            continue
        entry = blocks[outside.pop()]
        if entry.succ != (header.at,) or not entry.ops:
            continue
        references = tuple(ref for op in latch.ops for ref in (*op.loads, *op.stores))
        stored = tuple(dict.fromkeys(ref for op in latch.ops if op.kind is mir.Kind.FSTORE for ref in op.stores))
        if not stored or any(
            op.barrier or op.kind not in {mir.Kind.NOTHING, mir.Kind.COPY, mir.Kind.STORE,
                                          mir.Kind.SUB, mir.Kind.BRANCH}
            or op.floating is not None or op.stack is not None
            or any(mir.overlapping(written, read, dgroup) for written in op.stores for read in references)
            for op in header.ops
        ):
            continue
        counts = set()
        for counter in induction.basics(body, loop).values():
            width = counter.start.width
            last = induction._last_counter(body, loop, counter, integers, width)
            start = induction._signed(counter.start, integers, width)
            step = induction._signed(counter.step, integers, width)
            if last is not None and start is not None and step:
                counts.add((last - start) // step + 1)
        if len(counts) != 1:
            continue
        count = counts.pop()
        initial = consts._kills(memory[entry.at, len(entry.ops) - 1], entry.ops[-1], integers, dgroup, calls)
        final = repeated(latch.ops, count, initial, dgroup, integers)
        if final is None:
            continue
        facts = tuple((ref, consts._cell(final, ref)) for ref in stored)
        if all(fact is not None for _, fact in facts):
            exits.append(LoopExit(header.at, count, facts))
    return tuple(exits)


def exit_cells(body: mir.MirBody, dgroup: frozenset[int], calls: dict) -> dict[tuple[int, int], consts.Cells]:
    """Numeric memory facts on exit edges, never on a header's backedge.

    These facts describe executions which reach the exit. The strict
    operations establishing them remain in place, including their checks.
    """
    proofs = loop_exits(body, dgroup, calls)
    if not proofs:
        return {}
    regions = {loop.header: loop.body for loop in loops.loops(body.blocks, body.entry)}
    blocks = {block.at: block for block in body.blocks}
    edges = {}
    for proof in proofs:
        destination, = [at for at in blocks[proof.header].succ if at not in regions[proof.header]]
        memory = {}
        for ref, fact in proof.stores:
            memory.update(consts._fragments(mir._symbolic_ref(ref), fact))
        edges[proof.header, destination] = memory
    return edges


def known(body: mir.MirBody, dgroup: frozenset[int], calls: dict[int, str], *, initial=None) -> dict[mir.Value, Finite]:
    """Numeric facts, optionally given independently established entry bytes."""
    return _analyzed(body, dgroup, calls, initial)[0]


def converted(body: mir.MirBody, dgroup: frozenset[int], calls: dict[int, str], *, facts=None) -> dict[mir.Value, consts.Known]:
    """Exact integer conversion results, without permission to remove FP effects."""
    if not any(op.kind is mir.Kind.FSTORE and op.results and not op.stores
               for block in body.blocks for op in block.ops):
        return {}
    facts = known(body, dgroup, calls) if facts is None else facts
    results = {}
    for block in body.blocks:
        for op in block.ops:
            if (op.kind is not mir.Kind.FSTORE or op.floating is None or op.stores
                or op.floating.result not in _INTEGER or len(op.args) != 1 or len(op.results) != 1):
                continue
            source, target = op.args[0], op.results[0]
            if (not isinstance(source, mir.Held) or source.value not in facts
                or not isinstance(target, mir.Held) or target.width * 8 != _INTEGER[op.floating.result]):
                continue
            value = evaluated(op.kind, op.floating, (facts[source.value],))
            if value is not None:
                results[target.value] = consts.Known(consts.masked(int(value.value), target.width), target.width)
    return results


def cells(body: mir.MirBody, dgroup: frozenset[int], calls: dict) -> dict:
    """Memory facts including exact floating storage conversions."""
    return _analyzed(body, dgroup, calls, None)[1]


def _analyzed(body, dgroup, calls, initial):
    seed = None if initial is None else {(addr, 1): consts.Known(byte, 1) for addr, byte in initial.items()}
    integers = consts.known(body, dgroup, calls, initial=seed)
    facts: dict[mir.Value, Finite] = {}
    changed = True
    while changed:
        changed = False
        def stored(op):
            if (op.kind is not mir.Kind.FSTORE or op.floating is None or len(op.args) != 1
                or not isinstance(op.args[0], mir.Held) or op.args[0].value not in facts):
                return op
            result = evaluated(op.kind, op.floating, (facts[op.args[0].value],))
            bits = encoded(result, op.floating.result) if result is not None else None
            if bits is None or len(op.stores) != 1:
                return op
            return replace(op, kind=mir.Kind.STORE, args=(mir.Const(bits, op.stores[0].width),), uses=())
        shadow = replace(body, blocks=tuple(replace(block, ops=tuple(map(stored, block.ops))) for block in body.blocks))
        memory = consts.cells(shadow, dgroup, calls, integers, initial=seed)
        for block in body.blocks:
            for index, op in enumerate(block.ops):
                inputs = _inputs(op, integers, memory.get((block.at, index), {}), facts)
                if inputs is not None:
                    result = evaluated(op.kind, op.floating, inputs)
                    if result is not None:
                        for target in op.results:
                            if isinstance(target, mir.Held) and target.width == 10 and target.value not in facts:
                                facts[target.value] = result
                                changed = True
    return facts, memory
