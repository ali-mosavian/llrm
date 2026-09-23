"""
Small, disposable BASIC programs over INTEGER/LONG arithmetic, and the one
reference evaluator that says what each must print -- independent of BC and
of qbopt, built from agents.md's own measured semantics ("What the runtime
does that an instruction does not", "What widening changes, exactly") rather
than from either compiler's source.

Scope: scalars, arrays, FOR loops and SUB/FUNCTION procedures, over INTEGER
and LONG. No strings, no SINGLE/DOUBLE, no GOTO. What sets the boundary is the
optimiser's rule rather than the grammar's own taste -- a feature this cannot
generate is a feature nothing may rewrite, because once whole procedure bodies
are regenerated the byte-identical-except-at-known-sites gate is gone and this
differential is the only thing left. So the shapes an optimiser wants are the
shapes that had to be here: a subexpression that does not change across a
loop, the same subscript computed twice, a value live across a call, and a
call that can write memory the caller was holding in a register.

`IfPrint` is here specifically because nothing else in the grammar makes a JCC
depend on a computed value; every arithmetic result is otherwise only ever
stored or printed, which is not the failure mode a flag-sensitive widening bug
produces.

Both the generator and the golden author call the same evaluator --
deliberately. Using it twice only forces internal consistency, which is
trivial and proves nothing; the actual test of whether it is a real oracle is
external, run by tools/fuzzcheck.py: does BC's own independently-compiled,
independently-run build agree with it, checked before qbopt is ever in the
loop. The one thing generation must not do is filter *values* toward what
stays in range -- wraparound is exactly the behaviour this exists to exercise
(see `suite/divmod.bas`'s MULOVF and this project's own
multiply-is-absorbed-because-it-wraps reasoning). The only filters below are
the divide-by-zero and LONG/INTEGER MIN-by-(-1) trap cases, which qbopt's bare
`idiv` faults on and BC's runtime does not -- fuzzing that boundary on purpose
is suite/divmod.bas's job, by hand, not this generator's.

One more filter turned out to be load-bearing rather than optional, found by
compiling the first generated batch: BC's *compiler* constant-folds an
arithmetic operator whose both operands are literals, and that fold is
overflow-checked even though the equivalent runtime code is not (measured:
`(-1970530648 * -13199)` alone -- no variables -- fails to compile with "Math
overflow", where the same multiply through a variable, as in
`suite/divmod.bas`'s MULOVF, silently wraps at runtime). Every `BinOp` this
generator builds therefore keeps at least one variable-rooted operand, which
forces BC to emit real code instead of trying to fold it. An array element and
a function call both count as variable-rooted: BC can fold neither.

A second finding was BC's own, not the evaluator's or qbopt's: `IF <expr>
THEN ... ELSE ...` where NOT appears anywhere in <expr> takes the branch as if
testing <expr>'s own truthiness with the branches swapped, not the arithmetic
NOT's -- confirmed on VBDOS under /O, and it survives NOT being buried a level
deeper (`IF ((NOT v) + 0) THEN` still swaps on v alone), where a bare `PRINT
NOT v` computes the correct two's-complement value every time. Harmless for
NOT used as pure logical negation on a canonical -1/0 value (both readings
agree there, which is presumably why ordinary code never surfaces it) --
`_gen_condition` keeps NOT out of every `IfPrint` condition rather than
chase the exact rule further; written up in agents.md alongside the divide
one, as a BC behavior to know about rather than a qbopt bug.

## Why the loop bounds are literals and the counter is frozen

A FOR's limit and step are evaluated once, at entry, and cached; the counter
itself lives in the variable, so a body that assigns to it changes the
iteration. Both are real BASIC, and neither is worth betting an oracle on:
literal bounds make "once or every time" unobservable, and refusing to assign
to a counter -- through an assignment, through a nested FOR, or through a
BYREF parameter -- makes the trip count a property of the three literals
alone. The counter's own arithmetic is then far from either width's edge,
which matters because a counter that steps past INTEGER's range is an
`Overflow` error in BC, not the silent wrap every other operator here gets.

## Why every subscript is masked

`arr((<expr>) AND 15)` is in bounds for *every* value of `<expr>`, at either
width, two's complement -- there is no range reasoning to get wrong and no
generated program that can fault on a subscript. `DIM arr(15)` throughout, so
the mask and the bound are the same fact. An out-of-range subscript is a
runtime error, not the divergence this is hunting.

## Why FUNCTIONs are pure and only SUBs mutate

BASIC passes BYREF by default, so a callee writing a parameter writes the
caller's variable. Modelled as copy-in/copy-out, which is exactly equivalent
to aliasing only when nothing aliases: a CALL's arguments are therefore always
distinct variables, and never a SHARED one the callee could also reach by
name. That leaves the evaluation-order question a call inside an expression
would raise -- BC's argument and operand order is not something to assume --
and it is answered by construction instead: a FUNCTION writes nothing the
caller can see (no parameters, no SHARED, no array, no PRINT) and so may
appear anywhere in an expression, while a SUB, which writes all four, only
ever appears as a statement where the order is written down. Both still read
SHARED state, which is the constraint that actually bites a register
allocator.

## Why generation trial-runs every statement

`_gen_dividing` can only check a divisor against the values that hold where
the statement is being built. Inside a loop that is the first iteration, and
at a call site it is not the callee's parameters at all. So every statement is
executed against a copy of the environment before it is kept, and one that
raises `Trap` is thrown away and regenerated -- which also keeps the
environment the generator reasons about exactly the one the golden will see.
`_safe_divisor` is the other half: `((<expr> AND 32767) OR 1)` is positive and
nonzero whatever its operand does, so a divide can be generated that needs no
retry at all.

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
from enum import StrEnum
from dataclasses import field
from dataclasses import dataclass
from collections.abc import Callable

from qbprint import num
from qbprint import s16
from qbprint import s32

ARITH_OPS = ("+", "-", "*", "AND", "OR", "XOR", "\\", "MOD")
CMP_OPS = ("<", "<=", ">", ">=", "=", "<>")
DIVIDING_OPS = ("\\", "MOD")

# every array is DIM'd to this and every subscript is ANDed with it
ARRAY_LIMIT = 15

# BASIC's own source line is 255 characters; a statement that renders past it
# costs a sample to BCFAIL rather than finding anything, so it is regenerated
LINE_LIMIT = 240


class Width(Enum):
    INT = auto()
    LNG = auto()
    SNG = auto()
    DBL = auto()


TYPE_NAME = {Width.INT: "INTEGER", Width.LNG: "LONG", Width.SNG: "SINGLE", Width.DBL: "DOUBLE"}
SUFFIX = {Width.INT: "%", Width.LNG: "&", Width.SNG: "!", Width.DBL: "#"}

# The two that live on the x87 stack rather than in a register pair.
FLOAT = frozenset({Width.SNG, Width.DBL})

# What a float expression may be built from. No division: a quotient leaves
# the exactly-representable integers this generator stays inside, and no MOD,
# AND, OR, XOR or NOT, none of which BASIC defines on a float without first
# rounding it to an integer -- a conversion whose rule is BC's to decide and
# not something to infer.
FLOAT_OPS = ("+", "-", "*")

# The largest integer each format holds exactly: every integer up to 2**24 is
# a SINGLE and every one up to 2**53 is a DOUBLE. Staying inside this is what
# lets the evaluator below do integer arithmetic and still be right about a
# float program -- see this module's own docstring.
EXACT = {Width.SNG: 1 << 24, Width.DBL: 1 << 53}

# CLNG is what every float result is printed through, so a value that will
# not survive the conversion is not one to generate.
LONG_LIMIT = 2147483647


class ProcKind(StrEnum):
    SUB = "SUB"
    FUNCTION = "FUNCTION"


class Trap(Exception):
    """What BC's runtime survives and a bare 386 instruction does not."""


