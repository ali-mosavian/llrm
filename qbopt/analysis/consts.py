"""
Which values are known, and what they are.

Ordinary sparse constant propagation over the SSA graph, with one thing
about this machine that is not ordinary and cannot be skipped: a value here
is rooted to its 32-bit parent, and almost every instruction BC emits writes
sixteen bits of it. `mov ax,5` does not make eax five. It makes the low half
five and leaves the high half whatever the pass that widened it put there.

So a fact is a width as well as a number -- "the low `width` bytes of this
value are `n`" -- and an operation only folds where the widths agree. Taking
the number alone would be wrong in the direction that matters: it would fold
a 32-bit use of a value only half of which is known, and produce a plausible
answer that is not the program's.

Memory facts come from explicit stores and loader-established entry facts
supplied by the raise, not assumed zero-filled or immutable static contents.
Adjacent known fragments can supply a wider read only when every requested byte
is covered. Unknown and overlapping writes still invalidate the cell facts.
Phi inputs and memory facts meet on agreement across incoming paths.
"""

from dataclasses import replace
from dataclasses import dataclass
from dataclasses import field
from contextvars import ContextVar
from collections.abc import Iterator
from contextlib import contextmanager

from qbopt.model import ir
from qbopt.model import mir
from qbopt.model import memory
from qbopt.abi import runtime
from qbopt.analysis import cellmap
from qbopt.objectfile.module import Space

# What each operation does to two known numbers, within one width. Division
# has two results and is handled separately, with the faulting cases excluded.
ARITH = {
    mir.Kind.ADD: lambda a, b: a + b,
    mir.Kind.SUB: lambda a, b: a - b,
    mir.Kind.AND: lambda a, b: a & b,
    mir.Kind.OR: lambda a, b: a | b,
    mir.Kind.XOR: lambda a, b: a ^ b,
    mir.Kind.SHL: lambda a, b: a << (b & 31),
    mir.Kind.SHR: lambda a, b: (a & 0xFFFFFFFF) >> (b & 31),
    mir.Kind.MUL: lambda a, b: a * b,
}

UNARY = {
    mir.Kind.NEG: lambda a: -a,
    mir.Kind.NOT: lambda a: ~a,
}


# Memory facts are stored as bytes so partial writes and control-flow
# joins do not discard an untouched neighbor. Reads assemble their width.
Cells = dict


