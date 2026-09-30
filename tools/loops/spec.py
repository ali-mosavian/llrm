"""
One loop program, written once, emitted as C, QuickBASIC and Nib.

A case is a function over scalar parameters and arrays, plus the inputs it is
called on and what its inner loop should cost. Every operation carries its
type; emitters insert the conversions each language needs so all three mean
what the oracle (oracle.py) computes.
"""

from __future__ import annotations

from dataclasses import dataclass, field, replace


# --- types -----------------------------------------------------------------


@dataclass(frozen=True)
class Int:
    bits: int
    signed: bool

    @property
    def size(self) -> int:
        return self.bits // 8

    @property
    def lo(self) -> int:
        return -(1 << (self.bits - 1)) if self.signed else 0

    @property
    def hi(self) -> int:
        return (1 << (self.bits - 1)) - 1 if self.signed else (1 << self.bits) - 1

    @property
    def name(self) -> str:
        return f"{'i' if self.signed else 'u'}{self.bits}"

    def wrap(self, value: int) -> int:
        value &= (1 << self.bits) - 1
        return value - (1 << self.bits) if self.signed and value > self.hi else value


@dataclass(frozen=True)
class Float:
    """Holds integral values only, so every language's float agrees."""

    bits: int  # 32, 64, 80

    @property
    def size(self) -> int:
        return self.bits // 8

    @property
    def name(self) -> str:
        return f"f{self.bits}"

    signed = True


@dataclass(frozen=True)
class Struct:
    name: str
    fields: tuple[tuple[str, Int | Float], ...]

    @property
    def size(self) -> int:
        return sum(one.size for _, one in self.fields)

    def field(self, name: str) -> Int | Float:
        return dict(self.fields)[name]


@dataclass(frozen=True)
class Ptr:
    elem: Int | Float | Struct
    kind: str = "near"  # near, far, huge

    @property
    def name(self) -> str:
        return f"*{self.kind} {self.elem.name}"


Scalar = Int | Float
Type = Int | Float | Struct | Ptr

I8, U8, I16, U16, I32, U32 = Int(8, True), Int(8, False), Int(16, True), Int(16, False), Int(32, True), Int(32, False)
F32, F64, F80 = Float(32), Float(64), Float(80)
INTS = (I8, U8, I16, U16, I32, U32)


# --- storage ---------------------------------------------------------------


@dataclass(frozen=True)
class Array:
    """`dims` are the allocated extents. `where`: param, global or local.
    `ptr` is how a C or Nib parameter reaches it: near, far or huge."""

    name: str
    elem: Int | Float | Struct
    dims: tuple[int, ...]
    where: str = "param"
    ptr: str = "near"
    # a parameter bound to another array's storage: the two alias
    alias: str | None = None

    @property
    def count(self) -> int:
        total = 1
        for one in self.dims:
            total *= one
        return total

    @property
    def bytes(self) -> int:
        return self.count * self.elem.size


# --- expressions -----------------------------------------------------------


class Expr:
    pass


@dataclass(frozen=True)
class Const(Expr):
    value: int
    type: Scalar


@dataclass(frozen=True)
class Var(Expr):
    name: str


@dataclass(frozen=True)
class Bin(Expr):
    """+ - * / % & | ^ << >> min max; both operands of one type (shift counts
    excepted)."""

    op: str
    a: Expr
    b: Expr


@dataclass(frozen=True)
class Neg(Expr):
    a: Expr


@dataclass(frozen=True)
class Cast(Expr):
    a: Expr
    type: Scalar


@dataclass(frozen=True)
class Cmp(Expr):
    op: str  # < <= > >= == !=
    a: Expr
    b: Expr


@dataclass(frozen=True)
class Not(Expr):
    a: Expr


@dataclass(frozen=True)
class Logic(Expr):
    op: str  # and, or
    a: Expr
    b: Expr


