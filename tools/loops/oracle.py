"""
What a case computes, in each language's own semantics, with no compiler.

The languages differ only where one leaves a thing undefined or an error:
- C: signed overflow at int width or wider is undefined; narrower types
  compute in int and wrap on conversion back.
- BASIC: any INTEGER or LONG overflow is error 6, the FOR increment
  included; FOR evaluates its limit and step once.
- Nib: every integer wraps.
An input that meets one of those is not valid in that language (`Invalid`);
the driver leaves it out there. Anything else that goes wrong is the
generator's fault (`Broken`).

A run reports, per input: the result, a digest of each array the case may
write, and the `tick` count when the case calls it. `digest` is chosen so
BASIC computes it in LONG without overflow.
"""

from __future__ import annotations

from spec import (
    Int, Float, Struct, Ptr, Case, Input, Fill, Expr, Const, Var, Bin, Neg, Cast, Cmp, Not, Logic, Load,
    AddrOf, PtrAdd, Deref, Len, CallE, Assign, For, While, DoWhile, If, Break, Continue, Return, CallS,
    written_arrays, walk,
)

LANGS = ("c", "bas", "nib")
FUEL = 60_000_000
MODULUS = 65521


class Invalid(Exception):
    """The language leaves this input undefined, or stops it with an error."""


class Broken(Exception):
    """The case itself is wrong: an index out of bounds, a loop that never ends."""


# --- data -----------------------------------------------------------------


def fill_x(k: int, seed: int) -> int:
    return (k * 7919 + seed) % MODULUS


def fill_value(elem: Int | Float, k: int, fill: Fill, field: int = 0) -> int:
    if fill.step:
        return fill.lo + k * fill.step
    seed = fill.seed + 101 * field
    x = fill_x(k, seed)
    if fill.span:
        return fill.lo + x % fill.span
    if isinstance(elem, Float):
        return x - 32760
    if elem.bits == 32:
        x2 = fill_x(k, seed + 17)
        value = (x - 32760) * 65536 + x2
        return value if elem.signed else value & 0xFFFFFFFF
    return elem.wrap(x) if elem.bits == 16 else elem.wrap(x & 0xFF)


def chunks(elem: Int | Float, value: int) -> list[int]:
    if isinstance(elem, Float) or elem.bits == 32:
        value &= 0xFFFFFFFF
        return [value & 0xFFFF, value >> 16]
    return [value & (0xFF if elem.bits == 8 else 0xFFFF)]


def digest(elem, values: list) -> int:
    d = 0
    for one in values:
        parts = [one] if not isinstance(elem, Struct) else [one[name] for name, _ in elem.fields]
        types = [elem] if not isinstance(elem, Struct) else [t for _, t in elem.fields]
        for kind, value in zip(types, parts):
            for chunk in chunks(kind, value):
                d = (d * 31 + chunk) % MODULUS
    return d


def filled(array, fill: Fill) -> list:
    if isinstance(array.elem, Struct):
        return [
            {name: fill_value(kind, k, fill, f) for f, (name, kind) in enumerate(array.elem.fields)}
            for k in range(array.count)
        ]
    return [fill_value(array.elem, k, fill) for k in range(array.count)]


# --- arithmetic --------------------------------------------------------------


def _fits(kind, value: int, lang: str) -> int:
    """`value` as `kind` holds it after an operation computed exactly."""
    if isinstance(kind, Float):
        limit = 1 << (24 if kind.bits == 32 else 53)
        if abs(value) > limit:
            raise Invalid(f"{value} is not exact in {kind.name}")
        return value
    if kind.lo <= value <= kind.hi:
        return value
    if not kind.signed or lang == "nib" or (lang == "c" and kind.bits < 16):
        return kind.wrap(value)
    raise Invalid(f"{kind.name} overflow: {value}" + (" (error 6)" if lang == "bas" else ""))


def _divide(a: int, b: int) -> int:
    if b == 0:
        raise Invalid("division by zero")
    q = abs(a) // abs(b)
    return q if (a >= 0) == (b >= 0) else -q


def arith(op: str, kind, a: int, b: int, lang: str) -> int:
    if op == "+":
        return _fits(kind, a + b, lang)
    if op == "-":
        return _fits(kind, a - b, lang)
    if op == "*":
        return _fits(kind, a * b, lang)
    if op == "/":
        return _fits(kind, _divide(a, b), lang)
    if op == "%":
        return _fits(kind, a - _divide(a, b) * b, lang)
    if op == "&":
        return kind.wrap(a & b)
    if op == "|":
        return kind.wrap(a | b)
    if op == "^":
        return kind.wrap(a ^ b)
    if op == "min":
        return min(a, b)
    if op == "max":
        return max(a, b)
    if op in ("<<", ">>"):
        if not 0 <= b < kind.bits:
            raise Invalid(f"shift by {b}")
        if op == ">>":
            return a >> b
        if lang == "c" and kind.signed and kind.bits >= 16 and (a < 0 or a << b > kind.hi):
            raise Invalid("signed left shift out of range")
        return kind.wrap(a << b)
    raise Broken(f"no operator {op}")