@dataclass(slots=True)
class _MemoryQueries:
    """Alias questions for one immutable known-value epoch.

    ``cells`` walks a body to a fixed point, but its supplied ``known`` map
    cannot change during that invocation.  Address resolution, interval
    construction and overlap answers are therefore facts of the invocation,
    not work to repeat for every operation on every walk.

    Reference identity is deliberate.  ``MemRef`` excludes some semantic
    alias fields from dataclass equality, while this cache must never merge
    references merely because those fields compare equal.
    """

    known: dict[mir.Value, "Known"]
    dgroup: frozenset[int]
    # Which object each directly addressed byte is; see alias.named_bytes.
    named: dict = field(default_factory=dict)
    facts: dict = field(init=False, default_factory=dict)
    addressed: dict[int, mir.MemRef] = field(init=False, default_factory=dict)
    overlaps: dict[tuple[tuple, int], bool] = field(init=False, default_factory=dict)
    # `named`'s per-byte entries never change; `learn` adds whole symbols only.
    exact: dict[tuple, "tuple[memory.Object, int] | None"] = field(init=False, default_factory=dict)
    spans: dict[tuple, "tuple[int, int] | None"] = field(init=False, default_factory=dict)

    def __post_init__(self) -> None:
        self.facts = _intervals(self.known)
        self.addressed = {}
        self.overlaps = {}
        self.exact = {}
        self.spans = {}

    def resolve(self, ref: mir.MemRef) -> mir.MemRef:
        key = id(ref)
        if key not in self.addressed:
            self.addressed[key] = _addressed(ref, self.known)
        return self.addressed[key]

    def learn(self, ref: mir.MemRef) -> None:
        """The space a resolved store lands in is its object, where its displacement is its offset there."""
        if ref.provenance is None or len(ref.provenance.slices) != 1 or ref.addr is None:
            return
        one = next(iter(ref.provenance.slices))
        if one.stride == 1 and one.low <= ref.addr.disp and ref.addr.disp + ref.width <= one.high + one.width - 1:
            self.named.setdefault((ref.addr.space, ref.addr.index), one.object)

    def _named(self, where: tuple) -> "tuple[memory.Object, int] | None":
        """The object and offset every byte of cell `where` names, where they agree."""
        if where not in self.exact:
            named = [self.named.get(where[0].plus(byte)) for byte in range(where[1])]
            agree = named and named[0] is not None and all(one == (named[0][0], named[0][1] + i) for i, one in enumerate(named))
            self.exact[where] = named[0] if agree else None
        return self.exact[where]

    def bucket(self, where: tuple) -> tuple:
        """Cell `where`'s ``mir.overlap_bucket``, from the object its bytes name, if any."""
        named = self._named(where)
        return mir.object_bucket(None if named is None else named[0], (None, None, where[0].space, where[0].index))

    def span(self, where: tuple) -> tuple[int, int] | None:
        """Cell `where`'s ``mir.overlap_span``."""
        if where not in self.spans:
            self.spans[where] = mir.overlap_span(mir.MemRef(where[0], where[1], None, None))
        return self.spans[where]

    def owned(self, here: Cells) -> cellmap.CellMap:
        """A copy of `here` indexed by this epoch's buckets, for one operation to change."""
        if isinstance(here, cellmap.CellMap) and here.bucket_of == self.bucket:
            return here.copy()
        return cellmap.CellMap(self.bucket, here, self.span)

    def may_overlap(self, where: tuple, ref: mir.MemRef) -> bool:
        key = (where, id(ref))
        if key not in self.overlaps:
            cell = mir.MemRef(where[0], where[1], None, None)
            named = [self.named.get(where[0].plus(byte)) for byte in range(where[1])]
            whole = self.named.get((where[0].space, where[0].index))
            if (exact := self._named(where)) is not None:
                cell = replace(cell, provenance=memory.Provenance.one(exact[0], exact[1], exact[1] + where[1]))
            elif whole is not None and all(
                one is None or one == (whole, where[0].disp + i) for i, one in enumerate(named)
            ):
                cell = replace(cell, provenance=memory.Provenance.one(whole, where[0].disp, where[0].disp + where[1]))
            self.overlaps[key] = mir.overlapping(
                cell,
                ref,
                self.dgroup,
                known=self.facts,
                other_known=self.facts,
            )
        return self.overlaps[key]


# One MIR fixed-point transaction repeatedly asks several analyses for the
# same immutable body.  Keeping this cache dynamically scoped makes that
# sharing explicit and bounded: it cannot survive into another compilation or
# confuse a recycled object id with a new body.  Requests with edge/entry
# facts stay uncached because those fact maps are deliberately mutable proof
# inputs.
_reuse: ContextVar[dict | None] = ContextVar("qbopt_constant_analysis_reuse", default=None)


@contextmanager
def reusing() -> Iterator[None]:
    """Reuse ordinary constant facts for identical bodies in one transaction."""
    token = _reuse.set({})
    try:
        yield
    finally:
        _reuse.reset(token)


def _reuse_key(
    body: mir.MirBody,
    dgroup: frozenset[int] | None,
    calls: dict[int, str] | None,
    edges: dict[tuple[int, int], Cells] | None,
    initial: Cells | None,
) -> tuple | None:
    if edges is not None or initial is not None:
        return None
    named = None if calls is None else tuple(sorted(calls.items()))
    return id(body), dgroup, named


@dataclass(frozen=True, slots=True)
class Known:
    """The low `width` bytes of a value are `n`. Nothing is said above them."""

    n: int
    width: int

    def __repr__(self) -> str:
        return f"{self.n:#x}:{self.width}"


def masked(n: int, width: int) -> int:
    return n & ((1 << (width * 8)) - 1)