def bounds(width: Width) -> tuple[int, int]:
    if width is Width.INT:
        return (-32768, 32767)
    if width in FLOAT:
        # Deliberately far short of the format's real range. What is being
        # generated is exact integers, and the operands have to leave room for
        # a product to stay one -- see EXACT and this module's own docstring.
        limit = 2048 if width is Width.SNG else 4096
        return (-limit, limit)
    return (-2147483648, 2147483647)


def wrap(width: Width, v: int) -> int:
    """The value BASIC keeps, or a Trap where this generator will not go.

    An integer width wraps, which is the behaviour this exists to exercise.
    A float does not: it loses precision instead, and a program whose answer
    depends on which bits were lost is one whose oracle would have to model
    the x87's own rounding. So leaving the exactly-representable range is a
    Trap and the sample is regenerated, the way divide-by-zero already is.
    """
    if width in FLOAT:
        if abs(v) > EXACT[width] or abs(v) > LONG_LIMIT:
            raise Trap(f"{v} is past what {TYPE_NAME[width]} holds exactly, or past CLNG")
        return v
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
class Index:
    width: Width
    name: str
    index: "Expr"


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


@dataclass(frozen=True, slots=True)
class CallFn:
    width: Width
    name: str
    args: tuple["Expr", ...]


Expr = Lit | Var | Index | BinOp | UnaryOp | Cmp | CallFn


@dataclass(frozen=True, slots=True)
class Assign:
    name: str
    expr: Expr


@dataclass(frozen=True, slots=True)
class SetElem:
    name: str
    index: Expr
    expr: Expr


@dataclass(frozen=True, slots=True)
class Result:
    name: str
    width: Width
    expr: Expr


@dataclass(frozen=True, slots=True)
class Print:
    tag: str
    expr: Expr
    # The width BASIC itself infers, not the one the generator asked for.
    # A float result prints through CLNG and an integer one does not, and a
    # node's own tag is not evidence: an expression generated at DOUBLE
    # whose every leaf turned out to be an integer really is an integer.
    width: Width = Width.INT


@dataclass(frozen=True, slots=True)
class IfPrint:
    tag: str
    cond: Expr


@dataclass(frozen=True, slots=True)
class CallSub:
    name: str
    args: tuple[Expr, ...]


@dataclass(frozen=True, slots=True)
class ForLoop:
    var: str
    width: Width
    start: int
    limit: int
    step: int
    body: tuple["Stmt", ...]


@dataclass(frozen=True, slots=True)
class Done:
    pass


Stmt = Assign | SetElem | Result | Print | IfPrint | CallSub | ForLoop | Done


@dataclass(frozen=True, slots=True)
class Decl:
    name: str
    width: Width
    shared: bool
    size: int | None


@dataclass(frozen=True, slots=True)
class Param:
    name: str
    width: Width


@dataclass(frozen=True, slots=True)
class Proc:
    name: str
    kind: ProcKind
    result: Width | None
    params: tuple[Param, ...]
    locals: tuple[tuple[str, Width], ...]
    body: tuple[Stmt, ...]


@dataclass(frozen=True, slots=True)
class Program:
    declared: tuple[Decl, ...]
    procs: tuple[Proc, ...]
    stmts: tuple[Stmt, ...]
    widths: dict[str, Width]


# ---------------------------------------------------------------------------
# The reference evaluator
# ---------------------------------------------------------------------------


@dataclass(slots=True)
class Env:
    scalars: dict[str, int]
    arrays: dict[str, list[int]]
    procs: dict[str, Proc]
    out: list[str]


def initial_env(program: Program) -> Env:
    return Env(
        scalars={d.name: 0 for d in program.declared if d.size is None},
        arrays={d.name: [0] * (d.size + 1) for d in program.declared if d.size is not None},
        procs={p.name: p for p in program.procs},
        out=[],
    )