@dataclass(frozen=True)
class Load(Expr):
    """An element, or one field of it; also a place."""

    array: str
    index: tuple[Expr, ...]
    field: str | None = None


@dataclass(frozen=True)
class AddrOf(Expr):
    """&array[index], a pointer to one element."""

    array: str
    index: Expr


@dataclass(frozen=True)
class PtrAdd(Expr):
    ptr: Expr
    by: Expr  # elements


@dataclass(frozen=True)
class Deref(Expr):
    """ptr[at], or one field of it; also a place."""

    ptr: Expr
    at: Expr
    field: str | None = None


@dataclass(frozen=True)
class Len(Expr):
    """An array's allocated extent along one dimension, as the language knows it."""

    array: str
    dim: int = 0


@dataclass(frozen=True)
class CallE(Expr):
    """An opaque external call (see OPAQUE)."""

    name: str
    args: tuple[Expr, ...]


# The external functions a case may call. The compiler cannot see into them;
# the oracle, the DOS runtime and the MIR stub all implement them alike.
# `keep` returns its argument; `touch` does nothing; `tick` counts calls in a
# global the driver reports.
OPAQUE = {"keep": I16, "keep32": I32, "touch": None, "tick": None}


# --- statements ------------------------------------------------------------


class Stmt:
    pass


@dataclass(frozen=True)
class Assign(Stmt):
    place: Expr  # Var, Load or Deref
    value: Expr


@dataclass(frozen=True)
class For(Stmt):
    """C's `for (v = start; v cond end; v += step)`. `end` and `step` are
    loop-invariant and the body leaves `var` alone, so BASIC's FOR and Nib's
    range may carry it."""

    var: str
    start: Expr
    cond: str
    end: Expr
    step: Expr
    body: tuple[Stmt, ...]


@dataclass(frozen=True)
class While(Stmt):
    cond: Expr
    body: tuple[Stmt, ...]


@dataclass(frozen=True)
class DoWhile(Stmt):
    body: tuple[Stmt, ...]
    cond: Expr


@dataclass(frozen=True)
class If(Stmt):
    cond: Expr
    then: tuple[Stmt, ...]
    orelse: tuple[Stmt, ...] = ()


@dataclass(frozen=True)
class Break(Stmt):
    pass


@dataclass(frozen=True)
class Continue(Stmt):
    pass


@dataclass(frozen=True)
class Return(Stmt):
    value: Expr


@dataclass(frozen=True)
class CallS(Stmt):
    name: str
    args: tuple[Expr, ...] = ()


# --- a case ----------------------------------------------------------------


@dataclass(frozen=True)
class Fill:
    """An array's contents before a call: element k (row-major logical order)
    of field f is lo + x mod span, x = (k*7919 + seed + 101*f) mod 65521.
    span 0 takes the type's whole range (see oracle.fill_value); a nonzero
    step makes element k lo + k*step."""

    seed: int
    lo: int = 0
    span: int = 0
    # nonzero: element k is lo + k*step instead, sorted data
    step: int = 0


@dataclass(frozen=True)
class Input:
    args: tuple[int, ...]  # the scalar parameters, in order
    fills: tuple[tuple[str, Fill], ...] = ()
    # after the fills: (array, linear element, value), e.g. a terminator
    pokes: tuple[tuple[str, int, int], ...] = ()


@dataclass(frozen=True)
class Expect:
    """Derived by hand in the generator, with no compiler involved.

    ivs: the most induction variables the inner loop needs.
    invariant_loads: the most loads of loop-invariant values it may keep
      (0 unless the loop's own stores may alias them or registers run out).
    fits: whether the loop's live values fit the target's registers, so the
      two above apply; past it only correctness and the reference bar do.
    """

    ivs: int | None = None
    invariant_loads: int | None = 0
    fits: bool = True