def division(op: mir.Op, known: dict, here: Cells) -> tuple[int, int] | None:
    """Integer quotient and remainder, excluding the faulting cases."""
    if op.kind not in (mir.Kind.DIVMOD, mir.Kind.UDIVMOD) or len(op.args) != 2 or len(op.results) != 2 or op.stores:
        return None
    if any(not isinstance(result, mir.Held) for result in op.results):
        return None
    widths = {result.width for result in op.results}
    if len(widths) != 1 or not widths <= {2, 4, 8}:
        return None
    width = widths.pop()
    operands = [_operand(op, arg, known, here) for arg in op.args]
    if any(fact is None or fact.width < width for fact in operands):
        return None
    if op.kind is mir.Kind.DIVMOD:
        sign = 1 << (width * 8 - 1)
        dividend, divisor = [((masked(fact.n, width) ^ sign) - sign) for fact in operands]
    else:
        dividend, divisor = [masked(fact.n, width) for fact in operands]
    if divisor == 0 or (op.kind is mir.Kind.DIVMOD and dividend == -(1 << (width * 8 - 1)) and divisor == -1):
        return None
    quotient = abs(dividend) // abs(divisor)
    if op.kind is mir.Kind.DIVMOD and (dividend < 0) != (divisor < 0):
        quotient = -quotient
    return masked(quotient, width), masked(dividend - quotient * divisor, width)


def _put(op: mir.Op, known: dict[mir.Value, Known]) -> Known | None:
    """What this store puts in the cell, where that is a number.

    Two shapes and both are common: BC writes an initialiser as a store of
    a constant, so the number is in the operation itself and is no value at
    all, and it writes an assignment as a store of a value, where the
    number is whatever that value was known to hold.
    """
    if op.kind is not mir.Kind.STORE or len(op.args) != 1:
        return None
    source = op.args[0]
    if isinstance(source, mir.Const):
        return Known(masked(source.n, source.width), source.width)
    if isinstance(source, mir.Held):
        return known.get(source.value)
    return None


def initialized(op: mir.Op, ref: mir.MemRef) -> Known | None:
    """The complete value a direct constant store writes to a contained cell."""
    if op.kind is not mir.Kind.STORE or op.loads or op.barrier or len(op.stores) != 1:
        return None
    written = mir._symbolic_ref(op.stores[0])
    if written.addr is None or written.base is not None or written.segment is not None:
        return None
    fact = _put(op, {})
    return _cell({(written.addr, written.width): fact}, ref) if fact is not None else None


def updated(op: mir.Op, known: dict, here: Cells) -> Known | None:
    """The value of an exact scalar read-modify-write, before its store kills the facts."""
    if (
        op.barrier
        or op.floating
        or op.merges
        or len(op.stores) != 1
        or op.loads != op.stores
        or op.results != (mir.Cell(op.stores[0]),)
        or any(not value.flags for value in op.defines)
    ):
        return None
    width = op.stores[0].width
    if width not in (2, 4):
        return None
    parts = [_operand(op, arg, known, here) for arg in op.args]
    if not parts or any(fact is None or fact.width < width for fact in parts):
        return None
    if op.kind in ARITH and len(parts) == 2:
        result = ARITH[op.kind](parts[0].n, parts[1].n)
    elif op.kind in UNARY and len(parts) == 1:
        result = UNARY[op.kind](parts[0].n)
    elif len(parts) == 1 and (step := mir.stepping(replace(op, loads=(), stores=()))) is not None:
        if not isinstance(step[1], mir.Const):
            return None
        result = parts[0].n + step[1].n
    else:
        return None
    return Known(masked(result, width), width)


def memory_queries(body: mir.MirBody, known: dict[mir.Value, Known], dgroup: frozenset[int]) -> _MemoryQueries:
    """Alias questions about `body`'s cells, each cell carrying the object its references name."""
    from qbopt.analysis import alias

    return _MemoryQueries(known, dgroup, alias.named_bytes(body))


def _fragments(ref: mir.MemRef, fact: Known) -> Cells:
    return {
        (ref.addr.plus(offset), 1): Known((fact.n >> (offset * 8)) & 255, 1)
        for offset in range(min(ref.width, fact.width))
    }


def _selector(
    ref: mir.MemRef,
    known: dict[mir.Value, Known],
    allowed: "frozenset[mir.Value] | None" = None,
) -> "mir.Value | None":
    """A far store's selector, where nothing yet says which segment it is
    and it is still one this run may take on faith."""
    if ref.addr is None or ref.addr.space is not Space.FAR or ref.segment is None:
        return None
    if ref.segment in known or (allowed is not None and ref.segment not in allowed):
        return None
    return ref.segment


