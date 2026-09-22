"""Non-wrapping integer intervals, scoped to the taken body of a counted loop."""

from dataclasses import replace
from dataclasses import dataclass

from qbopt.model import ir
from qbopt.model import mir
from qbopt.analysis import loops
from qbopt.analysis import consts
from qbopt.analysis import induction
from qbopt.objectfile.module import Space


@dataclass(frozen=True)
class Interval:
    low: int
    high: int
    width: int


def on_edge(block, successor, known, facts=None):
    """Signed comparison facts on one CFG edge; None means that edge is impossible.

    The flags must come from an explicit comparison in this block. Facts are
    about the operand's own width, never an implicit extension of that value.
    """
    if successor not in block.succ:
        raise ValueError("not a successor")
    result = dict(known)
    if not block.ops or len(block.succ) != 2:
        return result
    branch = block.ops[-1]
    flags = [value for value in branch.uses if value.flags]
    if branch.kind is not mir.Kind.BRANCH or branch.target not in block.succ or len(flags) != 1:
        return result
    compare = next((op for op in reversed(block.ops[:-1]) if flags[0] in op.defines), None)
    if (
        compare is None
        or compare.op is not ir.Operation.COMPARE
        or len(compare.args) != 2
        or compare.kind is not mir.Kind.SUB
        or compare.defines != (flags[0],)
        or compare.results
        or compare.loads
        or compare.stores
        or compare.merges
        or compare.barrier
        or compare.floating is not None
    ):
        return result
    left, right = compare.args
    if not all(isinstance(arg, (mir.Held, mir.Const)) for arg in (left, right)):
        return result
    if left.width not in (2, 4) or right.width != left.width:
        return result
    kind = branch.test
    if successor != branch.target:
        kind = mir.NEGATED.get(kind)
    sign = 1 << (left.width * 8 - 1)
    full = Interval(-sign, sign - 1, left.width)
    first = _operand(left, known, facts or {}) or full
    second = _operand(right, known, facts or {}) or full
    if kind in (mir.Kind.ABOVE, mir.Kind.ABOVE_EQ, mir.Kind.BELOW, mir.Kind.BELOW_EQ):
        first_low, first_high = _unsigned_span(first)
        second_low, second_high = _unsigned_span(second)
        match kind:
            case mir.Kind.ABOVE:
                possible = first_high > second_low
            case mir.Kind.ABOVE_EQ:
                possible = first_high >= second_low
            case mir.Kind.BELOW:
                possible = first_low < second_high
            case mir.Kind.BELOW_EQ:
                possible = first_low <= second_high
        return result if possible else None
    if kind in (mir.Kind.GE, mir.Kind.GT):
        left, right, first, second = right, left, second, first
        kind = mir.Kind.LE if kind is mir.Kind.GE else mir.Kind.LT
    match kind:
        case mir.Kind.LE | mir.Kind.LT:
            strict = int(kind is mir.Kind.LT)
            spans = (
                (first.low, min(first.high, second.high - strict)),
                (max(second.low, first.low + strict), second.high),
            )
        case mir.Kind.EQ:
            shared = max(first.low, second.low), min(first.high, second.high)
            spans = shared, shared
        case mir.Kind.NE:

            def excluding(interval, other):
                low, high = interval.low, interval.high
                if other.low == other.high:
                    low += int(low == other.low)
                    high -= int(high == other.low)
                return low, high

            spans = excluding(first, second), excluding(second, first)
        case _:
            return result
    for arg, (low, high) in zip((left, right), spans):
        if low > high:
            return None
        if isinstance(arg, mir.Held):
            result[arg.value] = Interval(low, high, arg.width)
    return result


def _unsigned_span(interval: Interval) -> tuple[int, int]:
    mask = (1 << (interval.width * 8)) - 1
    if interval.low < 0 <= interval.high:
        return 0, mask
    return interval.low & mask, interval.high & mask


def covering(ref: mir.MemRef, known: dict[mir.Value, Interval]) -> mir.MemRef:
    """A non-wrapping near indexed access as the static byte interval it can touch."""
    ref = mir._symbolic_ref(ref)
    if ref.addr is None or ref.addr.space is not Space.SEGMENT or ref.segment is not None:
        return ref
    interval = known.get(ref.base)
    if interval is None or interval.width != ref.base_width or ref.base_width != 2:
        return ref
    low, end = ref.addr.disp + interval.low, ref.addr.disp + interval.high + ref.width
    if not 0 <= low < end <= 1 << (8 * ref.base_width):
        return ref
    return replace(ref, addr=replace(ref.addr, disp=low, base=0), base=None, width=end - low)


