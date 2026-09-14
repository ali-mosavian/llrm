"""One procedure's trees as a MirBody.

Every C variable is a frame cell and every tree node a fresh value, so the
body is in SSA by construction and promotion is left to the passes. What an
idiom is -- a far pointer's two halves, a call's convention, where a result
arrives -- is decided here, which is rule 5's place for it.

The ABI is Borland's medium model, which is what the rest of qcport is
built with: C procedures far and cdecl, results in AX or DX:AX, SI, DI, BP
and DS preserved, SS equal to DGROUP.
"""

import struct
from dataclasses import field
from dataclasses import replace
from dataclasses import dataclass

from qbopt.abi import runtime
from qbopt.model import ir
from qbopt.model import floating
from qbopt.model import mir
from qbopt.cfront import hir
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
}
FLOATS = frozenset({"TY_SINGLE", "TY_DOUBLE", "TY_LONG_DOUBLE"})
FLOAT_ARITHMETIC = {
    "O_PLUS": mir.Kind.FADD,
    "O_MINUS": mir.Kind.FSUB,
    "O_TIMES": mir.Kind.FMUL,
    "O_DIV": mir.Kind.FDIV,
}
EXTENDED = floating.Format.EXTENDED80
INTEGER_FORMATS = {2: floating.Format.SIGNED16, 4: floating.Format.SIGNED32}
# What each float operation computes, which MIR states and lowering spells.
ARITH_RULE = floating.Semantics((EXTENDED, EXTENDED), EXTENDED, floating.Precision.DYNAMIC, floating.Rounding.DYNAMIC)
STORE_SINGLE = floating.Semantics((EXTENDED,), floating.Format.BINARY32, floating.Precision.DESTINATION, floating.Rounding.DYNAMIC)


def _loaded(source: floating.Format) -> floating.Semantics:
    return floating.Semantics((source,), EXTENDED, floating.Precision.EXACT, floating.Rounding.NONE)
SIGNED = frozenset({"TY_INT_1", "TY_INT_2", "TY_INT_4", "TY_INTEGER"})
FAR_POINTERS = frozenset({"TY_LONG_POINTER", "TY_HUGE_POINTER"})

# Literal labels (a back handle with no symbol) share the symbol index space.
LITERAL = 1 << 20

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


@dataclass(frozen=True, slots=True)
class Global:
    space: Space
    index: int
    disp: int = 0


@dataclass(frozen=True, slots=True)
class Near:
    base: mir.Value
    disp: int = 0


@dataclass(frozen=True, slots=True)
class Far:
    segment: mir.Value
    offset: mir.Value
    disp: int = 0


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
    calls: dict[int, str]  # call site -> callee object name
    callees: dict[int, hir.Symbol]
    contracts: dict[int, runtime.Contract]


@dataclass(frozen=True, slots=True)
class Real:
    """A float literal: its bits where it is moved, the x87's where it is computed."""

    value: float


@dataclass(frozen=True, slots=True)
class FloatCell:
    """A float in memory, not yet read: moved as bytes or loaded onto the x87."""

    address: object


def raised(unit: hir.Unit, proc: hir.Proc) -> Raised:
    return _Raise(unit, proc).run()


def names(unit: hir.Unit) -> dict[tuple[Space, int], str]:
    """Every (space, index) a raised operand can name, as its object name."""
    out = {(_space(one), one.id): one.object_name for one in unit.symbols.values()}
    out.update({(Space.SEGMENT, LITERAL + back): f"L_b{back}" for back, symbol in unit.backs.items() if not symbol})
    return out


def _space(symbol: hir.Symbol) -> Space:
    return Space.EXTERNAL if symbol.imported else Space.SEGMENT


def _even(n: int) -> int:
    return n + (n & 1)