def _intervals(known: dict[mir.Value, Known]) -> dict:
    """What each value is, as the alias lattice asks for it.

    Axiom 3 wants the selector's number; this map is the same fact in the
    shape `regions` reads, so a far store through a resolved selector stops
    killing the statics it cannot reach.
    """
    from qbopt.analysis.ranges import Interval

    return {value: Interval(fact.n, fact.n, fact.width) for value, fact in known.items()}


def _kills(
    here: Cells,
    op: mir.Op,
    known: dict[mir.Value, Known],
    dgroup: frozenset[int],
    calls: dict[int, str],
    assume: "set[mir.Value] | None" = None,
    allowed: "frozenset[mir.Value] | None" = None,
    edge_facts: bool = False,
    queries: "_MemoryQueries | None" = None,
) -> Cells:
    """The cell facts still standing after this operation.

    `assume` collects the far selectors this took on faith. Nothing here can
    learn `b$seg` is 0xa000 while the POKE that reads it is taken to write
    every static, and the POKE cannot be placed until `b$seg` is known: the
    two wait on each other for ever and the pessimistic answer is stable. So
    an unresolved selector is assumed to be some absolute segment -- axiom 3,
    applied before it is proven -- and the caller checks afterwards that
    every one of them did resolve. `allowed` is which selectors a run may
    still assume; see `known`.
    """
    from qbopt.analysis import effects

    # A fact supplied for one CFG edge is a proof about reaching that edge,
    # not a durable summary of a callee.  In particular, a numeric loop exit
    # may describe a static cell exactly at its successor but cannot be
    # forwarded through a call merely because today's runtime contract does
    # not name that cell.  The caller may observe, replace, or resume through
    # memory the local contract cannot model.  Ordinary propagation keeps its
    # existing precise call handling; this conservative rule applies only
    # while explicitly supplied edge facts participate in the analysis.
    if edge_facts and op.kind is mir.Kind.CALL:
        here = {}
    if effects.unmodeled_write(op) and (op.barrier or op.at not in calls):
        here = {}
    if op.kind is mir.Kind.CALL and op.at in calls and not op.stores:
        # A raised call names what its callee writes as one of its stores,
        # and the loop below reads it like any other. Only a call with none
        # has to be taken at its word.
        contract = runtime.contract(calls[op.at])
        if runtime.barrier(contract) or runtime.writes_caller_memory(contract):
            here = {}
    put = _put(op, known) if op.kind is mir.Kind.STORE else updated(op, known, here)
    queries = queries if queries is not None else _MemoryQueries(known, dgroup)
    owned = False
    for ref in op.stores:
        ref = queries.resolve(ref)
        if assume is not None and (selector := _selector(ref, known, allowed)) is not None:
            # Taken on faith, and recorded so the caller can check it. A cell
            # in `here` is always a static -- `_fragments` adds no far ref --
            # so an absolute segment reaches none of them.
            assume.add(selector)
            continue
        if not owned:
            here, owned = queries.owned(here), True
        reached, displaced = mir.overlap_buckets(ref, here), mir.displaced_buckets(ref, here)
        here.kill(reached, lambda where: queries.may_overlap(where, ref), displaced)
        if put is not None and ref.addr is not None and ref.base is None and ref.segment is None:
            queries.learn(ref)
            here.update(_fragments(ref, put))
    if op.kind is mir.Kind.CALL and op.memory_values:
        here = here if owned else queries.owned(here)
        for ref, value in op.memory_values:
            if ref.addr is not None and ref.base is None and ref.segment is None:
                queries.learn(ref)
                here.update(_fragments(ref, Known(masked(value.n, value.width), value.width)))
    return here