def clone(env: Env) -> Env:
    return Env(dict(env.scalars), {n: list(v) for n, v in env.arrays.items()}, env.procs, list(env.out))


def eval_expr(node: Expr, env: Env) -> int:
    match node:
        case Lit(_, value):
            return value
        case Var(name, _):
            return env.scalars[name]
        case Index(_, name, index):
            return env.arrays[name][_subscript(name, index, env)]
        case UnaryOp(width, "-", operand):
            return wrap(width, -eval_expr(operand, env))
        case UnaryOp(width, "NOT", operand):
            return wrap(width, ~eval_expr(operand, env))
        case BinOp(width, op, left, right):
            x, y = eval_expr(left, env), eval_expr(right, env)
            if _traps(op, width, x, y):
                raise Trap(f"{x} {op} {y} faults as a bare instruction")
            return wrap(width, _apply(op, x, y))
        case Cmp(op, left, right):
            return -1 if _compare(op, eval_expr(left, env), eval_expr(right, env)) else 0
        case CallFn(_, name, args):
            return _call(env.procs[name], args, env)
        case _:
            raise TypeError(f"unhandled expr node: {node!r}")


def _subscript(name: str, index: Expr, env: Env) -> int:
    value = eval_expr(index, env)
    if not 0 <= value < len(env.arrays[name]):
        raise Trap(f"{name}({value}) is out of bounds")
    return value


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


def _call(proc: Proc, args: tuple[Expr, ...], env: Env) -> int:
    # names are unique across the whole program, so a frame is inserted into
    # the one scalar map and removed again rather than shadowing anything;
    # procedures never call procedures, so frames never nest
    values = [eval_expr(a, env) for a in args]
    frame = [p.name for p in proc.params] + [n for n, _ in proc.locals]
    if proc.result is not None:
        frame.append(proc.name)
    env.scalars |= dict(zip(frame, values + [0] * (len(frame) - len(values)), strict=True))
    try:
        run_stmts(proc.body, env)
        returned = env.scalars[proc.name] if proc.result is not None else 0
        for param, arg in zip(proc.params, args, strict=True):
            if isinstance(arg, Var):
                env.scalars[arg.name] = env.scalars[param.name]
    finally:
        # a Trap escaping mid-body must still take the frame with it: the
        # generator probes expressions against its own live environment
        for name in frame:
            del env.scalars[name]
    return returned


def run_stmts(stmts: tuple[Stmt, ...], env: Env) -> None:
    for stmt in stmts:
        run_stmt(stmt, env)


def run_stmt(stmt: Stmt, env: Env) -> None:
    match stmt:
        case Assign(name, expr) | Result(name, _, expr):
            env.scalars[name] = eval_expr(expr, env)
        case SetElem(name, index, expr):
            at = _subscript(name, index, env)
            env.arrays[name][at] = eval_expr(expr, env)
        case Print(tag, expr):
            env.out.append(f"{tag}={num(eval_expr(expr, env))}")
        case IfPrint(tag, cond):
            env.out.append(f"{tag}={'A' if eval_expr(cond, env) != 0 else 'B'}")
        case CallSub(name, args):
            _call(env.procs[name], args, env)
        case ForLoop(var, _, start, limit, step, body):
            # the limit and the step are read once, at entry; the counter
            # lives in the variable and nothing generated here assigns to it
            env.scalars[var] = start
            while env.scalars[var] <= limit if step > 0 else env.scalars[var] >= limit:
                run_stmts(body, env)
                env.scalars[var] += step
        case Done():
            env.out.append("DONE")
        case _:
            raise TypeError(f"unhandled statement: {stmt!r}")


def golden_lines(program: Program) -> list[str]:
    env = initial_env(program)
    run_stmts(program.stmts, env)
    return env.out


# ---------------------------------------------------------------------------
# Rendering
# ---------------------------------------------------------------------------


def render_lit(width: Width, value: int) -> str:
    if width in FLOAT:
        # Typed by its suffix. Without one BC reads `5` as an INTEGER, which
        # makes an all-literal float expression fold at INTEGER width and
        # stops the x87 code this exists to generate from being emitted.
        return f"{value}{SUFFIX[width]}" if value >= 0 else f"(- {abs(value)}{SUFFIX[width]})"
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
        case Index(_, name, index):
            return f"{name}({render_expr(index)})"
        case UnaryOp(_, op, operand):
            # a space after unary "-" matters: gluing it onto an operand that
            # itself renders leading with "-" (a negative literal, or another
            # unary "-") produces "--", which BC's parser does not recover
            # from cleanly -- it garbled a later, unrelated statement instead
            # of rejecting this one, on exactly the case an all-Lit smoke test
            # of this generator caught
            return f"({op} {render_expr(operand)})" if op == "NOT" else f"(- {render_expr(operand)})"
        case BinOp(_, op, left, right) | Cmp(op, left, right):
            return f"({render_expr(left)} {op} {render_expr(right)})"
        case CallFn(width, name, args):
            return f"{name}{SUFFIX[width]}({render_args(args)})"
        case _:
            raise TypeError(f"unhandled expr node: {node!r}")


def render_args(args: tuple[Expr, ...]) -> str:
    return ", ".join(render_expr(a) for a in args)


def render_params(params: tuple[Param, ...]) -> str:
    return ", ".join(f"{p.name} AS {TYPE_NAME[p.width]}" for p in params)


def render_header(proc: Proc) -> str:
    name = proc.name if proc.result is None else f"{proc.name}{SUFFIX[proc.result]}"
    return f"{proc.kind} {name} ({render_params(proc.params)})"


def render_decl(decl: Decl) -> str:
    shared = "SHARED " if decl.shared else ""
    size = "" if decl.size is None else f"({decl.size})"
    return f"DIM {shared}{decl.name}{size} AS {TYPE_NAME[decl.width]}"