def _operand(arg, known, facts):
    if isinstance(arg, mir.Const):
        number = consts.masked(arg.n, arg.width)
        sign = 1 << (arg.width * 8 - 1)
        number = (number ^ sign) - sign
        return Interval(number, number, arg.width)
    if not isinstance(arg, mir.Held):
        return None
    interval = known.get(arg.value)
    if interval is not None and interval.width == arg.width:
        return interval
    fact = facts.get(arg.value)
    if fact is not None and fact.width >= arg.width:
        return _operand(mir.Const(fact.n, arg.width), {}, {})
    return None


def _computed(op, known, facts):
    if op.loads or op.stores or op.barrier or len(op.results) != 1:
        return None
    result = op.results[0]
    if not isinstance(result, mir.Held) or result.width not in (2, 4):
        return None
    args = [_operand(arg, known, facts) for arg in op.args]
    if not args or any(arg is None for arg in args):
        return None
    first = args[0]
    if op.kind is mir.Kind.SIGN_EXTEND and len(args) == 1 and 0 < first.width < result.width:
        sign = 1 << (first.width * 8 - 1)
        return Interval(first.low, first.high, result.width) if -sign <= first.low <= first.high < sign else None
    if first.width != result.width:
        return None
    if op.kind is mir.Kind.COPY and len(args) == 1:
        return first
    if op.kind in (mir.Kind.INCREMENT, mir.Kind.DECREMENT) and len(args) == 1:
        step = 1 if op.kind is mir.Kind.INCREMENT else -1
        low, high = first.low + step, first.high + step
        sign = 1 << (result.width * 8 - 1)
        return Interval(low, high, result.width) if -sign <= low <= high < sign else None
    if len(args) != 2:
        return None
    second = args[1]
    if op.kind is mir.Kind.SHL:
        if second.low != second.high or not 0 <= second.low < result.width * 8:
            return None
        low, high = first.low << second.low, first.high << second.low
    elif second.width == result.width:
        match op.kind:
            case mir.Kind.ADD:
                low, high = first.low + second.low, first.high + second.high
            case mir.Kind.SUB:
                low, high = first.low - second.high, first.high - second.low
            case mir.Kind.MUL:
                products = [left * right for left in (first.low, first.high) for right in (second.low, second.high)]
                low, high = min(products), max(products)
            case _:
                return None
    else:
        return None
    sign = 1 << (result.width * 8 - 1)
    return Interval(low, high, result.width) if -sign <= low <= high < sign else None


def _recurrence_span(start: int, step: int, advances: int, width: int) -> Interval | None:
    """Taken values and the final latch update must all fit without wrapping."""
    last = start + advances * step
    sign = 1 << (width * 8 - 1)
    if advances >= 0 and -sign <= min(start, last, last + step) <= max(start, last, last + step) < sign:
        return Interval(min(start, last), max(start, last), width)
    return None


def bounded(body: mir.MirBody) -> dict[int, dict[mir.Value, Interval]]:
    facts = consts.known(body)
    result = {}
    predecessors = loops.predecessors(body.blocks)
    dominators = loops.dominators(body.blocks, body.entry)
    for loop in loops.loops(body.blocks, body.entry):
        inside = set(loop.body) - {loop.header}
        known = {}
        header = next(block for block in body.blocks if block.at == loop.header)
        phis = {phi.result.id: phi.result for phi in header.phis}
        counters = induction.basics(body, loop).values()
        trips = set()
        for proof in induction.counted(body, loop, facts):
            if proof.first is not None and proof.last is not None:
                span = min(proof.first, proof.last), max(proof.first, proof.last)
                known[proof.phi.result] = Interval(*span, proof.counter.start.width)
                trips.add(proof.count - 1)
        if len(trips) == 1:
            advances = next(iter(trips))
            for counter in counters:
                width = counter.start.width
                start = induction._signed(counter.start, facts, width)
                step = induction._signed(counter.step, facts, width)
                if start is None or step is None:
                    continue
                interval = _recurrence_span(start, step, advances, width)
                if interval is not None:
                    known[phis[counter.value]] = interval
        if not known:
            continue
        operations = [op for block in body.blocks if block.at in inside for op in block.ops]
        while True:
            before = len(known)
            for op in operations:
                if op.results and isinstance(op.results[0], mir.Held) and op.results[0].value not in known:
                    interval = _computed(op, known, facts)
                    if interval is not None:
                        known[op.results[0].value] = interval
            if len(known) == before:
                break
        for at in inside:
            scoped = dict(known)
            for block in body.blocks:
                for successor in block.succ:
                    if predecessors[successor] == {block.at} and successor in dominators.get(at, ()):
                        narrowed = on_edge(block, successor, scoped, facts)
                        if narrowed is not None:
                            scoped = narrowed
            while True:
                before = dict(scoped)
                for op in operations:
                    interval = _computed(op, scoped, facts)
                    if interval is None:
                        continue
                    value = op.results[0].value
                    previous = scoped.get(value)
                    if previous is not None and previous.width == interval.width:
                        low, high = max(previous.low, interval.low), min(previous.high, interval.high)
                        if low > high:
                            continue
                        interval = Interval(low, high, interval.width)
                    scoped[value] = interval
                if scoped == before:
                    break
            destination = result.setdefault(at, {})
            for value, interval in scoped.items():
                previous = destination.get(value)
                if previous is None:
                    destination[value] = interval
                elif previous.width == interval.width:
                    low, high = max(previous.low, interval.low), min(previous.high, interval.high)
                    if low <= high:
                        destination[value] = Interval(low, high, interval.width)
    return result