def cells(
    body: mir.MirBody,
    dgroup: frozenset[int],
    calls: dict[int, str],
    known: dict[mir.Value, Known] | None = None,
    *,
    initial: Cells | None = None,
    edges: dict[tuple[int, int], Cells] | None = None,
    assume: "set[mir.Value] | None" = None,
    allowed: "frozenset[mir.Value] | None" = None,
) -> dict[tuple[int, int], Cells]:
    """What each memory cell holds before each operation, where it is a number.

    Forward to a fixed point, meeting at a join on agreement, which is the
    same shape as known() and for the same reason. Keyed on the block and
    the operation's index within it rather than its address, because
    absorption puts several operations on one address.

    Optional edge facts are independently proved byte fragments. Apply them
    before the predecessor meet, so a loop's final value is not its invariant
    value and a bypass path must agree before a following read can fold.

    A block none of whose predecessors have been visited yet is *deferred*,
    not treated as knowing nothing. Saying "nothing is known here" poisons
    the meet for good -- a loop body's only predecessor is its header, which
    is unvisited on the first round, so hotlop could never learn that `n` is
    7 even though the entry block says so three instructions earlier.
    """
    known = known if known is not None else {}
    queries = memory_queries(body, known, dgroup)
    if initial is None:
        initial = {}
        for ref, value in body.initial:
            initial.update(_fragments(ref, Known(value.n, value.width)))
    outof: dict[int, Cells | None] = {block.at: None for block in body.blocks}
    preds = {block.at: [one.at for one in body.blocks if block.at in one.succ] for block in body.blocks}

    def entering(at: int) -> Cells | None:
        if not preds[at]:
            return dict(initial or {}) if at == body.entry else {}
        seen = []
        for one in preds[at]:
            if outof[one] is None:
                continue
            extra = (edges or {}).get((one, at), {})
            here = outof[one]
            if extra:
                here = {
                    where: fact
                    for where, fact in here.items()
                    if not any((where[0].plus(offset), 1) in extra for offset in range(where[1]))
                }
                here = {**here, **extra}
            seen.append(here)
        if at == body.entry:
            seen.append(initial or {})
        if not seen:
            return None
        return {where: fact for where, fact in seen[0].items() if all(one.get(where) == fact for one in seen[1:])}

    changing = True
    while changing:
        changing = False
        for block in body.blocks:
            here = entering(block.at)
            if here is None:
                continue
            for op in block.ops:
                here = _kills(here, op, known, dgroup, calls, assume, allowed, bool(edges), queries)
            if outof[block.at] != here:
                outof[block.at] = here
                changing = True

    found: dict[tuple[int, int], Cells] = {}
    for block in body.blocks:
        here = entering(block.at) or {}
        for index, op in enumerate(block.ops):
            found[(block.at, index)] = here
            here = _kills(here, op, known, dgroup, calls, assume, allowed, bool(edges), queries)
    return found


def _read(fact: Known | None, width: int) -> Known | None:
    if fact is None or fact.width < width:
        return None
    return Known(masked(fact.n, width), width)


def _cell(here: Cells, ref: mir.MemRef) -> Known | None:
    ref = mir._symbolic_ref(ref)
    if ref.addr is None or ref.base is not None or ref.segment is not None:
        return None
    if exact := _read(here.get((ref.addr, ref.width)), ref.width):
        return exact
    number = 0
    for offset in range(ref.width):
        wanted = ref.addr.plus(offset)
        fragments = {
            (fact.n >> (8 * byte)) & 255
            for (address, width), fact in here.items()
            for byte in range(min(width, fact.width))
            if address.plus(byte) == wanted
        }
        if len(fragments) != 1:
            return None
        number |= fragments.pop() << (8 * offset)
    return Known(number, ref.width)


def _addressed(ref: mir.MemRef, known: dict) -> mir.MemRef:
    """Resolve one proven constant offset using the existing no-wrap address proof."""
    from qbopt.analysis import ranges

    ref = mir._symbolic_ref(ref)
    if ref.base is None:
        return ref
    interval = ranges._operand(mir.Held(ref.base, ref.base_width), {}, known)
    if interval is None:
        return ref
    return ranges.covering(ref, {ref.base: interval})


