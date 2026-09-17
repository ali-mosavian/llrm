"""One procedure's trees as a MirBody.

Every C variable is a frame cell and every tree node a fresh value, so the
body is in SSA by construction and promotion is left to the passes. What an
idiom is -- a far pointer's two halves, a call's convention, where a result
arrives -- is decided here, which is rule 5's place for it.

The ABI is Borland's medium model, which is what the rest of qcport is
built with: C procedures far and cdecl, results in AX or DX:AX, SI, DI, BP
and DS preserved, SS equal to DGROUP.
"""

import math
import struct
from dataclasses import field
from dataclasses import replace
from dataclasses import dataclass

from qbopt.model import ir
from qbopt.model import mir
from qbopt.cfront import hir
from qbopt.abi import runtime
from qbopt.model import memory
from qbopt.model import floating
from qbopt.cfront.hir import Unsupported
from qbopt.objectfile.module import Addr
from qbopt.objectfile.module import Space

WIDTHS = {
    "TY_UINT_1": 1,
    "TY_INT_1": 1,
    "TY_UINT_2": 2,
    "TY_INT_2": 2,
    "TY_UINT_4": 4,
    "TY_INT_4": 4,
    "TY_UINT_8": 8,
    "TY_INT_8": 8,
    "TY_INTEGER": 2,
    "TY_UNSIGNED": 2,
    "TY_BOOLEAN": 2,
    "TY_NEAR_POINTER": 2,
    "TY_NEAR_CODE_PTR": 2,
    "TY_LONG_POINTER": 4,
    "TY_HUGE_POINTER": 4,
    "TY_LONG_CODE_PTR": 4,
    # A float moves as its bits; only arithmetic and conversion need the x87.
    "TY_SINGLE": 4,
    "TY_DOUBLE": 8,
}
FLOATS = frozenset({"TY_SINGLE", "TY_DOUBLE"})
FLOAT_ARITHMETIC = {
    "O_PLUS": mir.Kind.FADD,
    "O_MINUS": mir.Kind.FSUB,
    "O_TIMES": mir.Kind.FMUL,
    "O_DIV": mir.Kind.FDIV,
}
FLOAT_UNARY = {"O_UMINUS": mir.Kind.FNEG, "O_FABS": mir.Kind.FABS}
# Operators OW's front end has a node for and Borland's library a routine.
LIBRARY = {
    "O_SQRT": "sqrt", "O_COS": "cos", "O_SIN": "sin", "O_TAN": "tan", "O_ACOS": "acos", "O_ASIN": "asin",
    "O_ATAN": "atan", "O_LOG": "log", "O_LOG10": "log10", "O_EXP": "exp", "O_POW": "pow", "O_ATAN2": "atan2",
    "O_FMOD": "fmod",
}  # fmt: skip
# Borland's pseudo-function laying its constant arguments down as code.
EMITTED = frozenset({"__emit__"})
EXTENDED = floating.Format.EXTENDED80
FORMATS = {4: floating.Format.BINARY32, 8: floating.Format.BINARY64}
INTEGER_FORMATS = {2: floating.Format.SIGNED16, 4: floating.Format.SIGNED32, 8: floating.Format.SIGNED64}
# What each float operation computes, which MIR states and lowering spells.
ARITH_RULE = floating.Semantics((EXTENDED, EXTENDED), EXTENDED, floating.Precision.DYNAMIC, floating.Rounding.DYNAMIC)
EXACT_UNARY = floating.Semantics((EXTENDED,), EXTENDED, floating.Precision.EXACT, floating.Rounding.NONE)


def _loaded(source: floating.Format) -> floating.Semantics:
    return floating.Semantics((source,), EXTENDED, floating.Precision.EXACT, floating.Rounding.NONE)


def _stored(result: floating.Format) -> floating.Semantics:
    return floating.Semantics((EXTENDED,), result, floating.Precision.DESTINATION, floating.Rounding.DYNAMIC)


def _packed(value: float, width: int) -> bytes:
    return struct.pack("<f" if width == 4 else "<d", value)


SIGNED = frozenset({"TY_INT_1", "TY_INT_2", "TY_INT_4", "TY_INT_8", "TY_INTEGER"})
FAR_POINTERS = frozenset({"TY_LONG_POINTER", "TY_HUGE_POINTER"})
POINTERS = frozenset({"TY_POINTER", "TY_NEAR_POINTER", "TY_LONG_POINTER", "TY_HUGE_POINTER"})
# Standard allocation contracts. These names are language-library semantics,
# not fixture recognition; wrappers acquire the same fact through future
# return summaries rather than by adding their names here.
FRESH_ALLOCATORS = frozenset({"malloc", "_malloc", "calloc", "_calloc"})
# C's aliasing classes: a declared object is read and written only through its own, or a character type.
CLASSES = {
    "TY_INT_2": "int2", "TY_UINT_2": "int2", "TY_INTEGER": "int2", "TY_UNSIGNED": "int2",
    "TY_INT_4": "int4", "TY_UINT_4": "int4", "TY_INT_8": "int8", "TY_UINT_8": "int8",
    "TY_SINGLE": "float4", "TY_DOUBLE": "float8",
    "TY_NEAR_POINTER": "pointer2", "TY_LONG_POINTER": "pointer4", "TY_HUGE_POINTER": "pointer4",
}  # fmt: skip
# What a callee reads and writes: no byte this body can name, bounded later
# to miss a frame whose address never escapes.
CALLEE = (mir.MemRef(None, 4),)

# Literal labels (a back handle with no symbol) share the symbol index space;
# float constants the raise itself places come after them.
LITERAL = 1 << 20
POOL = 1 << 21
# Space.GROUP index of the selector of the segment symbol n is in; 0 is DGROUP's.
SELECTOR = 1 << 22

K = mir.Kind
TESTS = {  # (signed, unsigned)
    "O_EQ": (K.EQ, K.EQ),
    "O_NE": (K.NE, K.NE),
    "O_LT": (K.LT, K.BELOW),
    "O_LE": (K.LE, K.BELOW_EQ),
    "O_GT": (K.GT, K.ABOVE),
    "O_GE": (K.GE, K.ABOVE_EQ),
}
INVERSE = {
    K.EQ: K.NE, K.NE: K.EQ, K.LT: K.GE, K.GE: K.LT, K.LE: K.GT, K.GT: K.LE,
    K.BELOW: K.ABOVE_EQ, K.ABOVE_EQ: K.BELOW, K.BELOW_EQ: K.ABOVE, K.ABOVE: K.BELOW_EQ,
}  # fmt: skip
SWAPPED = {
    K.EQ: K.EQ, K.NE: K.NE, K.LT: K.GT, K.GT: K.LT, K.LE: K.GE, K.GE: K.LE,
    K.BELOW: K.ABOVE, K.ABOVE: K.BELOW, K.BELOW_EQ: K.ABOVE_EQ, K.ABOVE_EQ: K.BELOW_EQ,
}  # fmt: skip
ARITHMETIC = {
    "O_PLUS": (K.ADD, True),
    "O_MINUS": (K.SUB, False),
    "O_AND": (K.AND, True),
    "O_OR": (K.OR, True),
    "O_XOR": (K.XOR, True),
    "O_TIMES": (K.MUL, True),
}


# Addresses the raise holds before any of them is a value.
@dataclass(frozen=True, slots=True)
class Frame:
    disp: int
    declared: str | None = field(default=None, compare=False)  # the aliasing class of the scalar it names
    volatile: bool = field(default=False, compare=False)


@dataclass(frozen=True, slots=True)
class Global:
    space: Space
    index: int
    disp: int = 0
    base: mir.Value | None = None  # an index into the symbol, `_arr[j]`
    declared: str | None = field(default=None, compare=False)
    volatile: bool = field(default=False, compare=False)


@dataclass(frozen=True, slots=True)
class Near:
    base: mir.Value
    disp: int = 0
    # The source-language storage whose default selector is required.  A
    # frame-derived near pointer is an SS address even after arithmetic has
    # moved it out of BP-relative form; ordinary near pointers remain DS.
    space: Space = Space.LITERAL
    volatile: bool = field(default=False, compare=False)


@dataclass(frozen=True, slots=True)
class Far:
    segment: mir.Value
    offset: mir.Value
    disp: int = 0
    whole: mir.Value | None = None  # the 4-byte pointer these halves were split from, at disp 0
    named: int = 0  # the selector symbol when the segment is a named object's own, else 0
    declared: str | None = field(default=None, compare=False)
    volatile: bool = field(default=False, compare=False)


type Address = Frame | Global | Near | Far
type Operand = mir.Held | mir.Const


@dataclass(frozen=True, slots=True)
class Aggregate:
    address: Address
    size: int


@dataclass(frozen=True, slots=True)
class Returned:
    low: mir.Value
    high: mir.Value


