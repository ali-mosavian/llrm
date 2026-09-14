"""The stream as a unit: its symbols, its data, and each procedure's trees.

The trees are the code generator's own: a node is the call that built it,
and its operands are the handles of the nodes it was built from. Nothing is
lowered here; `raise_hir` does that.
"""

from dataclasses import field
from dataclasses import dataclass

from qbopt.cfront.stream import Record

# fe_attr (bld/cg/h/cg.h)
FE_PROC = 0x1
FE_GLOBAL = 0x4
FE_IMPORT = 0x8
# call_class and call_class_target (cgauxcc.h, x86auxcc.h)
REVERSE_PARMS = 0x1
CALLER_POPS = 0x80
FAR_CALL = 0x4
# cg_target_switches (x86swi.h)
BIG_DATA = 0x2
BIG_CODE = 0x4

STATEMENTS = frozenset(
    {"CGDone", "CGTrash", "CGControl", "CGReturn", "CGSelCase", "CGSelRange", "CGSelOther", "CGSelect", "CGBigLabel"}
)
IGNORED = frozenset(
    {"START", "STOP", "FINI", "ABORT", "BENewLabel", "BEFiniLabel", "CGLastParm", "DBSrcFile", "BEFiniBack"}
)


class Unsupported(Exception):
    """A construct the C path refuses rather than guesses at."""


@dataclass
class Symbol:
    id: int
    name: str
    base: str
    pattern: str
    attr: int
    call_class: int = 0
    call_target: int = 0
    register_parms: bool = False  # any argument passed in a register

    @property
    def proc(self) -> bool:
        return bool(self.attr & FE_PROC)

    @property
    def imported(self) -> bool:
        return bool(self.attr & FE_IMPORT)

    @property
    def exported(self) -> bool:
        return bool(self.attr & FE_GLOBAL) and not self.imported

    @property
    def far(self) -> bool:
        return bool(self.call_target & FAR_CALL)

    @property
    def object_name(self) -> str:
        if self.pattern == "^":
            return self.base.upper()
        return self.pattern.replace("*", self.base) if self.pattern else self.base


@dataclass(frozen=True, slots=True)
class Node:
    call: str
    args: tuple[str, ...]


@dataclass
class Call:
    target: str
    type: str
    symbol: int
    parms: list[tuple[str, str]] = field(default_factory=list)  # (node, type), last argument first


@dataclass(frozen=True, slots=True)
class Statement:
    call: str
    args: tuple[str, ...]
    line: int


@dataclass
class Proc:
    symbol: int
    type: str
    parms: list[tuple[int, str]] = field(default_factory=list)
    autos: list[tuple[str, str]] = field(default_factory=list)  # ("y5" | "t3", type)
    body: list[Statement] = field(default_factory=list)


@dataclass
class Segment:
    id: int
    name: str
    attr: int
    items: list[tuple[str, tuple[str, ...]]] = field(default_factory=list)


@dataclass
class Unit:
    target: int = 0
    symbols: dict[int, Symbol] = field(default_factory=dict)
    backs: dict[int, int] = field(default_factory=dict)  # back handle -> symbol, 0 for a literal
    types: dict[str, int] = field(default_factory=dict)
    segments: dict[int, Segment] = field(default_factory=dict)
    nodes: dict[int, Node] = field(default_factory=dict)
    calls: dict[int, Call] = field(default_factory=dict)
    procs: list[Proc] = field(default_factory=list)


def handle(token: str) -> int:
    return int(token[1:])


def unit(records: list[Record]) -> Unit:
    made = Unit()
    proc: Proc | None = None
    segment: int | None = None
    line = 0
    for one in records:
        args, fields = one.args, one.fields
        match one.call:
            case "UNSUPPORTED":
                raise Unsupported(f"stream line {one.line}: the shim refused {' '.join(args)}")
            case "INIT":
                made.target = int(fields["target"], 16)
            case "SEG":
                made.segments[int(args[0])] = Segment(int(args[0]), fields["name"], int(fields["attr"], 16))
            case "SETSEG":
                segment = int(args[0])
            case "TYPE":
                made.types[args[0]] = int(fields["size"])
            case "SYM":
                made.symbols[handle(args[0])] = Symbol(
                    handle(args[0]), fields["name"], fields["base"], fields["pattern"], int(fields["attr"], 16)
                )
            case "CALLCONV":
                symbol = made.symbols[handle(args[0])]
                symbol.call_class = int(fields["class"], 16)
                symbol.call_target = int(fields["target"], 16)
                symbol.register_parms = fields.get("parms", "[]") != "[]"
            case "BENewBack":
                made.backs[handle(one.result)] = handle(args[0])
            case "CGProcDecl":
                proc = Proc(handle(args[0]), args[1])
                made.procs.append(proc)
            case "CGParmDecl":
                proc.parms.append((handle(args[0]), args[1]))
            case "CGAutoDecl":
                proc.autos.append((args[0], args[1]))
            case "CGTemp":
                proc.autos.append((one.result, args[0]))
            case "CGInitCall":
                made.calls[handle(one.result)] = Call(args[0], args[1], handle(args[2]))
            case "CGAddParm":
                made.calls[handle(args[0])].parms.append((args[1], args[2]))
            case "DBSrcCue":
                line = int(args[1])
            case "CGSelInit":
                proc.body.append(Statement(one.call, (one.result,), line))
            case call if call in STATEMENTS:
                proc.body.append(Statement(call, args, line))
            case call if call.startswith("DG"):
                if segment is None:
                    raise Unsupported(f"stream line {one.line}: data before any segment")
                made.segments[segment].items.append((call, args))
            case call if call in IGNORED:
                pass
            case _ if one.result is not None and one.result.startswith("n"):
                made.nodes[handle(one.result)] = Node(one.call, args)
            case _:
                raise Unsupported(f"stream line {one.line}: {one.call}")
    return made


def text(made: Unit) -> str:
    """The unit, one fact per line, for a stage dump."""
    out = [f"target 0x{made.target:x}"]
    out += [f"segment {one.id} {one.name} attr=0x{one.attr:x} items={one.items}" for one in made.segments.values()]
    out += [f"symbol {one}" for one in made.symbols.values()]
    for proc in made.procs:
        out.append(f"proc {made.symbols[proc.symbol].object_name} {proc.type} parms={proc.parms} autos={proc.autos}")
        out += [f"  {one.line}: {one.call} {' '.join(one.args)}" for one in proc.body]
    return "\n".join(out) + "\n"