def _operand(op: mir.Op, one: mir.Arg, known: dict, here: Cells | None = None) -> Known | None:
    """One operand as a number, if it is one.

    MIR's own operands: a constant is one, a value is one where something
    has said so, and a cell is one where the memory walk has. This matched
    ir.Reg and resolved it back to a value through `origin` -- a pass
    asking which register an operand named.
    """
    if isinstance(one, mir.Const):
        return Known(masked(one.n, one.width), one.width)
    if isinstance(one, mir.Held):
        return _read(known.get(one.value), one.width)
    if isinstance(one, mir.Cell) and here is not None:
        # A cell whose content is known is as good as a constant. Without
        # this the propagation stops at BC's first store: it keeps every
        # variable in memory, so `n * k` reads two cells and neither is a
        # value this could ask about.
        return _cell(here, _addressed(one.ref, known))
    return None


def _defined(op: mir.Op, semantics: ir.Semantics | None = None, origin: dict | None = None) -> mir.Value | None:
    """The value this operation's first result gets, flags aside.

    Nearly every arithmetic operation defines its result and the flags
    together, so asking for a single definition rejects all of them --
    which it did, and the propagation found nothing but its own seeds until
    the flags were excluded here.

    Two results are the other case, and refusing them cost more: a widening
    multiply defines a pair, and hotlop's `n * k` is exactly that. Both
    halves are constant and the high one is dead, and nothing here could
    say so, so the product was recomputed on all twenty passes of the loop.
    The result the fold is about is the one the first result names -- which
    the operation says itself now, where it used to be looked up by which
    register the destination was.
    """
    real = [one for one in op.defines if not one.flags]
    if len(real) == 1:
        return real[0]
    first = next((one for one in op.results if isinstance(one, mir.Held)), None)
    if first is None:
        return None
    return first.value if first.value in real else None


def _result(
    op: mir.Op,
    known: dict[mir.Value, Known],
    origin: dict | None = None,
    here: Cells | None = None,
    carries: dict[mir.Value, int] | None = None,
) -> Known | None:
    """What this operation computes, where every input is known."""
    if _defined(op) is None:
        return None
    if op.kind is mir.Kind.EXTRACT and len(op.args) == 2 and len(op.results) == 1:
        source, offset = op.args
        fact = _operand(op, source, known, here)
        width = op.results[0].width
        if (
            isinstance(offset, mir.Const)
            and offset.n >= 0
            and fact is not None
            and fact.width * 8 >= offset.n + width * 8
        ):
            return Known(masked(fact.n >> offset.n, width), width)
        return None
    if (
        op.kind in (mir.Kind.XOR, mir.Kind.SUB)
        and len(op.args) == 2
        and isinstance(op.args[0], mir.Held)
        and op.args[0] == op.args[1]
    ):
        return Known(0, op.args[0].width)
    parts: list[Known] = []
    for one in op.args:
        got = _operand(op, one, known, here)
        if got is None:
            return None
        parts.append(got)
    if not parts:
        return None
    if op.kind in (mir.Kind.SIGN_EXTEND, mir.Kind.ZERO_EXTEND) and len(parts) == len(op.results) == 1:
        source, result = op.args[0], op.results[0]
        source_width = source.ref.width if isinstance(source, mir.Cell) else getattr(source, "width", 0)
        if (
            not isinstance(source, (mir.Held, mir.Const, mir.Cell))
            or not isinstance(result, mir.Held)
            or not 0 < source_width < result.width <= 8
            or parts[0].width < source_width
        ):
            return None
        number = masked(parts[0].n, source_width)
        if op.kind is mir.Kind.SIGN_EXTEND:
            sign = 1 << (source_width * 8 - 1)
            number = (number ^ sign) - sign
        return Known(masked(number, result.width), result.width)
    if op.kind is mir.Kind.CONCAT and len(parts) == 2 and len(op.results) == 1:
        high, low = op.args
        width = high.width + low.width
        if op.results[0].width != width or any(fact.width < arg.width for fact, arg in zip(parts, op.args)):
            return None
        return Known((masked(parts[0].n, high.width) << (low.width * 8)) | masked(parts[1].n, low.width), width)
    if op.kind in (mir.Kind.SHL, mir.Kind.SHR) and len(parts) == 2 and len(op.results) == 1:
        source, result = op.args[0], op.results[0]
        if (
            not isinstance(source, (mir.Held, mir.Const))
            or not isinstance(result, mir.Held)
            or source.width != result.width
            or parts[0].width < source.width
        ):
            return None
        count = parts[1].n & (result.width * 8 - 1)
        number = masked(parts[0].n, result.width)
        shifted = number << count if op.kind is mir.Kind.SHL else number >> count
        return Known(masked(shifted, result.width), result.width)
    width = min(one.width for one in parts)
    if op.kind is mir.Kind.SMULHI and len(parts) == 2 and len(op.results) == 1:
        result = op.results[0]
        if (
            not isinstance(result, mir.Held)
            or result.width not in (2, 4)
            or any(not isinstance(arg, (mir.Held, mir.Const)) or arg.width != result.width for arg in op.args)
            or width < result.width
        ):
            return None
        width = result.width
        sign = 1 << (width * 8 - 1)
        first, second = ((masked(part.n, width) ^ sign) - sign for part in parts)
        return Known(masked((first * second) >> (width * 8), width), width)
    if op.kind is mir.Kind.ADD_CARRY and len(parts) == 2:
        flags = [value for value in op.uses if value.flags]
        if len(flags) == 1 and flags[0] in (carries or {}):
            return Known(masked(parts[0].n + parts[1].n + carries[flags[0]], width), width)
        return None

    if len(parts) == 1 and (step := mir.stepping(op)) is not None and isinstance(step[1], mir.Const):
        return Known(masked(parts[0].n + step[1].n, width), width)

    if op.kind in (mir.Kind.COPY, mir.Kind.LOAD) and len(parts) == 1:
        return Known(masked(parts[0].n, width), width)
    if op.kind in ARITH and len(parts) == 2:
        a, b = parts
        return Known(masked(ARITH[op.kind](a.n, b.n), width), width)
    if op.kind in UNARY and len(parts) == 1:
        return Known(masked(UNARY[op.kind](parts[0].n), width), width)
    return None