def render_stmts(stmts: tuple[Stmt, ...], indent: int) -> list[str]:
    pad = "    " * indent
    lines: list[str] = []
    for stmt in stmts:
        match stmt:
            case Assign(name, expr):
                lines.append(f"{pad}{name} = {render_expr(expr)}")
            case Result(name, width, expr):
                lines.append(f"{pad}{name}{SUFFIX[width]} = {render_expr(expr)}")
            case SetElem(name, index, expr):
                lines.append(f"{pad}{name}({render_expr(index)}) = {render_expr(expr)}")
            case Print(tag, expr, width):
                # Through CLNG where the value is a float, exactly as
                # suite/fpemu.bas does: what is under test is that the x87
                # site computed the right number, never QuickBASIC's own
                # floating-point PRINT formatting, which is a far larger
                # thing to model and none of this pass's business.
                shown = render_expr(expr)
                if width in FLOAT:
                    shown = f"CLNG({shown})"
                lines.append(f'{pad}PRINT "{tag}="; {shown}')
            case IfPrint(tag, cond):
                lines.append(f'{pad}IF {render_expr(cond)} THEN PRINT "{tag}=A" ELSE PRINT "{tag}=B"')
            case CallSub(name, args):
                lines.append(f"{pad}CALL {name}({render_args(args)})")
            case ForLoop(var, width, start, limit, step, body):
                step_text = "" if step == 1 else f" STEP {render_lit(width, step)}"
                lines.append(f"{pad}FOR {var} = {render_lit(width, start)} TO {render_lit(width, limit)}{step_text}")
                lines += render_stmts(body, indent + 1)
                lines.append(f"{pad}NEXT {var}")
            case Done():
                lines.append(f'{pad}PRINT "DONE"')
            case _:
                raise TypeError(f"unhandled statement: {stmt!r}")
    return lines


def render_proc(proc: Proc) -> list[str]:
    head = render_header(proc)
    declarations = [f"    DIM {name} AS {TYPE_NAME[width]}" for name, width in proc.locals]
    return ["", head, *declarations, *render_stmts(proc.body, 1), f"END {proc.kind}"]


def render_program(program: Program) -> str:
    lines = ["DEFINT A-Z"]
    lines += [f"DECLARE {render_header(p)}" for p in program.procs]
    lines += [render_decl(d) for d in program.declared]
    lines += render_stmts(program.stmts, 0)
    for proc in program.procs:
        lines += render_proc(proc)
    return "\r\n".join(lines) + "\r\n"