def convert(value: int, source, target, lang: str) -> int:
    if isinstance(target, Float):
        return _fits(target, value, lang)
    if isinstance(source, Float) or lang == "bas":
        if not target.lo <= value <= target.hi:
            raise Invalid(f"{value} does not convert to {target.name}")
        return value
    return target.wrap(value)


# --- evaluation --------------------------------------------------------------


class _Break(Exception):
    pass


class _Continue(Exception):
    pass


class _Return(Exception):
    def __init__(self, value):
        self.value = value


class Machine:
    def __init__(self, case: Case, lang: str):
        self.case = case
        self.lang = lang
        self.types = case.types()
        self.fuel = FUEL

    # types

    def type_of(self, expr: Expr):
        if isinstance(expr, Const):
            return expr.type
        if isinstance(expr, Var):
            return self.types[expr.name]
        if isinstance(expr, (Bin, Neg)):
            return self.type_of(expr.a)
        if isinstance(expr, Cast):
            return expr.type
        if isinstance(expr, (Cmp, Not, Logic)):
            return None
        if isinstance(expr, Load):
            elem = self.case.array(expr.array).elem
            return elem.field(expr.field) if expr.field else elem
        if isinstance(expr, AddrOf):
            array = self.case.array(expr.array)
            return Ptr(array.elem, array.ptr)
        if isinstance(expr, PtrAdd):
            return self.type_of(expr.ptr)
        if isinstance(expr, Deref):
            elem = self.type_of(expr.ptr).elem
            return elem.field(expr.field) if expr.field else elem
        if isinstance(expr, Len):
            return Int(16, True)
        if isinstance(expr, CallE):
            from spec import OPAQUE

            return OPAQUE[expr.name]
        raise Broken(f"no type for {expr}")

    # running

    def run(self, inp: Input) -> list[int]:
        case = self.case
        fills = dict(inp.fills)
        self.memory = {}
        for array in case.arrays:
            if array.name not in fills:
                raise Broken(f"{case.name}: no fill for {array.name}")
            self.memory[array.name] = filled(array, fills[array.name])
        for name, at, value in inp.pokes:
            if isinstance(self.case.array(name).elem, Struct):
                raise Broken("a poke into a struct array")
            self.memory[name][at] = value
        self.env = {}
        for (name, kind), value in zip(case.params, inp.args):
            if isinstance(kind, Int) and not kind.lo <= value <= kind.hi:
                raise Broken(f"{case.name}: argument {name}={value} is not a {kind.name}")
            self.env[name] = value
        for name, kind in case.locals:
            self.env[name] = None
        self.ticks = 0
        try:
            self.block(case.body)
            raise Broken(f"{case.name}: the function ends without a return")
        except _Return as done:
            result = done.value
        out = [Int(32, True).wrap(result)]
        for name in written_arrays(case):
            array = case.array(name)
            out.append(digest(array.elem, self.memory[name]))
        if any(isinstance(one, CallS) and one.name == "tick" for one in walk(case.body)):
            out.append(self.ticks)
        return out

    def block(self, stmts):
        for one in stmts:
            self.stmt(one)

    def burn(self):
        self.fuel -= 1
        if self.fuel < 0:
            raise Broken(f"{self.case.name}: out of fuel")

    def stmt(self, s):
        self.burn()
        if isinstance(s, Assign):
            value = self.eval(s.value)
            self.store(s.place, value)
        elif isinstance(s, For):
            self.run_for(s)
        elif isinstance(s, While):
            while self.eval(s.cond):
                self.burn()
                try:
                    self.block(s.body)
                except _Break:
                    break
                except _Continue:
                    pass
        elif isinstance(s, DoWhile):
            while True:
                self.burn()
                try:
                    self.block(s.body)
                except _Break:
                    break
                except _Continue:
                    pass
                if not self.eval(s.cond):
                    break
        elif isinstance(s, If):
            self.block(s.then if self.eval(s.cond) else s.orelse)
        elif isinstance(s, Break):
            raise _Break()
        elif isinstance(s, Continue):
            raise _Continue()
        elif isinstance(s, Return):
            raise _Return(self.convert_to(self.eval(s.value), self.type_of(s.value), self.case.ret))
        elif isinstance(s, CallS):
            self.call(s.name, [self.eval(one) for one in s.args])
        else:
            raise Broken(f"no statement {s}")

    def convert_to(self, value, source, target):
        return value if source == target else convert(value, source, target, self.lang)

    def run_for(self, s: For):
        kind = self.types[s.var]
        start = self.eval(s.start)
        end = self.eval(s.end)
        step = self.eval(s.step)
        self.env[s.var] = self.convert_to(start, self.type_of(s.start), kind)
        if self.lang == "bas" and s.cond != "!=":
            # FOR v = start TO limit STEP step, evaluated once.
            limit = {"<": lambda: arith("-", kind, end, 1, "bas"), ">": lambda: arith("+", kind, end, 1, "bas")}.get(
                s.cond, lambda: end
            )()
            if (step > 0) != (s.cond in ("<", "<=")) or step == 0:
                raise Invalid("FOR's direction is its step's sign, not the condition's")
            test = (lambda value: value <= limit) if step > 0 else (lambda value: value >= limit)
        else:
            test = lambda value: compare(s.cond, value, self.eval(s.end))
        while test(self.env[s.var]):
            self.burn()
            try:
                self.block(s.body)
            except _Break:
                return
            except _Continue:
                pass
            if self.lang != "bas" and self.eval(s.end) != end:
                raise Broken(f"{self.case.name}: a For's end changed in its body")
            self.env[s.var] = arith("+", kind, self.env[s.var], step, self.lang)

    def call(self, name: str, args: list[int]):
        if name in ("keep", "keep32"):
            return args[0]
        if name == "touch":
            return None
        if name == "tick":
            self.ticks += 1
            return None
        raise Broken(f"no external {name}")

    def element(self, array: str, index: int):
        values = self.memory[array]
        if not 0 <= index < len(values):
            raise Broken(f"{self.case.name}: {array}[{index}] is out of bounds")
        return values

    def linear(self, array: str, indices: list[int]) -> int:
        dims = self.case.array(array).dims
        if len(indices) != len(dims):
            raise Broken(f"{array} has {len(dims)} dimensions")
        at = 0
        for index, extent in zip(indices, dims):
            if not 0 <= index < extent:
                raise Broken(f"{self.case.name}: {array}{indices} is out of bounds")
            at = at * extent + index
        return at

    def store(self, place, value):
        if isinstance(place, Var):
            kind = self.types[place.name]
            self.env[place.name] = value
            return
        if isinstance(place, Load):
            at = self.linear(place.array, [self.eval(one) for one in place.index])
            self._put(place.array, at, place.field, value)
            return
        if isinstance(place, Deref):
            array, at = self.eval(place.ptr)
            at += self.eval(place.at)
            self.element(array, at)
            self._put(array, at, place.field, value)
            return
        raise Broken(f"not a place: {place}")

    def _put(self, array, at, field, value):
        values = self.element(array, at)
        if field:
            values[at] = dict(values[at]) | {field: value}
        else:
            values[at] = value

    def _get(self, array, at, field):
        value = self.element(array, at)[at]
        return value[field] if field else value

    def eval(self, e):
        if isinstance(e, Const):
            return e.value
        if isinstance(e, Var):
            value = self.env[e.name]
            if value is None:
                raise Broken(f"{self.case.name}: {e.name} is read before it is set")
            return value
        if isinstance(e, Bin):
            kind = self.type_of(e.a)
            return arith(e.op, kind, self.eval(e.a), self.eval(e.b), self.lang)
        if isinstance(e, Neg):
            return _fits(self.type_of(e.a), -self.eval(e.a), self.lang)
        if isinstance(e, Cast):
            return self.convert_to(self.eval(e.a), self.type_of(e.a), e.type)
        if isinstance(e, Cmp):
            return compare(e.op, self.eval(e.a), self.eval(e.b))
        if isinstance(e, Not):
            return not self.eval(e.a)
        if isinstance(e, Logic):
            if e.op == "and":
                return bool(self.eval(e.a)) and bool(self.eval(e.b))
            return bool(self.eval(e.a)) or bool(self.eval(e.b))
        if isinstance(e, Load):
            at = self.linear(e.array, [self.eval(one) for one in e.index])
            return self._get(e.array, at, e.field)
        if isinstance(e, AddrOf):
            return (e.array, self.eval(e.index))
        if isinstance(e, PtrAdd):
            array, at = self.eval(e.ptr)
            return (array, at + self.eval(e.by))
        if isinstance(e, Deref):
            array, at = self.eval(e.ptr)
            return self._get(array, at + self.eval(e.at), e.field)
        if isinstance(e, Len):
            return self.case.array(e.array).dims[e.dim]
        if isinstance(e, CallE):
            return self.call(e.name, [self.eval(one) for one in e.args])
        raise Broken(f"no expression {e}")


def compare(op: str, a, b) -> bool:
    if isinstance(a, tuple):
        if a[0] != b[0]:
            raise Broken("pointers into different arrays compared")
        a, b = a[1], b[1]
    return {"<": a < b, "<=": a <= b, ">": a > b, ">=": a >= b, "==": a == b, "!=": a != b}[op]


def evaluate(case: Case, lang: str) -> list[list[int] | str]:
    """Per input, the reports, or why the language leaves it undefined."""
    machine = Machine(case, lang)
    out = []
    for inp in case.inputs:
        try:
            out.append(machine.run(inp))
        except Invalid as why:
            out.append(str(why))
        machine.fuel = FUEL
    return out