@dataclass(frozen=True)
class Case:
    name: str
    family: str
    params: tuple[tuple[str, Scalar | Ptr], ...]
    arrays: tuple[Array, ...]
    locals: tuple[tuple[str, Scalar | Ptr], ...]
    body: tuple[Stmt, ...]
    ret: Int
    inputs: tuple[Input, ...]
    expect: Expect = Expect()
    tags: frozenset[str] = frozenset()
    # a metamorphic variant names its base case and what it changed
    base: str | None = None
    relation: str | None = None
    # per language, why it cannot be written there
    skip: tuple[tuple[str, str], ...] = ()
    # per language, a stated induction-variable bound replacing expect.py's,
    # with why (e.g. bp released as a seventh register)
    bound: tuple[tuple[str, int, str], ...] = ()
    note: str = ""

    def array(self, name: str) -> Array:
        return next(one for one in self.arrays if one.name == name)

    def types(self) -> dict[str, Type]:
        return dict(self.params) | dict(self.locals)

    def with_(self, **changes) -> "Case":
        return replace(self, **changes)

    @property
    def symbol(self) -> str:
        """The function's name in C and Nib: never a library's."""
        return "f_" + self.name.lower()


# --- helpers for writing cases -----------------------------------------------


def c(value: int, type: Scalar = I16) -> Const:
    return Const(value, type)


def v(name: str) -> Var:
    return Var(name)


def add(a: Expr, b: Expr) -> Bin:
    return Bin("+", a, b)


def sub(a: Expr, b: Expr) -> Bin:
    return Bin("-", a, b)


def mul(a: Expr, b: Expr) -> Bin:
    return Bin("*", a, b)


def ld(array: str, *index: Expr, field: str | None = None) -> Load:
    return Load(array, tuple(index), field)


def walk(stmts) -> list:
    """Every statement, nested ones included, in source order."""
    out = []
    for one in stmts:
        out.append(one)
        for inner in _children(one):
            out.extend(walk(inner))
    return out


def _children(stmt: Stmt) -> list[tuple[Stmt, ...]]:
    if isinstance(stmt, (For, While)):
        return [stmt.body]
    if isinstance(stmt, DoWhile):
        return [stmt.body]
    if isinstance(stmt, If):
        return [stmt.then, stmt.orelse]
    return []


def exprs(expr: Expr) -> list[Expr]:
    """Every subexpression, `expr` first."""
    out = [expr]
    for name in getattr(expr, "__dataclass_fields__", {}):
        value = getattr(expr, name)
        if isinstance(value, Expr):
            out.extend(exprs(value))
        elif isinstance(value, tuple):
            for one in value:
                if isinstance(one, Expr):
                    out.extend(exprs(one))
    return out


def stmt_exprs(stmt: Stmt) -> list[Expr]:
    found = []
    for name in stmt.__dataclass_fields__:
        value = getattr(stmt, name)
        if isinstance(value, Expr):
            found.extend(exprs(value))
    return found


def written_arrays(case: Case) -> list[str]:
    """Arrays the case may store to, directly or through a pointer."""
    names = set()
    for one in walk(case.body):
        if isinstance(one, Assign):
            if isinstance(one.place, Load):
                names.add(one.place.array)
            elif isinstance(one.place, Deref):
                names.update(pointed(case, one.place.ptr))
    return [a.name for a in case.arrays if a.name in names]


def pointed(case: Case, expr: Expr) -> set[str]:
    """The arrays a pointer expression may point into."""
    found = set()
    for one in exprs(expr):
        if isinstance(one, AddrOf):
            found.add(one.array)
        elif isinstance(one, Var) and isinstance(case.types().get(one.name), Ptr):
            for stmt in walk(case.body):
                if isinstance(stmt, Assign) and stmt.place == one:
                    found |= {x.array for x in exprs(stmt.value) if isinstance(x, AddrOf)}
    return found


def backing(case: Case, array: Array) -> Array:
    """The array whose storage `array` uses."""
    return case.array(array.alias) if array.alias else array