# ---------------------------------------------------------------------------
# What BASIC itself infers
# ---------------------------------------------------------------------------


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
    the type going up through it, the way it does in real BASIC. An array
    element is its array's element type and a call is its function's declared
    return type -- neither depends on anything below it.
    """
    match node:
        case Lit(width, value):
            # A float literal carries its own suffix (render_lit writes one),
            # so BASIC types it by that and not by its magnitude.
            if width in FLOAT:
                return width
            lo, hi = bounds(Width.INT)
            return Width.INT if lo <= value <= hi else Width.LNG
        case Var(name, _) | Index(_, name, _) | CallFn(_, name, _):
            return declared[name]
        case Cmp():
            return Width.INT
        case UnaryOp(_, _, operand):
            return natural_width(operand, declared)
        case BinOp(_, _, left, right):
            return _wider(natural_width(left, declared), natural_width(right, declared))
        case _:
            raise TypeError(f"unhandled expr node: {node!r}")


# BASIC promotes an operator to the wider of its two operands, and this is
# that order. A float always wins over an integer width, which is why an
# integer subexpression may sit inside a float one and never the reverse.
_RANK = {Width.INT: 0, Width.LNG: 1, Width.SNG: 2, Width.DBL: 3}


def _wider(left: Width, right: Width) -> Width:
    return left if _RANK[left] >= _RANK[right] else right


def _contains_var(node: Expr) -> bool:
    match node:
        # BC folds neither an element nor a call, which is all this asks
        case Var() | Index() | CallFn():
            return True
        case Lit():
            return False
        case UnaryOp(_, _, operand):
            return _contains_var(operand)
        case BinOp(_, _, left, right) | Cmp(_, left, right):
            return _contains_var(left) or _contains_var(right)
        case _:
            raise TypeError(f"unhandled expr node: {node!r}")


def _contains_not(node: Expr) -> bool:
    match node:
        case UnaryOp(_, "NOT", _):
            return True
        case UnaryOp(_, _, operand) | Index(_, _, operand):
            return _contains_not(operand)
        case BinOp(_, _, left, right) | Cmp(_, left, right):
            return _contains_not(left) or _contains_not(right)
        case CallFn(_, _, args):
            return any(_contains_not(a) for a in args)
        case _:
            return False


# ---------------------------------------------------------------------------
# Generation
# ---------------------------------------------------------------------------


class Kind(StrEnum):
    ASSIGN = "assign"
    ELEMENT = "element"
    PRINT = "print"
    BRANCH = "branch"
    CALL = "call"
    LOOP = "loop"


@dataclass(slots=True)
class _Ids:
    next_id: int = 0


def _fresh(ids: _Ids, prefix: str) -> str:
    name = f"{prefix}{ids.next_id}"
    ids.next_id += 1
    return name


@dataclass(slots=True)
class _Ctx:
    rng: random.Random
    ids: _Ids
    env: Env
    declared: dict[str, Width]
    readable: dict[Width, list[str]]
    writable: dict[Width, list[str]]
    arrays: dict[Width, list[str]]
    writable_arrays: dict[Width, list[str]]
    byref: dict[Width, list[str]]
    shared: tuple[tuple[str, Width], ...] = ()
    subs: tuple[Proc, ...] = ()
    funcs: tuple[Proc, ...] = ()
    prints: bool = True
    frozen: set[str] = field(default_factory=set)
    loops: int = 0

    @property
    def shared_names(self) -> set[str]:
        return {name for name, _ in self.shared}


def _by_width() -> dict[Width, list[str]]:
    return {width: [] for width in Width}


def _writable(ctx: _Ctx, width: Width) -> list[str]:
    return [n for n in ctx.writable[width] if n not in ctx.frozen]


def _fits(stmt: Stmt) -> bool:
    return all(len(line) <= LINE_LIMIT for line in render_stmts((stmt,), 0))


def _commit(ctx: _Ctx, stmt: Stmt) -> bool:
    trial = clone(ctx.env)
    try:
        run_stmt(stmt, trial)
    except Trap:
        return False
    ctx.env = trial
    return True


def _gen_leaf(ctx: _Ctx, width: Width, depth: int) -> Expr:
    roll = ctx.rng.random()
    if depth > 0 and ctx.arrays[width] and roll < 0.25:
        return Index(width, ctx.rng.choice(ctx.arrays[width]), _gen_index(ctx, depth - 1))
    callable_here = [f for f in ctx.funcs if f.result is width]
    if depth > 0 and callable_here and roll < 0.4:
        return _gen_call_expr(ctx, ctx.rng.choice(callable_here), width, depth - 1)
    if ctx.readable[width] and ctx.rng.random() < 0.6:
        return Var(ctx.rng.choice(ctx.readable[width]), width)
    if width is Width.INT or width in FLOAT:
        # A float literal is typed by the suffix render_lit gives it, so it
        # needs no magnitude trick and must not get one: bounds() for a float
        # is the exactly-representable range, and a literal past it is a
        # value BC rounds and this evaluator does not. `CLNG(1259949101!)`
        # printed 1259949056 on VBDOS /G3 -- the same number with its low
        # bits gone -- which is what sent this generator's first float batch
        # to BASEDIFF.
        return Lit(width, ctx.rng.randint(*bounds(width)))
    # a LONG leaf literal must itself exceed INTEGER's range, or BASIC types
    # the bare literal INTEGER regardless of what this generator intends --
    # natural_width() is what catches a leaf that doesn't
    magnitude = ctx.rng.randint(32769, bounds(Width.LNG)[1])
    return Lit(width, magnitude if ctx.rng.random() < 0.5 else -magnitude)


def _gen_index(ctx: _Ctx, depth: int) -> Expr:
    inner = _gen_expr(ctx, Width.INT, min(depth, 2))
    if not _contains_var(inner):
        inner = _force_var(ctx, Width.INT)
    return BinOp(Width.INT, "AND", inner, Lit(Width.INT, ARRAY_LIMIT))


def _gen_call_expr(ctx: _Ctx, proc: Proc, width: Width, depth: int) -> CallFn:
    return CallFn(width, proc.name, tuple(_gen_expr(ctx, p.width, depth) for p in proc.params))


def _gen_child_width(ctx: _Ctx, width: Width) -> Width:
    # a LONG node may recurse into an all-INTEGER subexpression, which BASIC
    # evaluates at 16 bits and then promotes losslessly -- an INTEGER node
    # never recurses into LONG, which would need a narrowing conversion this
    # grammar deliberately does not model
    if width is Width.LNG and ctx.rng.random() < 0.35:
        return Width.INT
    if width in FLOAT and ctx.rng.random() < 0.30:
        # An integer subexpression promotes into a float one exactly, the way
        # an INTEGER one promotes into a LONG. The reverse would be a
        # narrowing conversion whose rounding rule is BC's, and this grammar
        # does not model it.
        return Width.INT if ctx.rng.random() < 0.5 else Width.LNG
    return width


def _force_float(ctx: _Ctx, width: Width, node: Expr) -> Expr:
    """`node`, guaranteed to be a float expression rather than merely tagged one.

    _gen_child_width may recurse a float node into an all-integer subtree, and
    _gen_leaf may fall back to a literal, either of which leaves an expression
    that BASIC types as INTEGER or LONG. It would still be correct -- and
    would emit no x87 instruction at all, which is the whole point of
    generating it. So a float leaf is added where none survived.
    """
    if natural_width(node, ctx.declared) in FLOAT:
        return node
    return BinOp(width, "+", _force_var(ctx, width), node)


def _force_var(ctx: _Ctx, width: Width) -> Expr:
    candidates = ctx.readable[width]
    if not candidates:
        # no variable of this width exists (never true for generate_program's
        # own defaults, which always declare at least one of each) -- a lone
        # literal cannot trigger the constant-fold overflow, only a pair does
        return Lit(width, ctx.rng.randint(*bounds(width)))
    return Var(ctx.rng.choice(candidates), width)


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

    The second rule is about LONG and only LONG. It used to read "INTEGER is
    fine, everything else needs a LONG among its operands", which quietly
    caught SINGLE and DOUBLE as well: a SINGLE node with two SINGLE operands
    has no LONG in sight, so its right operand was replaced -- every time.
    Float arithmetic here was `<subtree> op <plain var>` and nothing else,
    for as long as this function has existed. A float node that is genuinely
    float is checked by its own caller, right after this returns.
    """
    has_var = _contains_var(left) or _contains_var(right)
    natural = (natural_width(left, ctx.declared), natural_width(right, ctx.declared))
    long_enough = width is not Width.LNG or Width.LNG in natural
    if has_var and long_enough:
        return left, right
    return left, _force_var(ctx, width)


def _safe_divisor(ctx: _Ctx, width: Width, depth: int) -> Expr:
    # ((<expr> AND MAX) OR 1) is positive and odd whatever <expr> holds, so it
    # is neither zero nor the -1 that pairs with MIN -- the two cases a bare
    # idiv faults on. Both halves keep the node variable-rooted and, at LONG,
    # naturally LONG: MAX itself does not fit an INTEGER.
    inner = _gen_expr(ctx, width, depth)
    if not _contains_var(inner):
        inner = _force_var(ctx, width)
    masked = BinOp(width, "AND", inner, Lit(width, bounds(width)[1]))
    return BinOp(width, "OR", masked, Lit(Width.INT, 1))