def _carry(op: mir.Op, facts: dict, here: Cells) -> int | None:
    if op.kind is not mir.Kind.ADD or len(op.args) != 2 or len(op.results) != 1:
        return None
    if not isinstance(op.results[0], mir.Held):
        return None
    width = op.results[0].width
    operands = [_operand(op, arg, facts, here) for arg in op.args]
    if any(fact is None or fact.width < width for fact in operands):
        return None
    return int(sum(masked(fact.n, width) for fact in operands) >= 1 << (width * 8))


def _pointer_stores(body: mir.MirBody, dgroup: frozenset[int]) -> dict[mir.Value, mir.Arg]:
    """Dominating, exact stores supplying whole-pointer loads outside the static-cell lattice."""
    from qbopt.analysis import loops
    from qbopt.analysis import memoryssa

    candidates = [
        (memoryssa.Site(block.at, index), op)
        for block in body.blocks
        for index, op in enumerate(block.ops)
        if op.kind is mir.Kind.LOAD
        and not op.barrier
        and not op.floating
        and not op.stores
        and len(op.loads) == len(op.args) == len(op.results) == 1
        and op.loads[0].pointer
        and op.args == (mir.Cell(op.loads[0]),)
        and isinstance(op.results[0], mir.Held)
        and op.results[0].width == op.loads[0].width
    ]
    if not candidates:
        return {}
    graph = memoryssa.built(body)
    accesses = {access.id: access for access in graph.accesses}
    dominators = loops.dominators(body.blocks, body.entry)
    providers = {}
    for site, op in candidates:
        clobbers = graph.clobbers(site, op.loads[0], dgroup)
        access = accesses[next(iter(clobbers))] if len(clobbers) == 1 else None
        if access is None or access.kind is not memoryssa.Kind.DEF or access.site is None:
            continue
        source = access.site
        if source.block not in dominators[site.block] or source.block == site.block and source.index >= site.index:
            continue
        store = graph.operations[source]
        if (
            store.kind is mir.Kind.STORE
            and not store.barrier
            and not store.floating
            and not store.loads
            and not store.defines
            and not store.merges
            and len(store.stores) == len(store.args) == 1
            and graph.pointers.same_bytes(op.loads[0], store.stores[0])
            and isinstance(arg := store.args[0], (mir.Const, mir.Held))
            and arg.width == op.loads[0].width
        ):
            providers[op.results[0].value] = arg
    return providers