class _Raise:
    def __init__(self, unit: hir.Unit, proc: hir.Proc) -> None:
        self.unit = unit
        self.proc = proc
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
        self.frame: dict[str, int] = {}
        at = 6 if self.symbol.far else 4
        for symbol, type_ in proc.parms:
            self.frame[f"y{symbol}"] = at
            at += _even(max(2, self.size(type_)))
        self.down = 0
        for key, type_ in proc.autos:
            self.frame[key] = self.slot(self.size(type_))

    def slot(self, size: int) -> int:
        """A new frame cell below the last."""
        self.down -= _even(size)
        return self.down

    # ---- types ----

    def width(self, type_: str) -> int:
        if type_ == "TY_POINTER":
            return 4 if self.unit.target & hir.BIG_DATA else 2
        if type_ == "TY_CODE_PTR":
            return 4 if self.unit.target & hir.BIG_CODE else 2
        if type_ in WIDTHS:
            return WIDTHS[type_]
        raise Unsupported(f"{self.symbol.name}: no scalar width for {type_}")

    def size(self, type_: str) -> int:
        return self.unit.types[type_] if type_ in self.unit.types else self.width(type_)

    def far_pointer(self, type_: str) -> bool:
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
        made = mir.Op(
            self.at, ir.Operation.NOTHING, "", defines, uses, kind=kind, args=args, results=results, id=next(mir._IDS), **extra
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
                tuple(replace(op, target=at[op.target]) if isinstance(op.target, str) else op for op in block.ops),
                tuple(at[key] for key in block.succ),
            )
            for block in kept
        )
        body = mir.MirBody(blocks[0].at, blocks, origin=dict(self.origin), pins=dict(self.pins), sealed=True)
        problems = mir.verify(body)
        if problems:
            raise Unsupported(f"{self.symbol.name}: raised MIR is not SSA: {problems[:3]}")
        return Raised(self.symbol.object_name, self.symbol, body, self.calls, self.callees, self.contracts)

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
            case _:
                raise Unsupported(f"{self.symbol.name} line {one.line}: {one.call} {' '.join(one.args)}")

    def ret(self, node: str, type_: str) -> None:
        returned: tuple[mir.Value, ...] = ()
        if node != "n0":
            got = self.eval(node)
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
        self.op(K.RETURN, args=tuple(mir.Held(one, 2) for one in returned), uses=returned)
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
            raise Unsupported(f"{self.symbol.name}: float compare")
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
            case "CGFloat", (text, "TY_SINGLE"):
                return Real(float(text))
            case "CGFEName", (symbol, type_):
                return self.name(symbol)
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
            case "CGUnary", ("O_UMINUS" | "O_COMPLEMENT" as cg_op, inner, type_):
                return self.unary(cg_op, self.eval(inner), type_)
            case "CGBinary", (cg_op, left, right, type_):
                return self.binary(cg_op, left, right, type_)
            case "CGAssign", (target, source, type_):
                return self.assign(target, source, type_)
            case "CGLVAssign", (target, source, _):
                return self.aggregate(self.eval(target), self.eval(source))
            case "CGPostGets" | "CGPreGets", (cg_op, target, source, type_):
                address = self.address(self.eval(target))
                width = self.width(type_)
                old = self.load(self.cell(address, width), type_)
                new = self.arithmetic(cg_op, old, self.operand(self.eval(source), type_), type_)
                self.store(self.cell(address, width), new)
                return old if tree.call == "CGPostGets" else new
            case "CGCall", (call,):
                return self.call(self.unit.calls[hir.handle(call)])
            case "CGChoose", (test, yes, no, type_):
                return self.choose(test, yes, no, type_)
            case "CGCompare" | "CGFlow", _:
                return self.truth(node)
            case "CGEval" | "CGVolatile", (inner,):
                return self.eval(inner)
            case "CGAttr", (inner, _):
                return self.eval(inner)
        raise Unsupported(f"{self.symbol.name}: {tree.call} {' '.join(tree.args)}")

    def choose(self, test: str, yes: str, no: str, type_: str):
        return self.joined(
            test, lambda: self.coerced(self.eval(yes), yes, type_), lambda: self.coerced(self.eval(no), no, type_), type_
        )

    def truth(self, test: str):
        """A compare or flow as a value: 1 or 0."""
        return self.joined(test, lambda: mir.Const(1, 2), lambda: mir.Const(0, 2), "TY_INTEGER")

    def joined(self, test: str, yes, no, type_: str):
        """`test ? yes() : no()`: each arm stores into one frame cell, read after
        the join -- a variable like any other, left for promotion to make SSA."""
        width = max(2, self.width(type_))
        joined = Frame(self.slot(width))
        otherwise, join = self.label(), self.label()
        self.branch(test, otherwise, False)
        self.store(self.cell(joined, width), self.narrowed(yes(), width))
        self.op(K.JUMP, target=join)
        self.end(join)
        self.start(otherwise)
        self.store(self.cell(joined, width), self.narrowed(no(), width))
        self.start(join)
        loaded = self.load(self.cell(joined, width), type_)
        return self.split(loaded) if self.far_pointer(type_) else loaded

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

    def name(self, token: str):
        symbol = self.unit.symbols[hir.handle(token)]
        if symbol.proc:
            return Function(symbol)
        if token in self.frame:
            return Frame(self.frame[token])
        return Global(_space(symbol), symbol.id)

    def points(self, got, type_: str):
        if isinstance(got, Returned):
            if self.far_pointer(type_):
                return Far(got.high, got.low)
            if self.width(type_) == 4:
                whole = self.fresh()
                self.op(K.CONCAT, (mir.Held(whole, 4),), (mir.Held(got.high, 2), mir.Held(got.low, 2)))
                return mir.Held(whole, 4)
            return self.extended(mir.Held(got.low, 2), type_)
        address = self.address(got)
        if type_ in self.unit.types:
            return Aggregate(address, self.unit.types[type_])
        if type_ == "TY_SINGLE":
            return FloatCell(address)
        loaded = self.load(self.cell(address, self.width(type_)), type_)
        if self.far_pointer(type_):
            return self.split(loaded)
        return loaded

    def convert(self, got, source: str, type_: str):
        if source in FLOATS or type_ in FLOATS:
            if source == type_:
                return got
            if source == "TY_SINGLE" and type_ not in FLOATS:
                return self.truncated(self.floating(got), type_)
            if type_ == "TY_SINGLE" and source not in FLOATS:
                whole = self.operand(got, source)
                if source not in SIGNED and self.width(source) == 4:
                    raise Unsupported(f"{self.symbol.name}: fild of an unsigned long")
                if self.width(source) == 1 or source not in SIGNED:
                    whole = self.convert(whole, source, "TY_INT_4")
                result = self.fresh()
                self.op(K.FLOAD, (mir.Held(result, 10),), (whole,), floating=_loaded(INTEGER_FORMATS[whole.width]))
                return mir.Held(result, 10)
            raise Unsupported(f"{self.symbol.name}: conversion {source} to {type_}")
        if isinstance(got, (Frame, Global, Near)) and self.far_pointer(type_):
            return Far(self.dgroup(), self.near(got))
        if isinstance(got, (Frame, Global, Near, Far)):
            return got
        if isinstance(got, Returned):
            got = self.points(got, source)
            if not isinstance(got, (mir.Held, mir.Const)):
                return got
        to = self.width(type_)
        if isinstance(got, mir.Held) and got.width == 2 and self.far_pointer(type_):
            return Far(self.dgroup(), got.value)
        if isinstance(got, mir.Const):
            return mir.Const(self.wrapped(got.n, type_), max(2, to))
        if to == 4 and got.width < 4:
            wide = self.fresh()
            kind = K.SIGN_EXTEND if source in SIGNED else K.ZERO_EXTEND
            self.op(kind, (mir.Held(wide, 4),), (mir.Held(got.value, 2),))
            return mir.Held(wide, 4)
        if to == 1:
            return self.extended(mir.Held(got.value, 2), type_)
        if to == 2 and got.width == 4:
            return mir.Held(got.value, 2)
        return got

    def unary(self, cg_op: str, got, type_: str) -> Operand:
        if type_ in FLOATS:
            raise Unsupported(f"{self.symbol.name}: float {cg_op}")
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
        if type_ == "TY_SINGLE":
            if cg_op not in FLOAT_ARITHMETIC:
                raise Unsupported(f"{self.symbol.name}: float {cg_op}")
            kind = FLOAT_ARITHMETIC[cg_op]
            x = self.floating(self.convert(a, self.type_of(left), type_))
            y = self.floating(self.convert(b, self.type_of(right), type_))
            result = self.fresh()
            self.op(kind, (mir.Held(result, 10),), (x, y), floating=ARITH_RULE)
            return mir.Held(result, 10)
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
            return Far(address.segment, moved, address.disp)
        base = address.base if isinstance(address, Near) else self.near(replace(address, disp=0))
        return Near(self.add(mir.Held(base, 2), index), address.disp)

    def add(self, a: mir.Held, b: Operand) -> mir.Value:
        result = self.fresh()
        self.op(K.ADD, (mir.Held(result, a.width),), (a, b))
        return result

    def arithmetic(self, cg_op: str, a: Operand, b: Operand, type_: str) -> Operand:
        if type_ in FLOATS:
            raise Unsupported(f"{self.symbol.name}: float {cg_op}")
        width = max(2, self.width(type_))
        a, b = self.narrowed(a, width), self.narrowed(b, width)
        signed = type_ in SIGNED
        if isinstance(a, mir.Const) and isinstance(b, mir.Const):
            return mir.Const(self.wrapped(_fold(cg_op, a.n, b.n, signed), type_), width)
        result = self.fresh()
        if cg_op in ("O_DIV", "O_MOD"):
            if not signed:
                raise Unsupported(f"{self.symbol.name}: unsigned division")
            if isinstance(a, mir.Const):
                a = mir.Held(self.copy(a), width)
            remainder = self.fresh()
            self.op(K.DIVMOD, (mir.Held(result, width), mir.Held(remainder, width)), (a, b)
            )
            return mir.Held(result if cg_op == "O_DIV" else remainder, width)
        if cg_op in ("O_LSHIFT", "O_RSHIFT"):
            if not isinstance(b, mir.Const):
                raise Unsupported(f"{self.symbol.name}: shift by a variable count")
            if isinstance(a, mir.Const):
                a = mir.Held(self.copy(a), width)
            kind = K.SHL if cg_op == "O_LSHIFT" else (K.SAR if signed else K.SHR)
            self.op(kind, (mir.Held(result, width),), (a, mir.Const(b.n, 1)))
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
            self.store(self.cell(address, 2), self.near(Near(value.offset, value.disp)))
            self.store(self.cell(replace(address, disp=address.disp + 2), 2), mir.Held(value.segment, 2))
            return value
        operand = self.coerced(value, source, type_) if type_ != "TY_SINGLE" else self.convert(value, self.type_of(source), type_)
        if isinstance(operand, mir.Held) and operand.width == 10:
            # fstp pops what it stores, so the assignment's own value is the cell.
            ref = self.cell(address, width)
            self.op(K.FSTORE, (mir.Cell(ref),), (operand,), stores=(ref,), floating=STORE_SINGLE)
            return FloatCell(address)
        if isinstance(operand, (Real, FloatCell)):
            operand = self.operand(operand, type_)
        self.store(self.cell(address, width), operand)
        return operand

    def aggregate(self, target, source) -> None:
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

    def call(self, call: hir.Call) -> Returned:
        target = self.eval(call.target)
        if not isinstance(target, Function):
            raise Unsupported(f"{self.symbol.name}: indirect call")
        callee = target.symbol
        # Stack arguments only: cdecl (caller pops) or pascal (reversed, callee pops).
        stacked = callee.call_class & (hir.CALLER_POPS | hir.REVERSE_PARMS) in (hir.CALLER_POPS, hir.REVERSE_PARMS)
        if callee.register_parms or not stacked:
            raise Unsupported(f"{self.symbol.name}: {callee.object_name} has a register calling convention")
        arguments = [(self.eval(node), type_) for node, type_ in call.parms]
        if callee.call_class & hir.REVERSE_PARMS:
            arguments.reverse()
        pushed = sum(self.push(value, type_) for value, type_ in arguments)
        low, high = self.fresh(), self.fresh()
        site = self.op(K.CALL, (mir.Held(low, 2), mir.Held(high, 2)), defines=(low, high), uses=())
        caller_pops = bool(callee.call_class & hir.CALLER_POPS)
        self.calls[site.at] = callee.object_name
        self.callees[site.at] = callee
        self.contracts[site.at] = runtime.Contract(
            name=callee.object_name,
            cleanup=0 if caller_pops else pushed,
            control=runtime.Control.RETURNS,
            enters_user_code=False,
            raises_error=False,
            error_handling=False,
            writes=runtime.Memory.ANY,
            reads=runtime.Memory.ANY,
            clobbers=runtime.EVERY,
            established=True,
            evidence="Borland medium model: stack arguments, result in AX or DX:AX; every register assumed clobbered",
            inputs=frozenset(),
            caller_cleanup=pushed if caller_pops else 0,
        )
        return Returned(low, high)

    def push(self, value, type_: str) -> int:
        if isinstance(value, Far):
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
            case Far():
                whole = self.fresh()
                offset = self.near(Near(got.offset, got.disp))
                self.op(K.CONCAT, (mir.Held(whole, 4),), (mir.Held(got.segment, 2), mir.Held(offset, 2)))
                return mir.Held(whole, 4)
            case Frame() | Global() | Near():
                return mir.Held(self.near(got), 2)
            case Returned():
                return self.operand(self.points(got, type_), type_)
            case Real(value):
                return mir.Const(int.from_bytes(struct.pack("<f", value), "little", signed=True), 4)
            case FloatCell(address):
                return self.load(self.cell(address, 4), "TY_UINT_4")
        raise Unsupported(f"{self.symbol.name}: {got} used as a value")

    def floating(self, got) -> mir.Held:
        """A float on the x87."""
        match got:
            case mir.Held(width=10):
                return got
            case FloatCell(address):
                ref = self.cell(address, 4)
                result = self.fresh()
                self.op(
                    K.FLOAD, (mir.Held(result, 10),), (mir.Cell(ref),), loads=(ref,), floating=_loaded(floating.Format.BINARY32)
                )
                return mir.Held(result, 10)
            case Real(value) if value.is_integer() and -0x8000 <= value < 0x8000:
                result = self.fresh()
                self.op(K.FLOAD, (mir.Held(result, 10),), (mir.Const(int(value), 2),), floating=_loaded(INTEGER_FORMATS[2]))
                return mir.Held(result, 10)
        raise Unsupported(f"{self.symbol.name}: {got} computed as a float")

    def truncated(self, value: mir.Held, type_: str) -> Operand:
        """C's float-to-integer cast rounds toward zero, whatever the environment says."""
        width = max(2, self.width(type_))
        result = self.fresh()
        rule = floating.Semantics(
            (floating.Format.EXTENDED80,),
            floating.Format.SIGNED32 if width == 4 else floating.Format.SIGNED16,
            floating.Precision.DESTINATION,
            floating.Rounding.TOWARD_ZERO,
        )
        self.op(K.FSTORE, (mir.Held(result, width),), (value,), floating=rule)
        return self.convert(mir.Held(result, width), "TY_INT_4" if width == 4 else "TY_INT_2", type_)

    def address(self, got) -> Address:
        match got:
            case Frame() | Global() | Near() | Far():
                return got
            case mir.Held(value=value, width=2):
                return Near(value)
            case mir.Held(width=4):
                return self.split(got)
        raise Unsupported(f"{self.symbol.name}: {got} used as an address")

    def split(self, pointer: mir.Held) -> Far:
        segment = self.fresh()
        self.op(K.SHR, (mir.Held(segment, 4),), (pointer, mir.Const(16, 1)))
        return Far(segment, pointer.value)

    def near(self, address: Address) -> mir.Value:
        match address:
            case Frame(disp):
                result = self.fresh()
                self.op(K.ADDRESS, (mir.Held(result, 2),), (mir.FrameAddress(disp, 2),))
                return result
            case Global(space, index, disp):
                result = self.fresh()
                self.op(K.COPY, (mir.Held(result, 2),), (mir.Symbol(space, index, disp, 2),))
                return result
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

    def cell(self, address: Address, width: int) -> mir.MemRef:
        match address:
            case Frame(disp):
                return mir.MemRef(Addr(Space.FRAME, disp), width, space=Space.FRAME)
            case Global(space, index, disp):
                return mir.MemRef(Addr(space, disp, index), width, space=space)
            case Near(base, disp):
                return mir.MemRef(Addr(Space.LITERAL, disp), width, base=base, space=Space.LITERAL, base_width=2)
            case Far(segment, offset, disp):
                return mir.MemRef(
                    Addr(Space.FAR, disp),
                    width,
                    base=offset,
                    segment=segment,
                    space=Space.FAR,
                    base_width=2,
                )

    def load(self, ref: mir.MemRef, type_: str) -> mir.Held:
        result = self.fresh()
        if ref.width == 1:
            kind = K.SIGN_EXTEND if type_ in SIGNED else K.ZERO_EXTEND
            self.op(kind, (mir.Held(result, 2),), (mir.Cell(ref),), loads=(ref,))
            return mir.Held(result, 2)
        self.op(K.LOAD, (mir.Held(result, ref.width),), (mir.Cell(ref),), loads=(ref,))
        return mir.Held(result, ref.width)

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
            return int(a / b)
        case "O_MOD" if b:
            return a - int(a / b) * b
    raise Unsupported(f"constant {cg_op}")