def _probe(ctx: _Ctx, node: Expr) -> int | None:
    # what `node` is worth where it is being built, or None if it cannot be
    # asked -- a call in it can trap on the callee's own divide
    try:
        return eval_expr(node, ctx.env)
    except Trap:
        return None


def _gen_dividing(ctx: _Ctx, width: Width, op: str, depth: int) -> BinOp:
    left = _gen_expr(ctx, _gen_child_width(ctx, width), depth - 1)
    x = _probe(ctx, left)
    if x is None or ctx.rng.random() < 0.5:
        return BinOp(width, op, left, _safe_divisor(ctx, width, depth - 1))
    for _ in range(20):
        right = _gen_expr(ctx, _gen_child_width(ctx, width), depth - 1)
        y = _probe(ctx, right)
        valid = (_contains_var(left) or _contains_var(right)) and (
            width is Width.INT or Width.LNG in (natural_width(left, ctx.declared), natural_width(right, ctx.declared))
        )
        if y is not None and valid and not _traps(op, width, x, y):
            return BinOp(width, op, left, right)
    return BinOp(width, op, left, _safe_divisor(ctx, width, 0))


def _repeated(ctx: _Ctx, left: Expr) -> Expr | None:
    """Sometimes the same operand twice: `p(i) * p(i)`, not two fresh leaves.

    Two reads of one address, in one expression, is a shape this generator
    could reach only by picking the same array and the same index twice by
    chance -- which in practice it never did. It is the shape that broke
    qbopt: `fld dword ptr [si]` twice running are two pushes, and a pass
    that read si as the destination deleted the second as a redundant
    reload. bench/fpbench.bas found that; nothing generated here could
    have.

    Only a Var or an Index is repeated, so the copy reads memory and
    evaluates to the same value with nothing observable happening twice.
    Repeating a call would still be correct -- `f(x) * f(x)` really does
    call f twice, and eval_expr walks the tree twice too -- but it changes
    what the program does rather than how the same value is fetched, which
    is not what this is for.

    Not on a dividing operator: the caller reaches _gen_dividing before
    here, so a repeated operand can never land in a divisor and turn a
    zero left-hand side into a division by zero.
    """
    if not isinstance(left, Var | Index) or ctx.rng.random() >= 0.15:
        return None
    return left


def _gen_expr(ctx: _Ctx, width: Width, depth: int) -> Expr:
    if depth <= 0:
        return _gen_leaf(ctx, width, depth)
    choice = ctx.rng.random()
    if choice < 0.25:
        return _gen_leaf(ctx, width, depth)
    if choice < 0.35:
        # NOT is a bitwise operator, which BASIC defines on a float only by
        # rounding it to an integer first -- a conversion this does not model.
        op = "-" if width in FLOAT else ctx.rng.choice(("-", "NOT"))
        operand = _gen_expr(ctx, width, depth - 1)
        if width in FLOAT:
            operand = _force_float(ctx, width, operand)
        return UnaryOp(width, op, operand)
    # a comparison is always an INTEGER -1/0 truth value (a type barrier, see
    # natural_width) -- returning one directly only keeps this call's own
    # `width` contract when that width is INTEGER; at LONG width a comparison
    # is still generated freely, just as one operand of the BinOp below
    if choice < 0.55 and width is Width.INT:
        cmp_width = ctx.rng.choice((Width.INT, Width.LNG))
        left = _gen_expr(ctx, cmp_width, depth - 1)
        right = _gen_expr(ctx, cmp_width, depth - 1)
        return Cmp(ctx.rng.choice(CMP_OPS), left, right)
    op = ctx.rng.choice(FLOAT_OPS if width in FLOAT else ARITH_OPS)
    if op in DIVIDING_OPS:
        return _gen_dividing(ctx, width, op, depth)
    left = _gen_expr(ctx, _gen_child_width(ctx, width), depth - 1)
    right = _repeated(ctx, left) or _gen_expr(ctx, _gen_child_width(ctx, width), depth - 1)
    left, right = _ensure_valid_operand(ctx, width, left, right)
    if width in FLOAT and _wider(natural_width(left, ctx.declared), natural_width(right, ctx.declared)) not in FLOAT:
        # Both children came back integer, which _gen_child_width is allowed
        # to do -- but then BASIC types this node INTEGER or LONG and the
        # generator's own tag would be a lie, which
        # test_every_binop_width_matches_its_own_natural_type checks. It
        # would also emit no x87 instruction, which is the point of asking
        # for a float node at all.
        left = _force_var(ctx, width)
    return BinOp(width, op, left, right)


def _counters(ctx: _Ctx) -> list[tuple[str, Width]]:
    # a counter must be one nothing else can write: not SHARED (a called SUB
    # reaches those by name), and the frozen set keeps a nested FOR, an
    # assignment and a BYREF argument off the ones already counting
    shared = ctx.shared_names
    return [(n, w) for w in Width if w not in FLOAT for n in _writable(ctx, w) if n not in shared]


def _loop_bounds(ctx: _Ctx) -> tuple[int, int, int]:
    start = ctx.rng.randint(-3, 3)
    step = ctx.rng.choice((1, 1, 1, 2, 3, -1, -1, -2))
    trips = ctx.rng.randint(0, 5)
    sign = 1 if step > 0 else -1
    if trips == 0:
        return start, start - sign, step
    # the last value that still passes the test, plus a partial step that does
    # not reach the next one -- so the loop runs exactly `trips` times
    return start, start + step * (trips - 1) + sign * ctx.rng.randint(0, abs(step) - 1), step


def _gen_loop(ctx: _Ctx, depth: int) -> Stmt | None:
    counters = _counters(ctx)
    if not counters:
        return None
    var, width = ctx.rng.choice(counters)
    start, limit, step = _loop_bounds(ctx)
    body = _gen_scoped_body(ctx, var, start, ctx.rng.randint(1, 3), max(depth - 1, 1))
    return ForLoop(var, width, start, limit, step, body)


