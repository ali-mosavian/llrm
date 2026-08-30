"""
Small, disposable BASIC programs over INTEGER/LONG arithmetic, and the one
reference evaluator that says what each must print -- independent of BC and
of qbopt, built from AGENTS.md's own measured semantics ("What the runtime
does that an instruction does not", "What widening changes, exactly") rather
than from either compiler's source.

Scope, deliberately narrow: scalars only, no arrays/procs/loops/GOTO, no
SINGLE/DOUBLE -- a working oracle over a small grammar beats a broad one
nothing can trust. `IfPrint` is here specifically because nothing else in the
grammar makes a JCC depend on a computed value; every arithmetic result is
otherwise only ever stored or printed, which is not the failure mode a
flag-sensitive widening bug produces.

Both the generator and the golden author call `eval_expr` -- deliberately.
Using it twice only forces internal consistency, which is trivial and proves
nothing; the actual test of whether it is a real oracle is external, run by
tools/fuzzcheck.py: does BC's own independently-compiled, independently-run
build agree with it, checked before qbopt is ever in the loop. The one thing
generation must not do is filter *values* toward what stays in range --
wraparound is exactly the behaviour this exists to exercise (see
`suite/divmod.bas`'s MULOVF and this project's own multiply-is-absorbed-because
-it-wraps reasoning). The only filters below are the divide-by-zero and
LONG/INTEGER MIN-by-(-1) trap cases, which qbopt's bare `idiv` faults on and
BC's runtime does not -- fuzzing that boundary on purpose is suite/divmod.bas's
job, by hand, not this generator's.

One more filter turned out to be load-bearing rather than optional, found by
compiling the first generated batch: BC's *compiler* constant-folds an
arithmetic operator whose both operands are literals, and that fold is
overflow-checked even though the equivalent runtime code is not (measured:
`(-1970530648 * -13199)` alone -- no variables -- fails to compile with "Math
overflow", where the same multiply through a variable, as in
`suite/divmod.bas`'s MULOVF, silently wraps at runtime). Every `BinOp` this
generator builds therefore keeps at least one variable-rooted operand, which
forces BC to emit real code instead of trying to fold it.

A second finding was BC's own, not the evaluator's or qbopt's: `IF <expr>
THEN ... ELSE ...` where NOT appears anywhere in <expr> takes the branch as if
testing <expr>'s own truthiness with the branches swapped, not the arithmetic
NOT's -- confirmed on VBDOS under /O, and it survives NOT being buried a level
deeper (`IF ((NOT v) + 0) THEN` still swaps on v alone), where a bare `PRINT
NOT v` computes the correct two's-complement value every time. Harmless for
NOT used as pure logical negation on a canonical -1/0 value (both readings
agree there, which is presumably why ordinary code never surfaces it) --
`_gen_condition` keeps NOT out of every `IfPrint` condition rather than
chase the exact rule further; written up in AGENTS.md alongside the divide
one, as a BC behavior to know about rather than a qbopt bug.

Known residual gap, left for a future round: "one operand must be
variable-rooted" stops two *directly adjacent* literals from being folded,
but not two literals connected through a longer chain of the same
associative operator with a variable somewhere in between -- observed on a
generated program shaped like `lit1 * (unary_of(lit2) * v)`, which VBDOS/PDS
reject at compile time (their own recovery then misattributes the error to
an unrelated later line, the same way the "--" case did) and QB45 instead
silently miscompiles, continuing execution from a line further down with
wrong values, occasionally reaching a genuine runtime divide-by-zero no
divisor-safety check could have predicted at generation time. Both outcomes
are already handled safely -- the harness's BCFAIL/BASEDIFF exclusion drops
the program before it ever reaches a qbopt comparison -- so this shrinks
the usable sample slightly rather than producing a false finding, but a
tighter fix (detecting a flattened chain of the same commutative operator
and checking that no two literal leaves survive in it) would recover it.
"""

import random
from enum import Enum
from enum import auto
from dataclasses import field
from dataclasses import dataclass

from qbprint import num
from qbprint import s16
from qbprint import s32

ARITH_OPS = ("+", "-", "*", "AND", "OR", "XOR", "\\", "MOD")
CMP_OPS = ("<", "<=", ">", ">=", "=", "<>")
DIVIDING_OPS = ("\\", "MOD")


