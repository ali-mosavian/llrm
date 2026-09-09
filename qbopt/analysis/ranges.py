"""Non-wrapping integer intervals, scoped to the taken body of a counted loop."""

from dataclasses import dataclass, replace

from qbopt.analysis import consts, induction, loops
from qbopt.model import mir
from qbopt.objectfile.module import Space


@dataclass(frozen=True)
class Interval:
    low: int
    high: int
    width: int


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
    for loop in loops.loops(body.blocks, body.entry):
        inside = set(loop.body) - {loop.header}
        known = {}
        header = next(block for block in body.blocks if block.at == loop.header)
        phis = {phi.result.id: phi.result for phi in header.phis}
        counters = induction.basics(body, loop).values()
        trips = set()
        for counter in counters:
            width = counter.start.width
            last = induction._last_counter(body, loop, counter, facts, width)
            start = induction._signed(counter.start, facts, width)
            if last is not None and start is not None:
                known[phis[counter.value]] = Interval(min(start, last), max(start, last), width)
                step = induction._signed(counter.step, facts, width)
                trips.add((last - start) // step)
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
            destination = result.setdefault(at, {})
            for value, interval in known.items():
                previous = destination.get(value)
                if previous is None:
                    destination[value] = interval
                elif previous.width == interval.width:
                    low, high = max(previous.low, interval.low), min(previous.high, interval.high)
                    if low <= high:
                        destination[value] = Interval(low, high, interval.width)
    return result