def _gen_scoped_body(ctx: _Ctx, counter: str, start: int, count: int, depth: int) -> tuple[Stmt, ...]:
    # the body is built against the first iteration's values and the loop as a
    # whole is trial-run afterwards, by whoever commits it
    saved = clone(ctx.env)
    ctx.env.scalars[counter] = start
    ctx.frozen.add(counter)
    ctx.loops += 1
    body = tuple(_gen_committed(ctx, depth) for _ in range(count))
    ctx.loops -= 1
    ctx.frozen.discard(counter)
    ctx.env = saved
    return body


def _gen_call(ctx: _Ctx, depth: int) -> Stmt | None:
    proc = ctx.rng.choice(ctx.subs)
    args: list[Expr] = []
    taken: set[str] = set()
    for param in proc.params:
        pool = [n for n in ctx.byref[param.width] if n not in taken and n not in ctx.frozen]
        if not pool:
            return None
        name = ctx.rng.choice(pool)
        taken.add(name)
        args.append(Var(name, param.width))
    return CallSub(proc.name, tuple(args))


def _gen_stmt(ctx: _Ctx, depth: int) -> Stmt | None:
    weights: dict[Kind, float] = {Kind.ASSIGN: 4.0}
    if any(ctx.writable_arrays.values()):
        weights[Kind.ELEMENT] = 3.0
    if ctx.prints:
        weights |= {Kind.PRINT: 4.0, Kind.BRANCH: 2.0}
    if ctx.subs:
        weights[Kind.CALL] = 2.0
    if ctx.loops < 2:
        weights[Kind.LOOP] = 3.0
    kind = ctx.rng.choices(list(weights), list(weights.values()))[0]
    # Integers stay the common case -- they are what most of BC's output is,
    # and a float statement costs more of the sample to a Trap because it has
    # a narrower range to stay inside.
    width = ctx.rng.choices(
        (Width.INT, Width.LNG, Width.SNG, Width.DBL),
        (4.0, 4.0, 1.5, 1.5),
    )[0]
    match kind:
        case Kind.ASSIGN:
            targets = _writable(ctx, width)
            if not targets:
                return None
            return Assign(ctx.rng.choice(targets), _gen_expr(ctx, width, depth))
        case Kind.ELEMENT:
            if not ctx.writable_arrays[width]:
                return None
            name = ctx.rng.choice(ctx.writable_arrays[width])
            return SetElem(name, _gen_index(ctx, depth), _gen_expr(ctx, width, depth))
        case Kind.PRINT:
            shown = _gen_expr(ctx, width, depth)
            return Print(_fresh(ctx.ids, "T"), shown, natural_width(shown, ctx.declared))
        case Kind.BRANCH:
            # A condition is an INTEGER truth value and _gen_condition builds
            # it from comparisons; a float one would be asking a different
            # question -- what x87 compares -- which suite/fpemu.bas covers
            # by hand and this grammar does not generate.
            plain = width if width not in FLOAT else Width.INT
            return IfPrint(_fresh(ctx.ids, "T"), _gen_condition(ctx, plain, depth))
        case Kind.CALL:
            return _gen_call(ctx, depth)
        case Kind.LOOP:
            return _gen_loop(ctx, depth)
        case _:
            raise ValueError(f"unknown statement kind: {kind!r}")


def _gen_committed(ctx: _Ctx, depth: int) -> Stmt:
    for _ in range(8):
        stmt = _gen_stmt(ctx, depth)
        if stmt is not None and _fits(stmt) and _commit(ctx, stmt):
            return stmt
    return _gen_fallback(ctx)


def _gen_fallback(ctx: _Ctx) -> Stmt:
    # a literal into a free variable: no divisor, no subscript, no call, so
    # nothing left that could trap
    width = ctx.rng.choice([w for w in Width if _writable(ctx, w)])
    stmt = Assign(ctx.rng.choice(_writable(ctx, width)), Lit(width, ctx.rng.randint(*bounds(width))))
    _commit(ctx, stmt)
    return stmt


def _gen_body(ctx: _Ctx, count: int, depth: int) -> tuple[Stmt, ...]:
    return tuple(_gen_committed(ctx, depth) for _ in range(count))


def _proc_scope(ctx: _Ctx, kind: ProcKind, params: tuple[Param, ...], locals_: tuple[tuple[str, Width], ...]) -> _Ctx:
    # a local is readable only once its own initialiser has been built, which
    # `_gen_proc` arranges by appending it there -- otherwise the initialiser
    # can read the variable it is initialising
    readable, writable = _by_width(), _by_width()
    for param in params:
        readable[param.width].append(param.name)
    for name, width in locals_:
        writable[width].append(name)
    for name, width in ctx.shared:
        readable[width].append(name)
    if kind is ProcKind.SUB:
        for param in params:
            writable[param.width].append(param.name)
        for name, width in ctx.shared:
            writable[width].append(name)
    env = clone(ctx.env)
    env.scalars |= {p.name: ctx.rng.randint(*bounds(p.width)) for p in params}
    env.scalars |= {n: 0 for n, _ in locals_}
    return _Ctx(
        rng=ctx.rng,
        ids=ctx.ids,
        env=env,
        declared=ctx.declared,
        readable=readable,
        writable=writable,
        arrays=ctx.arrays,
        writable_arrays=ctx.arrays if kind is ProcKind.SUB else _by_width(),
        byref=_by_width(),
        shared=ctx.shared,
        prints=kind is ProcKind.SUB,
    )