def known(
    body: mir.MirBody,
    dgroup: frozenset[int] | None = None,
    calls: dict[int, str] | None = None,
    *,
    edges: dict[tuple[int, int], Cells] | None = None,
    initial: Cells | None = None,
) -> dict[mir.Value, Known]:
    """Every value this body computes that is a number, to a fixed point.

    Forward over the blocks until nothing new is learned. A value's fact
    only ever goes from unknown to known and never changes once set --
    which is SSA's own doing, since the value is defined once -- so the
    walk terminates on the count of values rather than on any ordering.
    """
    # Optimistic, then shrinking. A run may assume every selector it does
    # not know is some absolute segment; the ones that came out numbers keep
    # the assumption and the rest lose it, and the run is repeated until
    # every selector still assumed resolved. All or nothing threw the answer
    # away whenever one body had a selector that never could resolve -- a
    # $DYNAMIC array's, which is every one of qbdemo's 213.
    cache = _reuse.get()
    key = _reuse_key(body, dgroup, calls, edges, initial)
    if cache is not None and key is not None and (saved := cache.get(key)) is not None and saved[0] is body:
        # Analysis consumers receive a normal mutable dictionary.  Preserve
        # that API without letting one consumer corrupt the transaction's
        # retained immutable-body result.
        return dict(saved[1])
    allowed: frozenset[mir.Value] | None = None
    while True:
        got, assumed = _solved(body, dgroup, calls, edges=edges, initial=initial, assume=set(), allowed=allowed)
        resolved = frozenset(value for value in assumed if value in got)
        if resolved == assumed:
            if cache is not None and key is not None:
                cache[key] = (body, dict(got))
            return got
        allowed = resolved


def _solved(
    body: mir.MirBody,
    dgroup: frozenset[int] | None,
    calls: dict[int, str] | None,
    *,
    edges: dict[tuple[int, int], Cells] | None,
    initial: Cells | None,
    assume: "set[mir.Value] | None",
    allowed: "frozenset[mir.Value] | None" = None,
) -> "tuple[dict[mir.Value, Known], set[mir.Value]]":
    facts: dict[mir.Value, Known] = {}
    carries: dict[mir.Value, int] = {}
    held: dict[tuple[int, int], Cells] = {}
    pointer_stores = _pointer_stores(body, dgroup) if dgroup is not None and calls is not None else {}
    changing = True
    while changing:
        changing = False
        # What memory holds, recomputed from what is known so far. The two
        # feed each other: a cell is known because a value was stored to it,
        # and a value is known because it was read from a cell. Running them
        # to one fixed point together is what lets `n = 7 : k = 3` reach the
        # `n * k` inside the loop, which is three statements and a store
        # away.
        if dgroup is not None and calls is not None:
            held = cells(body, dgroup, calls, facts, edges=edges, initial=initial, assume=assume, allowed=allowed)
        for block in body.blocks:
            # A join is known where every path into it agrees. Nothing else
            # about a phi is knowable -- and this is what makes the
            # propagation cross-block rather than merely whole-body: a value
            # defined in one branch and read after the join was invisible
            # until the phi carrying it could be a number too.
            for phi in block.phis:
                if phi.result in facts or not phi.incoming:
                    continue
                seen = [facts.get(one) for one in phi.incoming.values()]
                known = [one for one in seen if one is not None]
                if len(known) != len(seen) or len({(o.n, o.width) for o in known}) != 1:
                    continue
                facts[phi.result] = known[0]
                changing = True
            for index, op in enumerate(block.ops):
                here = held.get((block.at, index), {})
                carry = _carry(op, facts, here)
                if carry is not None:
                    for value in op.defines:
                        if value.flags and value not in carries:
                            carries[value] = carry
                            changing = True
                target = _defined(op)
                if target is None or target in facts:
                    continue
                found = _result(op, facts, None, here, carries)
                if found is None and (source := pointer_stores.get(target)) is not None:
                    found = _operand(op, source, facts)
                if found is not None:
                    facts[target] = found
                    changing = True
    from qbopt.analysis import constant_cycles

    return constant_cycles.propagated(body, facts), (assume or set())