def dominated_edges(body: mir.MirBody) -> dict[int, dict[mir.Value, Interval]]:
    """Facts established by unavoidable branch edges at each block.

    This is the acyclic counterpart to :func:`bounded`.  A fact established
    on an edge is available below it only when that edge's destination has
    one predecessor and dominates the block being queried.  That deliberately
    refuses joins: a second way into the region is a second way around the
    check.  Address-form selection uses this to distinguish a non-negative
    word index, whose scaled 32-bit spelling is the same address, from a
    negative one where zero extension would change the wrapped word address.
    """
    facts = consts.known(body)
    predecessors = loops.predecessors(body.blocks)
    dominators = loops.dominators(body.blocks, body.entry)
    edges = [
        (block, successor)
        for block in body.blocks
        for successor in block.succ
        if predecessors.get(successor) == {block.at}
    ]
    edges.sort(key=lambda edge: len(dominators.get(edge[1], ())))
    result: dict[int, dict[mir.Value, Interval]] = {}
    for block in body.blocks:
        known: dict[mir.Value, Interval] = {}
        # Apply the path from outermost to innermost dominator once. Repeating
        # a relational ``a < b`` constraint would falsely walk both open
        # intervals inward rather than intersecting with one original fact.
        for parent, successor in edges:
            if successor not in dominators.get(block.at, ()):
                continue
            narrowed = on_edge(parent, successor, known, facts)
            if narrowed is not None:
                known = narrowed
        if known:
            result[block.at] = known
    return result


def constants(body: mir.MirBody, dgroup: frozenset[int] | None = None, calls: dict[int, str] | None = None) -> dict:
    """Every value `consts` knows, as the singleton interval an alias query reads.

    `bounded` knows loop counters, not that `es` was loaded with 0xA000; a far
    access through a literal selector is axiom 3's region only once its
    selector's value reaches the query.
    """
    return {value: Interval(fact.n, fact.n, fact.width) for value, fact in consts.known(body, dgroup, calls).items()}


def singletons(body: mir.MirBody) -> dict[mir.Value, Interval]:
    """Exact values computed without consulting memory.

    SROA needs only singleton indexes.  The full constant analysis also walks
    MemorySSA until scalar and memory facts agree, which is necessary for
    folding but needless for an address expression made solely from values.
    This small lattice reaches the same pure expressions without paying that
    compile-time cost at the pre-optimization boundary.
    """
    known: dict[mir.Value, Interval] = {}
    while True:
        before = len(known)
        for block in body.blocks:
            for phi in block.phis:
                if phi.result in known or not phi.incoming:
                    continue
                incoming = [known.get(value) for value in phi.incoming.values()]
                if incoming and None not in incoming and len(set(incoming)) == 1:
                    known[phi.result] = incoming[0]
            for op in block.ops:
                if not op.results or not isinstance(op.results[0], mir.Held):
                    continue
                result = op.results[0].value
                if result in known:
                    continue
                interval = _computed(op, known, {})
                if interval is not None and interval.low == interval.high:
                    known[result] = interval
        if len(known) == before:
            return known