class Width(Enum):
    INT = auto()
    LNG = auto()


def bounds(width: Width) -> tuple[int, int]:
    return (-32768, 32767) if width is Width.INT else (-2147483648, 2147483647)


def wrap(width: Width, v: int) -> int:
    return s16(v) if width is Width.INT else s32(v)


@dataclass(frozen=True, slots=True)
class Lit:
    width: Width
    value: int


@dataclass(frozen=True, slots=True)
class Var:
    name: str
    width: Width


@dataclass(frozen=True, slots=True)
class BinOp:
    width: Width
    op: str
    left: "Expr"
    right: "Expr"


@dataclass(frozen=True, slots=True)
class UnaryOp:
    width: Width
    op: str  # "-" or "NOT"
    operand: "Expr"


@dataclass(frozen=True, slots=True)
class Cmp:
    op: str
    left: "Expr"
    right: "Expr"


Expr = Lit | Var | BinOp | UnaryOp | Cmp


@dataclass(frozen=True, slots=True)
class Assign:
    name: str
    expr: Expr


@dataclass(frozen=True, slots=True)
class Print:
    tag: str
    expr: Expr


@dataclass(frozen=True, slots=True)
class IfPrint:
    tag: str
    cond: Expr


@dataclass(frozen=True, slots=True)
class Done:
    pass


Stmt = Assign | Print | IfPrint | Done


@dataclass(frozen=True, slots=True)
class Program:
    declared: tuple[tuple[str, Width], ...]
    stmts: tuple[Stmt, ...]


def eval_expr(node: Expr, env: dict[str, int]) -> int:
    match node:
        case Lit(_, value):
            return value
        case Var(name, _):
            return env[name]
        case UnaryOp(width, "-", operand):
            return wrap(width, -eval_expr(operand, env))
        case UnaryOp(width, "NOT", operand):
            return wrap(width, ~eval_expr(operand, env))
        case BinOp(width, op, left, right):
            return wrap(width, _apply(op, eval_expr(left, env), eval_expr(right, env)))
        case Cmp(op, left, right):
            return -1 if _compare(op, eval_expr(left, env), eval_expr(right, env)) else 0
        case _:
            raise TypeError(f"unhandled expr node: {node!r}")


