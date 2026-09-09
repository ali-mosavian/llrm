"""Exact finite numeric facts; strict effects remain a separate obligation."""

from dataclasses import dataclass, replace
from fractions import Fraction

from qbopt.analysis import consts
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


def known(body: mir.MirBody, dgroup: frozenset[int], calls: dict[int, str], *, initial=None) -> dict[mir.Value, Finite]:
    """Numeric facts, optionally given independently established entry bytes."""
    integers = consts.known(body, dgroup, calls)
    seed = {(addr, 1): consts.Known(byte, 1) for addr, byte in (initial or {}).items()}
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
                if op.floating is None or len(op.args) != len(op.floating.inputs):
                    continue
                inputs = []
                for arg, format in zip(op.args, op.floating.inputs):
                    if isinstance(arg, mir.Held) and arg.width == 10:
                        fact = facts.get(arg.value)
                    else:
                        bits = consts._operand(op, arg, integers, memory.get((block.at, index), {}))
                        fact = decoded(bits.n, format) if bits is not None else None
                    if fact is None:
                        break
                    inputs.append(fact)
                else:
                    result = evaluated(op.kind, op.floating, tuple(inputs))
                    if result is not None:
                        for target in op.results:
                            if isinstance(target, mir.Held) and target.width == 10 and target.value not in facts:
                                facts[target.value] = result
                                changed = True
    return facts