@dataclass(frozen=True, slots=True)
class Restricted:
    """The lvalue of a pointer declared restrict, before it is loaded."""

    value: object
    root: object


@dataclass(frozen=True, slots=True)
class Function:
    symbol: hir.Symbol


@dataclass
class _Block:
    key: str
    ops: list = field(default_factory=list)
    succ: list = field(default_factory=list)
    ended: bool = False


@dataclass(frozen=True, slots=True)
class Raised:
    name: str
    symbol: hir.Symbol
    body: mir.MirBody
    hints: mir.AllocationHints
    calls: dict[int, str]  # call site -> callee object name
    callees: dict[int, hir.Symbol]
    contracts: dict[int, runtime.Contract]
    # Source-order actual pointer provenance: Provenance, (value, offset), or None.
    arguments: dict[int, tuple[object, ...]]
    # Source-order scalar constants, kept outside MIR for module specialization.
    constants: dict[int, tuple[mir.Const | None, ...]]
    # Entry cells occupied by source parameters, in declaration order.
    parameters: tuple[mir.MemRef, ...]
    # A site whose callee is inline assembly: its bytes, and (kind, name, offset) where a symbol goes.
    inline: dict[int, tuple]


@dataclass(frozen=True, slots=True)
class Real:
    """A float literal: its bits where it is moved, the x87's where it is computed."""

    value: float
    width: int = 4


@dataclass(frozen=True, slots=True)
class FloatCell:
    """A float in memory, not yet read: moved as bytes or loaded onto the x87."""

    address: object
    width: int = 4


@dataclass
class Shared:
    """What a module's procedures raise into together."""

    literals: dict[bytes, int] = field(default_factory=dict)  # a constant's bits -> its number
    runtime: dict[str, hir.Symbol] = field(default_factory=dict)  # routines no declaration names


def raised(unit: hir.Unit, proc: hir.Proc, shared: Shared | None = None) -> Raised:
    return _Raise(unit, proc, shared or Shared()).run()


def names(unit: hir.Unit, shared: Shared | None = None) -> dict[tuple[Space, int], str]:
    """Every (space, index) a raised operand can name, as its object name."""
    out = {(_space(one), one.id): one.object_name for one in unit.symbols.values()}
    out[(Space.GROUP, 0)] = "DGROUP"
    out.update(
        {
            (Space.GROUP, SELECTOR + one.id): f"seg {one.object_name}"
            for one in unit.symbols.values()
            if not unit.grouped(one)
        }
    )
    out.update({(Space.SEGMENT, LITERAL + back): f"L_b{back}" for back, symbol in unit.backs.items() if not symbol})
    out.update({(Space.SEGMENT, POOL + n): f"L_f{n}" for n in (shared.literals.values() if shared else ())})
    return out


def _space(symbol: hir.Symbol) -> Space:
    return Space.EXTERNAL if symbol.imported else Space.SEGMENT


def _even(n: int) -> int:
    return n + (n & 1)