def _apply(op: str, x: int, y: int) -> int:
    match op:
        case "+":
            return x + y
        case "-":
            return x - y
        case "*":
            return x * y
        case "AND":
            return x & y
        case "OR":
            return x | y
        case "XOR":
            return x ^ y
        case "\\":
            # BASIC's \\ truncates toward zero; Python's // floors
            return -(-x // y) if (x < 0) != (y < 0) else x // y
        case "MOD":
            q = -(-x // y) if (x < 0) != (y < 0) else x // y
            return x - q * y
        case _:
            raise ValueError(f"unknown arithmetic op: {op}")


def _compare(op: str, x: int, y: int) -> bool:
    match op:
        case "<":
            return x < y
        case "<=":
            return x <= y
        case ">":
            return x > y
        case ">=":
            return x >= y
        case "=":
            return x == y
        case "<>":
            return x != y
        case _:
            raise ValueError(f"unknown comparison op: {op}")


def _traps(op: str, width: Width, x: int, y: int) -> bool:
    if op not in DIVIDING_OPS:
        return False
    if y == 0:
        return True
    lo, _ = bounds(width)
    return x == lo and y == -1


# ---------------------------------------------------------------------------
# Generation
# ---------------------------------------------------------------------------


@dataclass(slots=True)
class _Ctx:
    rng: random.Random
    env: dict[str, int]
    vars_by_width: dict[Width, list[str]] = field(default_factory=lambda: {Width.INT: [], Width.LNG: []})
    declared: dict[str, Width] = field(default_factory=dict)
    next_id: int = 0


def _fresh_name(ctx: _Ctx) -> str:
    name = f"v{ctx.next_id}"
    ctx.next_id += 1
    return name


def render_lit(width: Width, value: int) -> str:
    lo, hi = bounds(width)
    # a bare MIN literal (-32768, -2147483648) does not compile -- BASIC's
    # lexer sees unary minus over a literal one past the positive range.
    # suite/cmpord.bas writes the LONG one the same way: "-2147483647 - 1"
    return f"(-{hi} - 1)" if value == lo else str(value)


def render_expr(node: Expr) -> str:
    match node:
        case Lit(width, value):
            return render_lit(width, value)
        case Var(name, _):
            return name
        case UnaryOp(_, op, operand):
            # a space after unary "-" matters: gluing it onto an operand that
            # itself renders leading with "-" (a negative literal, or another
            # unary "-") produces "--", which BC's parser does not recover
            # from cleanly -- it garbled a later, unrelated statement instead
            # of rejecting this one, on exactly the case an all-Lit smoke test
            # of this generator caught
            return f"({op} {render_expr(operand)})" if op == "NOT" else f"(- {render_expr(operand)})"
        case BinOp(_, op, left, right):
            return f"({render_expr(left)} {op} {render_expr(right)})"
        case Cmp(op, left, right):
            return f"({render_expr(left)} {op} {render_expr(right)})"
        case _:
            raise TypeError(f"unhandled expr node: {node!r}")


def natural_width(node: Expr, declared: dict[str, Width]) -> Width:
    """The width BASIC itself infers for `node`, from its real leaves.

    A generator's own `width` tag on a `BinOp`/`UnaryOp` node is only true if
    it agrees with this: BASIC decides an operator's width from its own two
    immediate operands' types, not from what a generator intended several
    levels up. A `Var`'s type is whatever it was DIMmed as, not whichever
    `width` a caller happened to request when picking it; a bare `Lit`'s type
    is INTEGER unless its own magnitude does not fit, regardless of which
    `width` a caller generated it for. `Cmp` is a type *barrier*: a
    comparison of two LONGs is still an INTEGER truth value, so it resets
    the type going up through it, the way it does in real BASIC.
    """
    match node:
        case Lit(_, value):
            lo, hi = bounds(Width.INT)
            return Width.INT if lo <= value <= hi else Width.LNG
        case Var(name, _):
            return declared[name]
        case Cmp():
            return Width.INT
        case UnaryOp(_, _, operand):
            return natural_width(operand, declared)
        case BinOp(_, _, left, right):
            lw, rw = natural_width(left, declared), natural_width(right, declared)
            return Width.LNG if Width.LNG in (lw, rw) else Width.INT
        case _:
            raise TypeError(f"unhandled expr node: {node!r}")


def _gen_leaf(ctx: _Ctx, width: Width) -> Expr:
    candidates = list(ctx.vars_by_width[width])
    if candidates and ctx.rng.random() < 0.6:
        return Var(ctx.rng.choice(candidates), width)
    if width is Width.INT:
        return Lit(width, ctx.rng.randint(*bounds(Width.INT)))
    # a LONG leaf literal must itself exceed INTEGER's range, or BASIC types
    # the bare literal INTEGER regardless of what this generator intends --
    # natural_width() is what catches a leaf that doesn't
    magnitude = ctx.rng.randint(32769, bounds(Width.LNG)[1])
    return Lit(width, magnitude if ctx.rng.random() < 0.5 else -magnitude)


def _gen_child_width(ctx: _Ctx, width: Width) -> Width:
    # a LONG node may recurse into an all-INTEGER subexpression, which BASIC
    # evaluates at 16 bits and then promotes losslessly -- an INTEGER node
    # never recurses into LONG, which would need a narrowing conversion this
    # grammar deliberately does not model
    if width is Width.LNG and ctx.rng.random() < 0.35:
        return Width.INT
    return width


def _contains_var(node: Expr) -> bool:
    match node:
        case Var():
            return True
        case Lit():
            return False
        case UnaryOp(_, _, operand):
            return _contains_var(operand)
        case BinOp(_, _, left, right) | Cmp(_, left, right):
            return _contains_var(left) or _contains_var(right)
        case _:
            raise TypeError(f"unhandled expr node: {node!r}")


def _force_var(ctx: _Ctx, width: Width) -> Expr:
    candidates = ctx.vars_by_width[width]
    if not candidates:
        # no variable of this width exists (never true for generate_program's
        # own defaults, which always declare at least one of each) -- a lone
        # literal cannot trigger the constant-fold overflow, only a pair does
        return Lit(width, ctx.rng.randint(*bounds(width)))
    return Var(ctx.rng.choice(candidates), width)


def _contains_not(node: Expr) -> bool:
    match node:
        case UnaryOp(_, "NOT", _):
            return True
        case UnaryOp(_, _, operand):
            return _contains_not(operand)
        case BinOp(_, _, left, right) | Cmp(_, left, right):
            return _contains_not(left) or _contains_not(right)
        case _:
            return False


def _gen_condition(ctx: _Ctx, width: Width, depth: int) -> Expr:
    # BC's own optimizer (VBDOS and PDS under /O, confirmed on both) misjudges
    # "IF <expr> THEN ... ELSE ..." whenever NOT appears anywhere in <expr> --
    # not only as the bare top-level shape "IF (NOT v) THEN", which could read
    # as a plausible peephole match on that literal syntax, but survives being
    # buried a level deeper too: "IF ((NOT v) + 0) THEN" still swaps branches
    # on v's own truthiness rather than the arithmetic NOT's, where a bare
    # `PRINT NOT v` computes the correct two's-complement value. NOT used as
    # pure logical negation on a canonical -1/0 value never exposes this
    # (both branch-swap and real bitwise NOT agree there, which is presumably
    # why ordinary BASIC code never surfaces it) -- this grammar's NOT is
    # exercised freely everywhere else, just never inside a condition.
    for _ in range(30):
        cond = _gen_expr(ctx, width, depth)
        if not _contains_not(cond):
            return cond
    # exhausted retries: a variable compared to zero can never contain NOT
    return Cmp("<>", _force_var(ctx, width), Lit(width, 0))


def _ensure_valid_operand(ctx: _Ctx, width: Width, left: Expr, right: Expr) -> tuple[Expr, Expr]:
    """What a freshly built `BinOp(width, op, left, right)` needs before it
    can keep that `width` tag, checked and (if need be) repaired in one pass.

    Two independent things can go wrong, and a single `Var` fix (drawn from
    the declared variables of `width`) happens to repair both at once:

    - BC constant-folds a BinOp of two literals at compile time, and that
      fold is overflow-checked where the runtime instruction is not (see the
      module docstring) -- at least one operand must be variable-rooted.
    - a LONG-tagged BinOp whose own two immediate operands (by
      `natural_width`, which looks straight through a variable-rooted but
      all-INTEGER subtree) are both naturally INTEGER is not actually LONG
      arithmetic in the BASIC BC compiles -- it is INTEGER arithmetic that
      merely gets *assigned* to something LONG-shaped further up, computed at
      16 bits where this generator's own AST would wrap it at 32. A `Var`
      declared LONG is both variable-rooted and naturally LONG in one shape.
    """
    has_var = _contains_var(left) or _contains_var(right)
    natural = (natural_width(left, ctx.declared), natural_width(right, ctx.declared))
    long_enough = width is Width.INT or Width.LNG in natural
    if has_var and long_enough:
        return left, right
    return left, _force_var(ctx, width)


def _gen_dividing(ctx: _Ctx, width: Width, op: str, depth: int) -> BinOp:
    left = _gen_expr(ctx, _gen_child_width(ctx, width), depth - 1)
    x = eval_expr(left, ctx.env)
    for _ in range(20):
        right = _gen_expr(ctx, _gen_child_width(ctx, width), depth - 1)
        y = eval_expr(right, ctx.env)
        valid = (_contains_var(left) or _contains_var(right)) and (
            width is Width.INT or Width.LNG in (natural_width(left, ctx.declared), natural_width(right, ctx.declared))
        )
        if not _traps(op, width, x, y) and valid:
            return BinOp(width, op, left, right)
    # exhausted retries: the first declared variable of this width whose
    # current value is a safe divisor -- also both variable-rooted and (for a
    # LONG op) naturally LONG in the one shape, and every program declares at
    # least one
    for name in ctx.vars_by_width[width]:
        y = ctx.env[name]
        if not _traps(op, width, x, y):
            return BinOp(width, op, left, Var(name, width))
    # every variable of this width is currently an unsafe divisor -- vanishing
    # probability with generate_program's own variable counts. AND never traps.
    return BinOp(width, "AND", left, _force_var(ctx, width))


def _gen_expr(ctx: _Ctx, width: Width, depth: int) -> Expr:
    if depth <= 0:
        return _gen_leaf(ctx, width)
    choice = ctx.rng.random()
    if choice < 0.25:
        return _gen_leaf(ctx, width)
    if choice < 0.35:
        op = ctx.rng.choice(("-", "NOT"))
        return UnaryOp(width, op, _gen_expr(ctx, width, depth - 1))
    # a comparison is always an INTEGER -1/0 truth value (a type barrier, see
    # natural_width) -- returning one directly only keeps this call's own
    # `width` contract when that width is INTEGER; at LONG width a comparison
    # is still generated freely, just as one operand of the BinOp below
    if choice < 0.55 and width is Width.INT:
        cmp_width = ctx.rng.choice((Width.INT, Width.LNG))
        left = _gen_expr(ctx, cmp_width, depth - 1)
        right = _gen_expr(ctx, cmp_width, depth - 1)
        return Cmp(ctx.rng.choice(CMP_OPS), left, right)
    op = ctx.rng.choice(ARITH_OPS)
    if op in DIVIDING_OPS:
        return _gen_dividing(ctx, width, op, depth)
    left = _gen_expr(ctx, _gen_child_width(ctx, width), depth - 1)
    right = _gen_expr(ctx, _gen_child_width(ctx, width), depth - 1)
    left, right = _ensure_valid_operand(ctx, width, left, right)
    return BinOp(width, op, left, right)


def generate_program(seed: int, n_int: int = 4, n_lng: int = 4, n_stmts: int = 20, depth: int = 3) -> Program:
    rng = random.Random(seed)
    ctx = _Ctx(rng=rng, env={})
    declared: list[tuple[str, Width]] = []
    stmts: list[Stmt] = []

    for width, count in ((Width.INT, n_int), (Width.LNG, n_lng)):
        for _ in range(count):
            name = _fresh_name(ctx)
            lo, hi = bounds(width)
            value = rng.randint(lo, hi)
            declared.append((name, width))
            ctx.vars_by_width[width].append(name)
            ctx.declared[name] = width
            ctx.env[name] = value
            stmts.append(Assign(name, Lit(width, value)))

    for i in range(n_stmts):
        kind = rng.random()
        if kind < 0.4:
            width = rng.choice((Width.INT, Width.LNG))
            name = rng.choice(ctx.vars_by_width[width])
            expr = _gen_expr(ctx, width, depth)
            ctx.env[name] = eval_expr(expr, ctx.env)
            stmts.append(Assign(name, expr))
        elif kind < 0.8:
            width = rng.choice((Width.INT, Width.LNG))
            stmts.append(Print(f"T{i}", _gen_expr(ctx, width, depth)))
        else:
            width = rng.choice((Width.INT, Width.LNG))
            stmts.append(IfPrint(f"T{i}", _gen_condition(ctx, width, depth)))

    stmts.append(Done())
    return Program(tuple(declared), tuple(stmts))


def render_program(program: Program) -> str:
    lines = ["DEFINT A-Z"]
    for name, width in program.declared:
        lines.append(f"DIM {name} AS {'INTEGER' if width is Width.INT else 'LONG'}")
    for stmt in program.stmts:
        match stmt:
            case Assign(name, expr):
                lines.append(f"{name} = {render_expr(expr)}")
            case Print(tag, expr):
                lines.append(f'PRINT "{tag}="; {render_expr(expr)}')
            case IfPrint(tag, cond):
                lines.append(f'IF {render_expr(cond)} THEN PRINT "{tag}=A" ELSE PRINT "{tag}=B"')
            case Done():
                lines.append('PRINT "DONE"')
    return "\r\n".join(lines) + "\r\n"


def golden_lines(program: Program) -> list[str]:
    env: dict[str, int] = {}
    out: list[str] = []
    for stmt in program.stmts:
        match stmt:
            case Assign(name, expr):
                env[name] = eval_expr(expr, env)
            case Print(tag, expr):
                out.append(f"{tag}={num(eval_expr(expr, env))}")
            case IfPrint(tag, cond):
                out.append(f"{tag}={'A' if eval_expr(cond, env) != 0 else 'B'}")
            case Done():
                out.append("DONE")
    return out