def _gen_proc(ctx: _Ctx, kind: ProcKind, count: int, depth: int) -> Proc:
    result = ctx.rng.choice((Width.INT, Width.LNG)) if kind is ProcKind.FUNCTION else None
    name = _fresh(ctx.ids, "proc" if kind is ProcKind.SUB else "func")
    params = tuple(Param(_fresh(ctx.ids, "arg"), ctx.rng.choice((Width.INT, Width.LNG))) for _ in range(2))
    # two of each width, so two nested FOR loops can both find a counter and
    # still leave something assignable
    locals_ = tuple((_fresh(ctx.ids, "tmp"), width) for width in (Width.INT, Width.INT, Width.LNG, Width.LNG))
    ctx.declared |= {p.name: p.width for p in params} | dict(locals_)
    if result is not None:
        ctx.declared[name] = result

    scope = _proc_scope(ctx, kind, params, locals_)
    # every local is written before it is read, so whether BC's own prologue
    # zeroes the frame never enters the answer
    body = []
    for local, width in locals_:
        body.append(_commit_expr(scope, _assign_to(local), width, 1))
        scope.readable[width].append(local)
    body += _gen_body(scope, count, depth)
    if result is not None:
        body.append(_commit_expr(scope, _result_of(name, result), result, depth))
    return Proc(name, kind, result, params, locals_, tuple(body))


def _assign_to(name: str) -> Callable[[Expr], Stmt]:
    return lambda expr: Assign(name, expr)


def _result_of(name: str, width: Width) -> Callable[[Expr], Stmt]:
    return lambda expr: Result(name, width, expr)


def _fill_of(name: str, counter: str) -> Callable[[Expr], Stmt]:
    index = BinOp(Width.INT, "AND", Var(counter, Width.INT), Lit(Width.INT, ARRAY_LIMIT))
    return lambda expr: ForLoop(counter, Width.INT, 0, ARRAY_LIMIT, 1, (SetElem(name, index, expr),))


def _commit_expr(ctx: _Ctx, build: Callable[[Expr], Stmt], width: Width, depth: int) -> Stmt:
    for _ in range(6):
        stmt = build(_gen_expr(ctx, width, depth))
        if _fits(stmt) and _commit(ctx, stmt):
            return stmt
    stmt = build(Lit(width, ctx.rng.randint(*bounds(width))))
    _commit(ctx, stmt)
    return stmt


def _fill_array(ctx: _Ctx, decl: Decl, counter: str, depth: int) -> Stmt:
    # every element written exactly once, by the counter that is its own
    # subscript -- an array left at BASIC's own zeroes folds most of what
    # later reads it to nothing
    ctx.env.scalars[counter] = 0
    ctx.frozen.add(counter)
    loop = _commit_expr(ctx, _fill_of(decl.name, counter), decl.width, depth)
    ctx.frozen.discard(counter)
    return loop


def generate_program(
    seed: int,
    n_int: int = 4,
    n_lng: int = 4,
    n_sng: int = 2,
    n_dbl: int = 2,
    # One of each width. It was 2, and the widths are handed out in the
    # order INT, LNG, SNG, DBL -- so a float array was never once generated
    # and float array access went entirely uncovered. That is where the
    # miscompile bench/fpbench.bas found was living.
    n_arrays: int = 4,
    n_subs: int = 2,
    n_funcs: int = 2,
    n_stmts: int = 20,
    depth: int = 3,
) -> Program:
    rng = random.Random(seed)
    ids = _Ids()
    declared: dict[str, Width] = {}
    ctx = _Ctx(
        rng=rng,
        ids=ids,
        env=Env({}, {}, {}, []),
        declared=declared,
        readable=_by_width(),
        writable=_by_width(),
        arrays=_by_width(),
        writable_arrays=_by_width(),
        byref=_by_width(),
    )
    decls: list[Decl] = []
    stmts: list[Stmt] = []
    shared: list[tuple[str, Width]] = []

    for width, count in ((Width.INT, n_int), (Width.LNG, n_lng), (Width.SNG, n_sng), (Width.DBL, n_dbl)):
        for i in range(count):
            name = _fresh(ids, "v")
            # exactly one scalar of each width is what a procedure can reach
            # by name; the rest are the only ones a CALL may pass BYREF, which
            # is what keeps copy-in/copy-out equal to real aliasing
            is_shared = i == 0
            decls.append(Decl(name, width, is_shared, None))
            declared[name] = width
            ctx.readable[width].append(name)
            ctx.writable[width].append(name)
            if is_shared:
                shared.append((name, width))
            else:
                ctx.byref[width].append(name)
            value = rng.randint(*bounds(width))
            ctx.env.scalars[name] = 0
            stmts.append(Assign(name, Lit(width, value)))
            ctx.env.scalars[name] = value
    ctx.shared = tuple(shared)

    spread = (Width.INT, Width.LNG, Width.SNG, Width.DBL)
    for i in range(n_arrays):
        width = spread[i % len(spread)]
        name = _fresh(ids, "arr")
        decls.append(Decl(name, width, True, ARRAY_LIMIT))
        declared[name] = width
        ctx.arrays[width].append(name)
        ctx.writable_arrays[width].append(name)
        ctx.env.arrays[name] = [0] * (ARRAY_LIMIT + 1)

    counter = ctx.byref[Width.INT][-1]
    for decl in (d for d in decls if d.size is not None):
        stmts.append(_fill_array(ctx, decl, counter, 2))

    procs: list[Proc] = []
    for kind, count in ((ProcKind.SUB, n_subs), (ProcKind.FUNCTION, n_funcs)):
        for _ in range(count):
            proc = _gen_proc(ctx, kind, rng.randint(2, 4), depth - 1)
            procs.append(proc)
            ctx.env.procs[proc.name] = proc
            if proc.kind is ProcKind.SUB:
                ctx.subs += (proc,)
            else:
                ctx.funcs += (proc,)

    stmts += _gen_body(ctx, n_stmts, depth)
    stmts.append(Done())
    return Program(tuple(decls), tuple(procs), tuple(stmts), declared)