class _Raise:
    def __init__(self, unit: hir.Unit, proc: hir.Proc, shared: Shared) -> None:
        self.unit = unit
        self.proc = proc
        self.shared = shared
        self.symbol = unit.symbols[proc.symbol]
        if not self.symbol.call_class & hir.CALLER_POPS or self.symbol.call_class & hir.REVERSE_PARMS:
            raise Unsupported(f"{self.symbol.name}: only cdecl procedures are defined")
        self.values = 0
        self.at = 0
        self.anonymous = 0
        self.blocks: list[_Block] = []
        self.current: _Block | None = None
        self.done: dict[int, object] = {}
        self.origin: dict[mir.Value, int] = {}
        self.pins: dict[mir.Value, int] = {}
        self.calls: dict[int, str] = {}
        self.callees: dict[int, hir.Symbol] = {}
        self.contracts: dict[int, runtime.Contract] = {}
        self.arguments: dict[int, tuple[object, ...]] = {}
        self.constants: dict[int, tuple[mir.Const | None, ...]] = {}
        self.inline: dict[int, tuple] = {}
        self.frame: dict[str, int] = {}
        self.objects: list[tuple[int, int]] = []  # each frame object's bytes
        self.pointer_values: set[mir.Value] = set()
        self.pointer_seeds: dict[mir.Value, memory.Provenance] = {}
        self.parameter_at: dict[int, int] = {}
        self.selects: dict[str, tuple[list[tuple[int, str]], str | None]] = {}
        at = 6 if self.symbol.far else 4
        for number, (symbol, type_) in enumerate(proc.parms):
            self.frame[f"y{symbol}"] = at
            self.parameter_at[at] = number
            size = _even(max(2, self.size(type_)))
            self.objects.append((at, at + size))
            at += size
        self.down = 0
        for key, type_ in proc.autos:
            self.frame[key] = self.slot(self.size(type_))

    def slot(self, size: int) -> int:
        """A new frame cell below the last."""
        self.down -= _even(size)
        self.objects.append((self.down, self.down + _even(size)))
        return self.down

    def extent(self, disp: int) -> tuple[int, int] | None:
        """The frame object holding `disp`: C keeps an address into an object inside it."""
        return next(((low, high) for low, high in self.objects if low <= disp < high), None)

    # ---- types ----

    def width(self, type_: str) -> int:
        type_ = self.unit.canonical_type(type_)
        if type_ == "TY_POINTER":
            return 4 if self.unit.target & hir.BIG_DATA else 2
        if type_ == "TY_CODE_PTR":
            return 4 if self.unit.target & hir.BIG_CODE else 2
        if type_ in WIDTHS:
            return WIDTHS[type_]
        raise Unsupported(f"{self.symbol.name}: no scalar width for {type_}")

    def size(self, type_: str) -> int:
        type_ = self.unit.canonical_type(type_)
        return self.unit.types[type_] if type_ in self.unit.types else self.width(type_)

    def far_pointer(self, type_: str) -> bool:
        type_ = self.unit.canonical_type(type_)
        return type_ in FAR_POINTERS or (type_ == "TY_POINTER" and bool(self.unit.target & hir.BIG_DATA))

    # ---- blocks and operations ----

    def fresh(self, flags: bool = False) -> mir.Value:
        self.values += 1
        return mir.Value(self.values, self.at + 1, flags=flags, variable=self.values, version=1)

    def start(self, key: str) -> None:
        if self.current is not None and not self.current.ended:
            self.current.succ.append(key)
        self.current = _Block(key)
        self.blocks.append(self.current)
        self.op(K.NOTHING)

    def label(self) -> str:
        self.anonymous += 1
        return f"a{self.anonymous}"

    def end(self, *succ: str) -> None:
        self.current.succ.extend(succ)
        self.current.ended = True

    def op(self, kind, results=(), args=(), *, defines=None, uses=None, **extra) -> mir.Op:
        if self.current is None or self.current.ended:
            self.start(self.label())
        self.at += 1
        if defines is None:
            defines = tuple(one.value for one in results if isinstance(one, mir.Held))
        if uses is None:
            read = [one.value for one in args if isinstance(one, mir.Held)]
            read += [
                part
                for one in (*args, *results)
                if isinstance(one, mir.Cell)
                for part in (one.ref.base, one.ref.segment)
                if part is not None
            ]
            uses = tuple(dict.fromkeys(read))
        references = (*extra.get("loads", ()), *extra.get("stores", ()))
        observable = any(ref.volatile for ref in references)
        if observable:
            # The explicit references are the complete footprint; BARRIER
            # prevents elimination and motion without pretending a volatile
            # read writes all memory or a volatile store reads all memory.
            extra["memory_complete"] = True
            extra["reads_complete"] = True
        made = mir.Op(
            self.at,
            ir.Operation.BARRIER if observable else ir.Operation.NOTHING,
            "",
            defines,
            uses,
            kind=kind,
            args=args,
            results=results,
            id=next(mir._IDS),
            **extra,
        )
        self.current.ops.append(made)
        return made

    def run(self) -> Raised:
        self.start("entry")
        for one in self.proc.body:
            self.statement(one)
        if not self.current.ended:
            raise Unsupported(f"{self.symbol.name}: control reaches the end with no return")
        reached, queue = set(), ["entry"]
        by_key = {block.key: block for block in self.blocks}
        while queue:
            key = queue.pop()
            if key not in reached:
                reached.add(key)
                queue.extend(by_key[key].succ)
        kept = [block for block in self.blocks if block.key in reached]
        at = {block.key: block.ops[0].at for block in kept}
        blocks = tuple(
            mir.MirBlock(
                at[block.key],
                (),
                tuple(
                    replace(op, target=at[op.target], cases=tuple((n, at[label]) for n, label in op.cases))
                    if isinstance(op.target, str)
                    else op
                    for op in block.ops
                ),
                tuple(at[key] for key in block.succ),
            )
            for block in kept
        )
        body = mir._RaisedBody(
            blocks[0].at,
            blocks,
            origin=dict(self.origin),
            pins=dict(self.pins),
            sealed=True,
            pointer_values=frozenset(self.pointer_values),
            pointer_seeds=dict(self.pointer_seeds),
        )
        body = mir._frame_bounded(body, pointers=True)
        from qbopt.analysis import alias

        body = alias.annotated(body)
        # Even an unknown C callee has a precise language-level boundary: it
        # can reach nonlocal storage, pointer actuals and frame objects that
        # escaped before the call, but not every byte of this activation.
        body = alias.calls_annotated(alias.Procedure(body, self.calls, self.arguments), {})
        body = mir._with_live_outs(body)
        problems = mir.verify(body)
        if problems:
            raise Unsupported(f"{self.symbol.name}: raised MIR is not SSA: {problems[:3]}")
        hints = mir.AllocationHints.from_body(body)
        return Raised(
            self.symbol.object_name,
            self.symbol,
            mir._public(body),
            hints,
            self.calls,
            self.callees,
            self.contracts,
            self.arguments,
            self.constants,
            tuple(self.placed(Frame(self.frame[f"y{symbol}"]), self.size(type_)) for symbol, type_ in self.proc.parms),
            self.inline,
        )

    # ---- statements ----

    def statement(self, one: hir.Statement) -> None:
        match one.call, one.args:
            case "CGDone" | "CGTrash", (node,):
                self.eval(node)
            case "CGControl", ("O_LABEL", _, label):
                self.start(label)
            case "CGControl", ("O_GOTO", _, label):
                self.op(K.JUMP, target=label)
                self.end(label)
            case "CGControl", ("O_IF_TRUE" | "O_IF_FALSE" as test, node, label):
                self.branch(node, label, test == "O_IF_TRUE")
            case "CGReturn", (node, type_):
                self.ret(node, type_)
            case "CGSelInit", (select,):
                self.selects[select] = ([], None)
            case "CGSelCase", (select, label, value):
                self.selects[select][0].append((int(value), label))
            case "CGSelOther", (select, label):
                self.selects[select] = (self.selects[select][0], label)
            case "CGSelect", (select, node) if self.selects[select][1] is not None:
                cases, other = self.selects.pop(select)
                type_ = self.type_of(node)
                value = self.narrowed(self.operand(self.eval(node), type_), max(2, self.width(type_)))
                self.op(K.SWITCH, args=(value,), target=other, cases=tuple(cases))
                self.end(*dict.fromkeys((other, *(label for _, label in cases))))
            case _:
                raise Unsupported(f"{self.symbol.name} line {one.line}: {one.call} {' '.join(one.args)}")

    def ret(self, node: str, type_: str) -> None:
        returned: tuple[mir.Value, ...] = ()
        if node != "n0" and type_ in FLOATS:
            value = self.floating(self.eval(node))
            self.op(K.RETURN, args=(value,), uses=(value.value,), reads_complete=True)
            self.end()
            return
        if node != "n0" and self.width(type_) == 8:
            value = self.operand(self.eval(node), type_)
            if isinstance(value, mir.Const):
                value = mir.Held(self.copy(value), 8)
            value = self.narrowed(value, 8)
            self.op(K.RETURN, args=(value,), uses=(value.value,), reads_complete=True)
            self.end()
            return
        last = self.current.ops[-1] if self.current is not None and self.current.ops else None
        if node == "n0" and type_ in WIDTHS and last is not None and last.kind is K.CALL and len(last.results) == 2:
            # A value-less return from a function that has one: the value is
            # what the call before it left, as inline assembly means it to be.
            got = Returned(last.results[0].value, last.results[1].value)
            node = None
        if node != "n0":
            got = self.eval(node) if node is not None else got
            if isinstance(got, Far):
                offset = mir.Held(self.near(Near(got.offset, got.disp)), 2)
                returned = (self.copy(offset), self.copy(mir.Held(got.segment, 2)))
            elif self.width(type_) == 4:
                whole = self.operand(got, type_)
                if isinstance(whole, mir.Const):
                    low, high = mir.Const(whole.n & 0xFFFF, 2), mir.Const((whole.n >> 16) & 0xFFFF, 2)
                else:
                    shifted = self.fresh()
                    self.op(K.SHR, (mir.Held(shifted, 4),), (whole, mir.Const(16, 1)))
                    low, high = mir.Held(whole.value, 2), mir.Held(shifted, 2)
                returned = (self.copy(low), self.copy(high))
            else:
                returned = (self.copy(self.narrowed(self.operand(got, type_), 2)),)
        self.op(K.RETURN, args=tuple(mir.Held(one, 2) for one in returned), uses=returned, reads_complete=True)
        self.end()

    def branch(self, node: str, label: str, when: bool) -> None:
        """Go to `label` when `node` is `when`; fall through otherwise."""
        tree = self.unit.nodes[hir.handle(node)]
        match tree.call, tree.args:
            case "CGCompare", _:
                flags, test = self.compare(tree)
                self.jump_if(flags, test if when else INVERSE[test], label)
            case "CGFlow", ("O_FLOW_NOT", inner, _):
                self.branch(inner, label, not when)
            case "CGFlow", ("O_FLOW_AND" | "O_FLOW_OR" as flow, left, right):
                if (flow == "O_FLOW_OR") == when:
                    self.branch(left, label, when)
                    self.branch(right, label, when)
                else:
                    skip = self.label()
                    self.branch(left, skip, not when)
                    self.branch(right, label, when)
                    self.start(skip)
            case _:
                value = self.operand(self.eval(node), "TY_INTEGER")
                flags = self.fresh(flags=True)
                width = value.width if isinstance(value, mir.Held) else 2
                self.op(K.SUB, (), (value, mir.Const(0, width)), defines=(flags,))
                self.jump_if(flags, K.NE if when else K.EQ, label)

    def compare(self, tree: hir.Node) -> tuple[mir.Value, mir.Kind]:
        cg_op, left, right, type_ = tree.args
        if type_ in FLOATS:
            x = self.floating(self.convert(self.eval(left), self.type_of(left), type_))
            y = self.floating(self.convert(self.eval(right), self.type_of(right), type_))
            flags = self.fresh(flags=True)
            self.op(K.FCOMPARE, (), (x, y), defines=(flags,))
            return flags, TESTS[cg_op][0]
        width = max(2, self.width(type_))
        a = self.narrowed(self.coerced(self.eval(left), left, type_), width)
        b = self.narrowed(self.coerced(self.eval(right), right, type_), width)
        test = TESTS[cg_op][0 if type_ in SIGNED else 1]
        if isinstance(a, mir.Const):
            a, b, test = b, a, SWAPPED[test]
        if isinstance(a, mir.Const):
            a = mir.Held(self.copy(a), width)
        flags = self.fresh(flags=True)
        self.op(K.SUB, (), (a, b), defines=(flags,))
        return flags, test

    def jump_if(self, flags: mir.Value, test: mir.Kind, label: str) -> None:
        self.op(K.BRANCH, uses=(flags,), test=test, target=label)
        fall = self.label()
        self.end(fall, label)
        self.start(fall)

    # ---- expressions ----

    def eval(self, node: str):
        key = hir.handle(node)
        if key not in self.done:
            self.done[key] = self.expression(self.unit.nodes[key], node)
        return self.done[key]

    def expression(self, tree: hir.Node, node: str):
        match tree.call, tree.args:
            case "CGInteger", (value, type_):
                return mir.Const(int(value), max(2, self.width(type_)))
            case "CGInt64", (value, type_):
                return mir.Const(self.wrapped(int(value), type_), 8)
            case "CGFloat", (text, type_) if type_ in FLOATS:
                return self.real(float(text), self.width(type_))
            case "CGFEName", (symbol, type_):
                return self.name(symbol, type_)
            case "CGTempName", (temp, _):
                return Frame(self.frame[temp])
            case "CGBackName", (back, _):
                symbol = self.unit.backs[hir.handle(back)]
                if symbol:
                    return self.name(f"y{symbol}")
                return Global(Space.SEGMENT, LITERAL + hir.handle(back))
            case "CGUnary", ("O_POINTS", inner, type_):
                return self.points(self.eval(inner), type_)
            case "CGUnary", ("O_CONVERT", inner, type_):
                return self.convert(self.eval(inner), self.type_of(inner), type_)
            case "CGUnary", ("O_UMINUS" | "O_COMPLEMENT" | "O_FABS" as cg_op, inner, type_):
                return self.unary(cg_op, self.eval(inner), type_)
            case "CGUnary", (cg_op, inner, type_) if cg_op in LIBRARY:
                return self.library(LIBRARY[cg_op], [(inner, self.eval(inner))], type_)
            case "CGBinary", ("O_COMMA", left, right, _):
                self.eval(left)
                return self.eval(right)
            case "CGBinary", (cg_op, left, right, type_) if cg_op in LIBRARY:
                # The runtime takes them last first, as every call's parms are listed.
                return self.library(LIBRARY[cg_op], [(right, self.eval(right)), (left, self.eval(left))], type_)
            case "CGBinary", (cg_op, left, right, type_):
                return self.binary(cg_op, left, right, type_)
            case "CGAssign", (target, source, type_):
                return self.assign(target, source, type_)
            case "CGLVAssign", (target, source, _):
                return self.aggregate(self.eval(target), self.eval(source))
            case "CGPostGets" | "CGPreGets", (cg_op, target, source, type_) if type_ in FLOATS:
                address = self.address(self.eval(target))
                width = self.width(type_)
                old = self.floating(FloatCell(address, width))
                new = self.float_arithmetic(
                    cg_op, old, self.floating(self.convert(self.eval(source), self.type_of(source), type_))
                )
                self.put_float(address, width, new)
                return old if tree.call == "CGPostGets" else FloatCell(address, width)
            case "CGPostGets" | "CGPreGets", ("O_PLUS" | "O_MINUS" as cg_op, target, source, type_) if self.far_pointer(
                type_
            ):
                address = self.address(self.eval(target))
                old = self.far_loaded(address, type_)
                delta = self.operand(self.eval(source), self.type_of(source))
                new = self.offset(old, delta, cg_op == "O_MINUS")
                self.put_pointer(address, new, type_)
                return old if tree.call == "CGPostGets" else new
            case "CGPostGets" | "CGPreGets", (cg_op, target, source, type_):
                address = self.address(self.eval(target))
                width = self.width(type_)
                old = self.load(self.cell(address, width, type_), type_)
                new = self.arithmetic(cg_op, old, self.coerced(self.eval(source), source, type_), type_)
                self.store(self.cell(address, width, type_), new)
                return old if tree.call == "CGPostGets" else new
            case "CGCall", (call,):
                return self.call(self.unit.calls[hir.handle(call)])
            case "CGChoose", (test, yes, no, type_):
                return self.choose(test, yes, no, type_)
            case "CGCompare" | "CGFlow", _:
                return self.truth(node)
            case "CGEval", (inner,):
                return self.eval(inner)
            case "CGVolatile", (inner,):
                got = self.eval(inner)
                if isinstance(got, (Frame, Global, Near, Far)):
                    return replace(got, volatile=True)
                if isinstance(got, Restricted) and isinstance(got.value, (Frame, Global, Near, Far)):
                    return replace(got, value=replace(got.value, volatile=True))
                raise Unsupported(f"{self.symbol.name}: volatile access through {got}")
            case "CGAttr", (inner, "3"):
                got = self.eval(inner)
                match got:
                    case Frame(disp):
                        root = ("frame", disp)
                    case Global(index=index):
                        root = ("global", index)
                    case Far(named=named) if named:
                        root = ("far", named)
                    case _:
                        root = ("node", hir.handle(inner))
                return Restricted(got, root)
            case "CGAttr", (inner, _):
                return self.eval(inner)
        raise Unsupported(f"{self.symbol.name}: {tree.call} {' '.join(tree.args)}")

    def choose(self, test: str, yes: str, no: str, type_: str):
        if type_ in FLOATS:
            return self.joined(
                test,
                lambda: self.convert(self.eval(yes), self.type_of(yes), type_),
                lambda: self.convert(self.eval(no), self.type_of(no), type_),
                type_,
            )
        return self.joined(
            test,
            lambda: self.coerced(self.eval(yes), yes, type_),
            lambda: self.coerced(self.eval(no), no, type_),
            type_,
        )

    def truth(self, test: str):
        """A compare or flow as a value: 1 or 0."""
        return self.joined(test, lambda: mir.Const(1, 2), lambda: mir.Const(0, 2), "TY_INTEGER")

    def joined(self, test: str, yes, no, type_: str):
        """`test ? yes() : no()`: each arm stores into one frame cell, read after
        the join -- a variable like any other, left for promotion to make SSA."""
        width = self.width(type_) if type_ in FLOATS else max(2, self.width(type_))
        joined = Frame(self.slot(width))

        def put(value) -> None:
            if type_ in FLOATS:
                self.put_float(joined, width, value)
            else:
                self.store(self.cell(joined, width), self.narrowed(value, width))

        otherwise, join = self.label(), self.label()
        self.branch(test, otherwise, False)
        put(yes())
        self.op(K.JUMP, target=join)
        self.end(join)
        self.start(otherwise)
        put(no())
        self.start(join)
        if type_ in FLOATS:
            return FloatCell(joined, width)
        if self.far_pointer(type_):
            return self.far_loaded(joined, type_)
        return self.load(self.cell(joined, width), type_)

    def type_of(self, node: str) -> str:
        tree = self.unit.nodes[hir.handle(node)]
        match tree.call:
            case "CGCall":
                return self.unit.calls[hir.handle(tree.args[0])].type
            case "CGEval" | "CGVolatile" | "CGAttr":
                return self.type_of(tree.args[0])
            case "CGFlow" | "CGCompare":
                return "TY_BOOLEAN"
        return tree.args[-1]

    def coerced(self, got, node: str, type_: str) -> Operand:
        """An operand at the operation's type: the code generator converts operands implicitly."""
        return self.operand(self.convert(got, self.type_of(node), type_), type_)

    def name(self, token: str, type_: str | None = None):
        symbol = self.unit.symbols[hir.handle(token)]
        if symbol.proc:
            return Function(symbol)
        declared = self.aliasing(type_)
        if token in self.frame:
            return Frame(self.frame[token], declared)
        if not self.unit.grouped(symbol):
            segment, offset = self.fresh(), self.fresh()
            self.op(K.COPY, (mir.Held(segment, 2),), (mir.Symbol(Space.GROUP, SELECTOR + symbol.id, 0, 2),))
            self.op(K.COPY, (mir.Held(offset, 2),), (mir.Symbol(_space(symbol), symbol.id, 0, 2),))
            return Far(segment, offset, named=SELECTOR + symbol.id, declared=declared)
        return Global(_space(symbol), symbol.id, declared=declared)

    def aliasing(self, type_: str | None) -> str | None:
        """A scalar type's aliasing class; None for a character, an aggregate or no type, which reach anything."""
        type_ = self.unit.canonical_type(type_) if type_ is not None else None
        if type_ == "TY_POINTER":
            return f"pointer{self.width(type_)}"
        return CLASSES.get(type_)

    def points(self, got, type_: str):
        restricted = got.root if isinstance(got, Restricted) else None
        got = got.value if isinstance(got, Restricted) else got
        if isinstance(got, mir.Held) and got.width == 10:
            return got  # a float call's result, already a value
        if type_ in self.unit.types:
            if isinstance(got, Aggregate):
                if got.size != self.unit.types[type_]:
                    raise Unsupported(
                        f"{self.symbol.name}: {got.size}-byte aggregate used as {self.unit.types[type_]} bytes"
                    )
                return got
            return Aggregate(self.address(got), self.unit.types[type_])
        if isinstance(got, mir.Held) and got.width == self.width(type_) == 8:
            return got  # an int64 call's result is likewise already a whole MIR value
        if isinstance(got, Returned):
            if self.far_pointer(type_):
                return Far(got.high, got.low)
            if self.width(type_) == 4:
                whole = self.fresh()
                self.op(K.CONCAT, (mir.Held(whole, 4),), (mir.Held(got.high, 2), mir.Held(got.low, 2)))
                self.pointer_values.add(whole)
                return mir.Held(whole, 4)
            if self.width(type_) == 8:
                raise Unsupported(f"{self.symbol.name}: a DX:AX result cannot provide an 8-byte value")
            return self.extended(mir.Held(got.low, 2), type_)
        address = self.address(got)
        if type_ in FLOATS:
            return FloatCell(address, self.width(type_))
        if self.far_pointer(type_):
            return self.far_loaded(address, type_)
        loaded = self.load(self.cell(address, self.width(type_), type_), type_)
        if restricted is not None:
            fact = self.pointer_seeds.get(loaded.value, memory.Provenance.one(memory.Object(memory.Kind.UNKNOWN)))
            self.pointer_seeds[loaded.value] = memory.Provenance(fact.slices, frozenset({restricted}))
            self.pointer_values.add(loaded.value)
        return loaded

    def convert(self, got, source: str, type_: str):
        # An address's node is typed by what it addresses: `(void far *) &a_float`.
        if isinstance(got, (Frame, Global, Near)) and self.far_pointer(type_):
            return Far(self.dgroup(), self.near(got))
        if isinstance(got, (Frame, Global, Near, Far)):
            return got
        if source in FLOATS or type_ in FLOATS:
            if source in FLOATS and type_ in FLOATS:
                return self.real(got.value, self.width(type_)) if isinstance(got, Real) else got
            if source in FLOATS:
                return self.truncated(self.floating(got), type_)
            if isinstance(got, mir.Const):
                return self.real(float(self.wrapped(got.n, source)), self.width(type_))
            whole = self.operand(got, source)
            if source == "TY_UINT_8":
                result = self.fresh()
                self.op(
                    K.FLOAD,
                    (mir.Held(result, 10),),
                    (whole,),
                    floating=_loaded(floating.Format.UNSIGNED64),
                )
                return mir.Held(result, 10)
            if source not in SIGNED and self.width(source) == 4:
                # No 32-bit integer load reads it unsigned; its zero extension as a quad does.
                quad = Frame(self.slot(8))
                self.store(self.cell(quad, 4), whole)
                self.store(self.cell(replace(quad, disp=quad.disp + 4), 4), mir.Const(0, 4))
                ref = self.cell(quad, 8)
                result = self.fresh()
                self.op(
                    K.FLOAD,
                    (mir.Held(result, 10),),
                    (mir.Cell(ref),),
                    loads=(ref,),
                    floating=_loaded(floating.Format.SIGNED64),
                )
                return mir.Held(result, 10)
            if self.width(source) == 1 or source not in SIGNED:
                whole = self.convert(whole, source, "TY_INT_4")
            result = self.fresh()
            self.op(K.FLOAD, (mir.Held(result, 10),), (whole,), floating=_loaded(INTEGER_FORMATS[whole.width]))
            return mir.Held(result, 10)
        if isinstance(got, Returned):
            got = self.points(got, source)
            if not isinstance(got, (mir.Held, mir.Const)):
                return got
        to = self.width(type_)
        if isinstance(got, mir.Held) and got.width == 2 and self.far_pointer(type_):
            return Far(self.dgroup(), got.value)
        if isinstance(got, mir.Const):
            return mir.Const(self.wrapped(got.n, type_), max(2, to))
        if to > got.width:
            wide = self.fresh()
            kind = K.SIGN_EXTEND if source in SIGNED else K.ZERO_EXTEND
            self.op(kind, (mir.Held(wide, to),), (mir.Held(got.value, got.width),))
            return mir.Held(wide, to)
        if got.width == 8 and to < 8:
            narrow = self.fresh()
            width = max(2, to)
            self.op(K.EXTRACT, (mir.Held(narrow, width),), (got, mir.Const(0, 1)))
            return self.extended(mir.Held(narrow, width), type_)
        if to == 1:
            return self.extended(mir.Held(got.value, 2), type_)
        if to == 2 and got.width == 4:
            return mir.Held(got.value, 2)
        return got

    def unary(self, cg_op: str, got, type_: str) -> Operand:
        if type_ in FLOATS:
            if cg_op not in FLOAT_UNARY:
                raise Unsupported(f"{self.symbol.name}: float {cg_op}")
            result = self.fresh()
            self.op(FLOAT_UNARY[cg_op], (mir.Held(result, 10),), (self.floating(got),), floating=EXACT_UNARY)
            return mir.Held(result, 10)
        if cg_op == "O_FABS":
            raise Unsupported(f"{self.symbol.name}: O_FABS of {type_}")
        value = self.operand(got, type_)
        width = max(2, self.width(type_))
        if isinstance(value, mir.Const):
            n = -value.n if cg_op == "O_UMINUS" else ~value.n
            return mir.Const(self.wrapped(n, type_), width)
        result = self.fresh()
        kind = K.NEG if cg_op == "O_UMINUS" else K.NOT
        self.op(kind, (mir.Held(result, width),), (self.narrowed(value, width),))
        return self.extended(mir.Held(result, width), type_)

    def binary(self, cg_op: str, left: str, right: str, type_: str):
        a, b = self.eval(left), self.eval(right)
        if type_ in FLOATS:
            x = self.floating(self.convert(a, self.type_of(left), type_))
            y = self.floating(self.convert(b, self.type_of(right), type_))
            return self.float_arithmetic(cg_op, x, y)
        if (
            cg_op in ("O_PLUS", "O_MINUS")
            and type_ in ("TY_POINTER", "TY_NEAR_POINTER")
            and not self.far_pointer(type_)
        ):
            # A loaded near pointer is an address too, so its constant steps fold into cells.
            a, b = (self.loaded(one, node) for one, node in ((a, left), (b, right)))
        if cg_op in ("O_PLUS", "O_MINUS") and isinstance(b, (Frame, Global, Near, Far)) and cg_op == "O_PLUS":
            a, b = b, a
        if isinstance(a, mir.Held) and self.far_pointer(self.type_of(left)):
            a = self.split(a)
        if isinstance(a, (Frame, Global, Near, Far)) and cg_op in ("O_PLUS", "O_MINUS"):
            return self.offset(a, self.operand(b, self.type_of(right)), cg_op == "O_MINUS")
        if cg_op in ("O_LSHIFT", "O_RSHIFT"):
            return self.arithmetic(cg_op, self.coerced(a, left, type_), self.operand(b, self.type_of(right)), type_)
        return self.arithmetic(cg_op, self.coerced(a, left, type_), self.coerced(b, right, type_), type_)

    def offset(self, address: Address, by: Operand, subtract: bool) -> Address:
        # Arithmetic leaves the declared object: what it reaches is an access.
        if not isinstance(address, Near):
            address = replace(address, declared=None)
        if isinstance(by, mir.Const):
            n = -by.n if subtract else by.n
            return replace(address, disp=address.disp + n)
        index = self.narrowed(by, 2)
        if subtract:
            negated = self.fresh()
            self.op(K.NEG, (mir.Held(negated, 2),), (index,))
            index = mir.Held(negated, 2)
        if isinstance(address, Far):
            moved = self.add(mir.Held(address.offset, 2), index)
            self.pointer_values.add(moved)
            return Far(address.segment, moved, address.disp, named=address.named)
        if isinstance(address, Global):
            # The symbol stays named, so its cells alias only the symbol's own.
            moved = index.value if address.base is None else self.add(mir.Held(address.base, 2), index)
            self.pointer_values.add(moved)
            return replace(address, base=moved)
        if isinstance(address, Near):
            moved = self.add(mir.Held(address.base, 2), index)
            self.pointer_values.add(moved)
            return Near(moved, address.disp, address.space)
        # From the object's first byte, so the address says which object it is in.
        extent = self.extent(address.disp)
        start = extent[0] if extent is not None else 0
        base = self.near(replace(address, disp=start))
        moved = self.add(mir.Held(base, 2), index)
        self.pointer_values.add(moved)
        return Near(moved, address.disp - start, Space.FRAME)

    def float_arithmetic(self, cg_op: str, x: mir.Held, y: mir.Held) -> mir.Held:
        if cg_op not in FLOAT_ARITHMETIC:
            raise Unsupported(f"{self.symbol.name}: float {cg_op}")
        result = self.fresh()
        self.op(FLOAT_ARITHMETIC[cg_op], (mir.Held(result, 10),), (x, y), floating=ARITH_RULE)
        return mir.Held(result, 10)

    def add(self, a: mir.Held, b: Operand) -> mir.Value:
        result = self.fresh()
        self.op(K.ADD, (mir.Held(result, a.width),), (a, b))
        return result

    def arithmetic(self, cg_op: str, a: Operand, b: Operand, type_: str) -> Operand:
        if type_ in FLOATS:
            raise Unsupported(f"{self.symbol.name}: float {cg_op}")
        width = max(2, self.width(type_))
        shift = cg_op in ("O_LSHIFT", "O_RSHIFT")
        a, b = self.narrowed(a, width), (b if shift else self.narrowed(b, width))
        signed = type_ in SIGNED
        if isinstance(a, mir.Const) and isinstance(b, mir.Const):
            return mir.Const(self.wrapped(_fold(cg_op, a.n, b.n, signed), type_), width)
        result = self.fresh()
        if cg_op in ("O_DIV", "O_MOD"):
            if isinstance(a, mir.Const):
                a = mir.Held(self.copy(a), width)
            remainder = self.fresh()
            kind = K.DIVMOD if signed else K.UDIVMOD
            self.op(kind, (mir.Held(result, width), mir.Held(remainder, width)), (a, b))
            return mir.Held(result if cg_op == "O_DIV" else remainder, width)
        if shift:
            if isinstance(a, mir.Const):
                a = mir.Held(self.copy(a), width)
            count = mir.Const(b.n, 1) if isinstance(b, mir.Const) else mir.Held(b.value, 1)
            kind = K.SHL if cg_op == "O_LSHIFT" else (K.SAR if signed else K.SHR)
            self.op(kind, (mir.Held(result, width),), (a, count))
            return self.extended(mir.Held(result, width), type_)
        if cg_op not in ARITHMETIC:
            raise Unsupported(f"{self.symbol.name}: {cg_op}")
        kind, commutes = ARITHMETIC[cg_op]
        if isinstance(a, mir.Const) and commutes:
            a, b = b, a
        if isinstance(a, mir.Const):
            a = mir.Held(self.copy(a), width)
        self.op(kind, (mir.Held(result, width),), (a, b))
        return self.extended(mir.Held(result, width), type_)

    def assign(self, target: str, source: str, type_: str) -> Operand:
        value = self.eval(source)
        address = self.address(self.eval(target))
        width = self.width(type_)
        if isinstance(value, Far):
            self.put_pointer(address, value, type_)
            return value
        if type_ in FLOATS:
            return self.put_float(address, width, self.convert(value, self.type_of(source), type_))
        operand = self.coerced(value, source, type_)
        self.store(self.cell(address, width, type_), operand)
        return operand

    def put_pointer(self, address: Address, value: Far, type_: str) -> None:
        """Store a far pointer without turning its selector and offset into integer arithmetic."""
        if value.whole is not None and value.disp == 0:
            self.store(self.cell(address, 4, type_), mir.Held(value.whole, 4))
            return
        self.store(self.cell(address, 2, type_), self.near(Near(value.offset, value.disp)))
        self.store(self.cell(replace(address, disp=address.disp + 2), 2, type_), mir.Held(value.segment, 2))

    def put_float(self, address: Address, width: int, got):
        """A float into a cell of `width` bytes, and the value the assignment is."""
        match got:
            case Real(value=value):
                packed = _packed(value, width)
                for at in range(0, width, 4):
                    bits = int.from_bytes(packed[at : at + 4], "little", signed=True)
                    self.store(self.cell(replace(address, disp=address.disp + at), 4), mir.Const(bits, 4))
                return Real(value, width)
            case FloatCell(address=source, width=size) if size == width:
                for at in range(0, width, 4):
                    moved = self.load(self.cell(replace(source, disp=source.disp + at), 4), "TY_UINT_4")
                    self.store(self.cell(replace(address, disp=address.disp + at), 4), moved)
                return FloatCell(address, width)
        # fstp pops what it stores, so the assignment's own value is the cell.
        ref = self.cell(address, width)
        self.op(K.FSTORE, (mir.Cell(ref),), (self.floating(got),), stores=(ref,), floating=_stored(FORMATS[width]))
        return FloatCell(address, width)

    def aggregate(self, target, source) -> Aggregate:
        if not isinstance(source, Aggregate):
            raise Unsupported(f"{self.symbol.name}: aggregate assignment from {source}")
        into = self.address(target)
        done = 0
        while done < source.size:
            width = 4 if source.size - done >= 4 else (2 if source.size - done >= 2 else 1)
            moved = self.fresh()
            got = self.cell(replace(source.address, disp=source.address.disp + done), width)
            self.op(K.LOAD, (mir.Held(moved, width),), (mir.Cell(got),), loads=(got,))
            put = self.cell(replace(into, disp=into.disp + done), width)
            self.op(K.STORE, (mir.Cell(put),), (mir.Held(moved, width),), stores=(put,))
            done += width
        return Aggregate(into, source.size)

    def call(self, call: hir.Call):
        target = self.eval(call.target)
        if isinstance(target, Function) and target.symbol.name in EMITTED:
            values = [self.eval(node) for node, _ in reversed(call.parms)]
            if not all(isinstance(one, mir.Const) and 0 <= one.n < 256 for one in values):
                raise Unsupported(f"{self.symbol.name}: {target.symbol.name} of anything but constant bytes")
            return self.inline_code(replace(target.symbol, code=hir.Code(bytes(one.n for one in values), ())))
        if isinstance(target, Function):
            callee, indirect = target.symbol, None
        else:
            callee, indirect = self.unit.symbols[call.symbol], target
            if not isinstance(indirect, mir.Held) or indirect.width != 2 or callee.far:
                raise Unsupported(f"{self.symbol.name}: indirect call through anything but a near code pointer")
        return self.invoke(
            callee,
            [(self.eval(node), type_) for node, type_ in call.parms],
            call.type,
            indirect=indirect,
        )

    def library(self, name: str, arguments: list, type_: str):
        """A C runtime routine taking and returning doubles, for an operator."""
        callee = self.shared.runtime.get(name)
        if callee is None:
            attr = hir.FE_PROC | hir.FE_IMPORT
            callee = hir.Symbol(-1 - len(self.shared.runtime), name, name, "_*", attr, hir.CALLER_POPS, hir.FAR_CALL)
            self.shared.runtime[name] = callee
        doubles = [(self.convert(value, self.type_of(node), "TY_DOUBLE"), "TY_DOUBLE") for node, value in arguments]
        return self.convert(self.invoke(callee, doubles, "TY_DOUBLE"), "TY_DOUBLE", type_)

    def invoke(self, callee: hir.Symbol, arguments: list, type_: str, *, indirect: mir.Held | None = None):
        """A call, with `arguments` last first; a float result arrives on the x87."""
        if callee.code is not None:
            if arguments or type_ in FLOATS:
                raise Unsupported(f"{self.symbol.name}: inline code taking arguments or giving a float")
            return self.inline_code(callee)
        # Stack arguments only: cdecl (caller pops) or pascal (reversed, callee pops).
        stacked = callee.call_class & (hir.CALLER_POPS | hir.REVERSE_PARMS) in (hir.CALLER_POPS, hir.REVERSE_PARMS)
        if callee.register_parms or not stacked:
            raise Unsupported(f"{self.symbol.name}: {callee.object_name} has a register calling convention")
        source_arguments = tuple(reversed(arguments))
        if callee.call_class & hir.REVERSE_PARMS:
            arguments = arguments[::-1]
        pushed = sum(self.push(value, type_) for value, type_ in arguments)
        call_args = (indirect,) if indirect is not None else ()
        call_uses = (indirect.value,) if indirect is not None else ()
        call_extra = {"symbol": False, "indirect": True} if indirect is not None else {}
        if type_ in FLOATS:
            result = self.fresh()
            site = self.op(
                K.CALL,
                (mir.Held(result, 10),),
                call_args,
                defines=(result,),
                uses=call_uses,
                loads=CALLEE,
                stores=CALLEE,
                memory_complete=True,
                reads_complete=True,
                **call_extra,
            )
            returned = mir.Held(result, 10)
        elif self.width(type_) == 8:
            result = self.fresh()
            site = self.op(
                K.CALL,
                (mir.Held(result, 8),),
                call_args,
                defines=(result,),
                uses=call_uses,
                loads=CALLEE,
                stores=CALLEE,
                memory_complete=True,
                reads_complete=True,
                **call_extra,
            )
            returned = mir.Held(result, 8)
        else:
            low, high = self.fresh(), self.fresh()
            site = self.op(
                K.CALL,
                (mir.Held(low, 2), mir.Held(high, 2)),
                call_args,
                defines=(low, high),
                uses=call_uses,
                loads=CALLEE,
                stores=CALLEE,
                memory_complete=True,
                reads_complete=True,
                **call_extra,
            )
            returned = Returned(low, high)
        caller_pops = bool(callee.call_class & hir.CALLER_POPS)
        self.calls[site.at] = callee.object_name
        if indirect is None:
            self.callees[site.at] = callee
        self.arguments[site.at] = tuple(self._call_actual(value, arg_type) for value, arg_type in source_arguments)
        self.constants[site.at] = tuple(self._call_constant(value, arg_type) for value, arg_type in source_arguments)
        canonical = self.unit.canonical_type(type_)
        if canonical in POINTERS and isinstance(returned, Returned):
            self.pointer_values.add(returned.low)
            name = callee.object_name
            if name in FRESH_ALLOCATORS:
                extent = self._allocation_extent(name, source_arguments)
                object_ = memory.Object(memory.Kind.ALLOCATION, (name, site.at), generation=site.at, extent=extent)
                self.pointer_seeds[returned.low] = memory.Provenance.one(object_, 0, 1)
            else:
                self.pointer_seeds[returned.low] = memory.Provenance.one(memory.Object(memory.Kind.UNKNOWN))
        self.contracts[site.at] = runtime.Contract(
            name=callee.object_name,
            cleanup=0 if caller_pops else pushed,
            control=runtime.Control.RETURNS,
            enters_user_code=False,
            raises_error=False,
            error_handling=False,
            writes=runtime.Memory.ANY,
            reads=runtime.Memory.ANY,
            clobbers=frozenset(
                {runtime.Reg.AX, runtime.Reg.BX, runtime.Reg.CX, runtime.Reg.DX, runtime.Reg.ES, runtime.Reg.FLAGS}
            ),
            established=True,
            evidence="Borland medium model: stack arguments, result in AX or DX:AX; SI, DI, BP and DS kept as 16-bit registers",
            i386=True,
            inputs=frozenset(),
            caller_cleanup=pushed if caller_pops else 0,
        )
        return returned

    def _call_constant(self, value, type_: str) -> mir.Const | None:
        """A scalar actual known without inspecting the callee's ABI."""
        type_ = self.unit.canonical_type(type_)
        if type_ in POINTERS or type_ in FLOATS or not isinstance(value, mir.Const):
            return None
        return self.narrowed(value, self.width(type_))

    def _call_actual(self, value, type_: str):
        """A pointer actual without emitting another address computation."""
        type_ = self.unit.canonical_type(type_)
        if type_ not in POINTERS:
            return None
        match value:
            case (Frame() | Global()) as address:
                return self.placed(address, 1).provenance
            case Near(base, disp):
                return (base, disp)
            case Far(whole=whole, disp=disp) if whole is not None:
                return (whole, disp)
            case Far(named=named, disp=disp) if named:
                return memory.Provenance.one(memory.Object(memory.Kind.NAMED, named), disp, disp + 1)
            case mir.Held(value=pointer):
                return (pointer, 0)
        return memory.Provenance.one(memory.Object(memory.Kind.UNKNOWN))

    @staticmethod
    def _allocation_extent(name: str, arguments: tuple) -> int | None:
        numbers = [value.n for value, _ in arguments if isinstance(value, mir.Const)]
        if name in {"malloc", "_malloc"} and numbers:
            return numbers[0] if numbers[0] > 0 else None
        if name in {"calloc", "_calloc"} and len(numbers) >= 2:
            extent = numbers[0] * numbers[1]
            return extent if extent > 0 else None
        return None

    def inline_code(self, callee: hir.Symbol) -> Returned:
        """Inline assembly, as a call whose callee is laid down at the site.

        It reads and writes what it likes: every register, like a call with
        no contract, and the locals it names by BP offset, which are passed
        as addresses so nothing takes them for the body's alone. Whatever it
        leaves in AX and DX is its result, as a call's is.
        """
        data = bytearray(callee.code.data)
        parts, start, named = [], 0, []
        for fixup in callee.code.fixups:
            target = self.unit.symbols[fixup.symbol]
            key = f"y{fixup.symbol}"
            if fixup.kind == "offset" and key in self.frame:
                data[fixup.at : fixup.at + 2] = ((self.frame[key] + fixup.offset) & 0xFFFF).to_bytes(2, "little")
                named.append(self.frame[key])
            elif fixup.kind in ("offset", "segment") and not target.proc:
                parts.append(bytes(data[start : fixup.at]))
                parts.append((fixup.kind, target.object_name, fixup.offset))
                start = fixup.at + 2
            else:
                raise Unsupported(f"{self.symbol.name}: inline code's {fixup.kind} of {target.name}")
        parts.append(bytes(data[start:]))
        low, high = self.fresh(), self.fresh()
        addresses = tuple(mir.FrameAddress(disp, 2, self.extent(disp)) for disp in dict.fromkeys(named))
        site = self.op(
            K.CALL,
            (mir.Held(low, 2), mir.Held(high, 2)),
            addresses,
            defines=(low, high),
            uses=(),
            loads=CALLEE,
            stores=CALLEE,
        )
        self.inline[site.at] = tuple(parts)
        self.calls[site.at] = callee.object_name
        self.callees[site.at] = callee
        self.contracts[site.at] = runtime.Contract(
            name=callee.object_name,
            cleanup=0,
            control=runtime.Control.RETURNS,
            enters_user_code=False,
            raises_error=False,
            error_handling=False,
            writes=runtime.Memory.ANY,
            reads=runtime.Memory.ANY,
            clobbers=runtime.EVERY,
            established=True,
            evidence="inline assembly: every register assumed clobbered, the result left in AX or DX:AX",
            inputs=frozenset(),
        )
        return Returned(low, high)

    def push(self, value, type_: str) -> int:
        if type_ in FLOATS:
            width = self.width(type_)
            value = self.convert(value, type_, type_)
            if not isinstance(value, (Real, FloatCell)) or value.width != width:
                temporary = Frame(self.slot(width))
                self.put_float(temporary, width, value)
                value = FloatCell(temporary, width)
            # The high doubleword first, so the low one is at the lower address.
            for at in reversed(range(0, width, 4)):
                if isinstance(value, Real):
                    bits = int.from_bytes(_packed(value.value, width)[at : at + 4], "little", signed=True)
                    self.op(K.ARG, (), (mir.Const(bits, 4),))
                else:
                    cell = self.cell(replace(value.address, disp=value.address.disp + at), 4)
                    self.op(K.ARG, (), (self.load(cell, "TY_UINT_4"),))
            return width
        if isinstance(value, Aggregate):
            size = self.size(type_)
            if value.size != size:
                raise Unsupported(f"{self.symbol.name}: {value.size}-byte aggregate used as {size} bytes")
            stacked = _even(size)
            at = stacked
            while at:
                width = 2 if at > size or at < 4 else 4
                at -= width
                loaded = min(width, size - at)
                cell = self.cell(replace(value.address, disp=value.address.disp + at), loaded)
                self.op(K.ARG, (), (self.load(cell, f"TY_UINT_{loaded}"),))
            return stacked
        if isinstance(value, Far):
            if value.whole is not None and value.disp == 0:
                self.op(K.ARG, (), (mir.Held(value.whole, 4),))
                return 4
            self.op(K.ARG, (), (mir.Held(value.segment, 2),))
            self.op(K.ARG, (), (mir.Held(self.near(Near(value.offset, value.disp)), 2),))
            return 4
        width = max(2, self.width(type_))
        operand = self.narrowed(self.operand(value, type_), width)
        self.op(K.ARG, (), (operand,))
        return width

    # ---- values and cells ----

    def operand(self, got, type_: str) -> Operand:
        match got:
            case mir.Held() | mir.Const():
                return got
            case Far(whole=split, disp=0) if split is not None:
                return mir.Held(split, 4)
            case Far():
                whole = self.fresh()
                offset = self.near(Near(got.offset, got.disp))
                self.op(K.CONCAT, (mir.Held(whole, 4),), (mir.Held(got.segment, 2), mir.Held(offset, 2)))
                self.pointer_values.add(whole)
                return mir.Held(whole, 4)
            case Frame() | Global() | Near():
                return mir.Held(self.near(got), 2)
            case Returned():
                return self.operand(self.points(got, type_), type_)
            case Real(value=value, width=4):
                return mir.Const(int.from_bytes(_packed(value, 4), "little", signed=True), 4)
            case FloatCell(address=address, width=4):
                return self.load(self.cell(address, 4), "TY_UINT_4")
        raise Unsupported(f"{self.symbol.name}: {got} used as a value")

    def real(self, value: float, width: int) -> Real:
        """A literal at a float type's precision."""
        return Real(struct.unpack("<f", _packed(value, 4))[0] if width == 4 else value, width)

    def floating(self, got) -> mir.Held:
        """A float on the x87."""
        match got:
            case mir.Held(width=10):
                return got
            case FloatCell(address=address, width=width):
                ref = self.cell(address, width)
                result = self.fresh()
                self.op(
                    K.FLOAD, (mir.Held(result, 10),), (mir.Cell(ref),), loads=(ref,), floating=_loaded(FORMATS[width])
                )
                return mir.Held(result, 10)
            # fldz and fld1 load these with no operand; any other constant costs a
            # slot write before `fild` from the stack, where the pool is one operand.
            case Real(value=value) if value in (0.0, 1.0) and math.copysign(1, value) > 0:
                result = self.fresh()
                self.op(
                    K.FLOAD, (mir.Held(result, 10),), (mir.Const(int(value), 2),), floating=_loaded(INTEGER_FORMATS[2])
                )
                return mir.Held(result, 10)
            case Real(value=value):
                # A constant in memory, at the narrowest width that holds it exactly.
                try:
                    width = 4 if struct.unpack("<f", _packed(value, 4))[0] == value else 8
                except OverflowError:
                    width = 8
                number = self.shared.literals.setdefault(_packed(value, width), len(self.shared.literals))
                return self.floating(FloatCell(Global(Space.SEGMENT, POOL + number), width))
        raise Unsupported(f"{self.symbol.name}: {got} computed as a float")

    def truncated(self, value: mir.Held, type_: str) -> Operand:
        """C's float-to-integer cast rounds toward zero, whatever the environment says."""
        width = max(2, self.width(type_))
        result = self.fresh()
        format_ = floating.Format.UNSIGNED64 if type_ == "TY_UINT_8" else INTEGER_FORMATS[width]
        rule = floating.Semantics(
            (floating.Format.EXTENDED80,),
            format_,
            floating.Precision.DESTINATION,
            floating.Rounding.TOWARD_ZERO,
        )
        self.op(K.FSTORE, (mir.Held(result, width),), (value,), floating=rule)
        if width == 8:
            return mir.Held(result, width)
        return self.convert(mir.Held(result, width), "TY_INT_4" if width == 4 else "TY_INT_2", type_)

    def address(self, got) -> Address:
        match got:
            case Frame() | Global() | Near() | Far():
                return got
            case mir.Held(value=value, width=2):
                return Near(value)
            case mir.Held(width=4):
                return self.split(got)
            case mir.Const(n=n, width=4):
                return Far(self.copy(mir.Const((n >> 16) & 0xFFFF, 2)), self.copy(mir.Const(n & 0xFFFF, 2)))
            case mir.Const(n=n, width=2):
                return Near(self.copy(mir.Const(n & 0xFFFF, 2)))
        raise Unsupported(f"{self.symbol.name}: {got} used as an address")

    def loaded(self, got, node: str):
        type_ = self.type_of(node)
        near = type_ == "TY_NEAR_POINTER" or (type_ == "TY_POINTER" and not self.far_pointer(type_))
        return Near(got.value) if near and isinstance(got, mir.Held) and got.width == 2 else got

    def far_loaded(self, address: Address, type_: str) -> Far:
        """A far pointer in memory, as its offset word, its segment word and the whole.

        Read one after another here, so all three see the same memory; the
        ones nothing uses are dead. Splitting the whole instead cost a shift."""
        whole = self.load(self.cell(address, 4, type_), type_)
        offset = self.load(self.cell(address, 2, type_), "TY_UINT_2")
        segment = self.load(self.cell(replace(address, disp=address.disp + 2), 2, type_), "TY_UINT_2")
        return Far(segment.value, offset.value, whole=whole.value)

    def split(self, pointer: mir.Held) -> Far:
        segment = self.fresh()
        self.op(K.SHR, (mir.Held(segment, 4),), (pointer, mir.Const(16, 1)))
        return Far(segment, pointer.value, whole=pointer.value)

    def near(self, address: Address) -> mir.Value:
        match address:
            case Frame(disp):
                result = self.fresh()
                self.op(K.ADDRESS, (mir.Held(result, 2),), (mir.FrameAddress(disp, 2, self.extent(disp)),))
                self.pointer_values.add(result)
                extent = self.extent(disp)
                if extent is not None:
                    low, high = extent
                    object_ = memory.Object(memory.Kind.FRAME, (self.symbol.id, low, high), extent=high - low)
                    self.pointer_seeds[result] = memory.Provenance.one(object_, disp - low, disp - low + 1)
                return result
            case Global(space, index, disp, None):
                result = self.fresh()
                self.op(K.COPY, (mir.Held(result, 2),), (mir.Symbol(space, index, disp, 2),))
                self.pointer_values.add(result)
                kind = memory.Kind.EXTERNAL if space is Space.EXTERNAL else memory.Kind.GLOBAL
                self.pointer_seeds[result] = memory.Provenance.one(memory.Object(kind, (space, index)), disp, disp + 1)
                return result
            case Global(space, index, disp, base):
                return self.add(mir.Held(self.near(Global(space, index, disp)), 2), mir.Held(base, 2))
            case Near(base, 0):
                return base
            case Near(base, disp):
                return self.add(mir.Held(base, 2), mir.Const(disp, 2))
        raise Unsupported(f"{self.symbol.name}: {address} has no near form")

    def dgroup(self) -> mir.Value:
        """DGROUP's selector, which is SS's in this model: a near address's far form."""
        result = self.fresh()
        self.op(K.COPY, (mir.Held(result, 2),), (mir.Symbol(Space.GROUP, 0, 0, 2),))
        return result

    def cell(self, address: Address, width: int, type_: str | None = None) -> mir.MemRef:
        """The reference, typed by the object it names where declared, else by the lvalue's type."""
        declared = getattr(address, "declared", None)
        access = self.aliasing(type_)
        ref = self.placed(address, width)
        if address.volatile:
            ref = replace(ref, volatile=True)
        if declared is not None:
            return replace(ref, typed=(declared, True))
        return replace(ref, typed=(access, False)) if access is not None else ref

    def placed(self, address: Address, width: int) -> mir.MemRef:
        match address:
            case Frame(disp):
                extent = self.extent(disp)
                provenance = None
                if extent is not None:
                    low, high = extent
                    object_ = memory.Object(memory.Kind.FRAME, (self.symbol.id, low, high), extent=high - low)
                    provenance = memory.Provenance.one(object_, disp - low, disp - low + width)
                return mir.MemRef(Addr(Space.FRAME, disp), width, space=Space.FRAME, provenance=provenance)
            case Global(space, index, disp, None):
                kind = memory.Kind.EXTERNAL if space is Space.EXTERNAL else memory.Kind.GLOBAL
                provenance = memory.Provenance.one(memory.Object(kind, (space, index)), disp, disp + width)
                return mir.MemRef(Addr(space, disp, index), width, space=space, provenance=provenance)
            case Global(space, index, disp, base):
                kind = memory.Kind.EXTERNAL if space is Space.EXTERNAL else memory.Kind.GLOBAL
                provenance = memory.Provenance.one(memory.Object(kind, (space, index)))
                return mir.MemRef(
                    Addr(space, disp, index), width, base=base, space=space, base_width=2, provenance=provenance
                )
            case Near(base, disp):
                return mir.MemRef(
                    Addr(Space.LITERAL, disp), width, base=base, space=address.space, base_width=2
                )
            case Far(segment, offset, disp, _, named):
                provenance = memory.Provenance.one(memory.Object(memory.Kind.NAMED, named)) if named else None
                return mir.MemRef(
                    Addr(Space.FAR, disp, named),
                    width,
                    base=offset,
                    segment=segment,
                    space=Space.FAR,
                    base_width=2,
                    provenance=provenance,
                )

    def load(self, ref: mir.MemRef, type_: str) -> mir.Held:
        result = self.fresh()
        if ref.width == 1:
            kind = K.SIGN_EXTEND if type_ in SIGNED else K.ZERO_EXTEND
            self.op(kind, (mir.Held(result, 2),), (mir.Cell(ref),), loads=(ref,))
            held = mir.Held(result, 2)
            self._pointer_loaded(held, ref, type_)
            return held
        self.op(K.LOAD, (mir.Held(result, ref.width),), (mir.Cell(ref),), loads=(ref,))
        held = mir.Held(result, ref.width)
        self._pointer_loaded(held, ref, type_)
        return held

    def _pointer_loaded(self, held: mir.Held, ref: mir.MemRef, type_: str) -> None:
        type_ = self.unit.canonical_type(type_)
        if type_ not in POINTERS:
            return
        self.pointer_values.add(held.value)
        if ref.addr is not None and ref.addr.space is Space.FRAME and ref.addr.disp in self.parameter_at:
            number = self.parameter_at[ref.addr.disp]
            self.pointer_seeds[held.value] = memory.Provenance.one(memory.Object(memory.Kind.PARAMETER, number), 0, 1)

    def store(self, ref: mir.MemRef, value: Operand | mir.Value) -> None:
        if isinstance(value, mir.Value):
            value = mir.Held(value, 2)
        value = self.narrowed(value, ref.width)
        self.op(K.STORE, (mir.Cell(ref),), (value,), stores=(ref,))

    def copy(self, value: Operand) -> mir.Value:
        result = self.fresh()
        self.op(K.COPY, (mir.Held(result, value.width),), (value,))
        return result

    def narrowed(self, value: Operand, width: int) -> Operand:
        if isinstance(value, mir.Const):
            return mir.Const(value.n & ((1 << (8 * width)) - 1) if value.n >= 0 else value.n, width)
        if value.width > width:
            return mir.Held(value.value, width)
        if value.width < width:
            raise Unsupported(f"{self.symbol.name}: {value} used at width {width}")
        return value

    def extended(self, value: mir.Held, type_: str) -> mir.Held:
        """A one-byte type is carried as a word, extended the way the type says."""
        if self.width(type_) != 1:
            return value
        result = self.fresh()
        kind = K.SIGN_EXTEND if type_ in SIGNED else K.ZERO_EXTEND
        self.op(kind, (mir.Held(result, 2),), (mir.Held(value.value, 1),))
        return mir.Held(result, 2)

    def wrapped(self, n: int, type_: str) -> int:
        bits = 8 * self.width(type_)
        n &= (1 << bits) - 1
        return n - (1 << bits) if type_ in SIGNED and n >> (bits - 1) else n


def _fold(cg_op: str, a: int, b: int, signed: bool) -> int:
    def quotient() -> int:
        # C truncates signed division toward zero.  Going through Python's
        # float did that accidentally for 16/32-bit values, but loses low
        # bits as soon as a long long exceeds binary64's exact range.
        magnitude = abs(a) // abs(b)
        return -magnitude if signed and (a < 0) != (b < 0) else magnitude

    match cg_op:
        case "O_PLUS":
            return a + b
        case "O_MINUS":
            return a - b
        case "O_TIMES":
            return a * b
        case "O_AND":
            return a & b
        case "O_OR":
            return a | b
        case "O_XOR":
            return a ^ b
        case "O_LSHIFT":
            return a << b
        case "O_RSHIFT":
            return a >> b
        case "O_DIV" if b:
            return quotient()
        case "O_MOD" if b:
            return a - quotient() * b
    raise Unsupported(f"constant {cg_op}")
