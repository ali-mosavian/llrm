"""Small executable reference semantics for integer MIR.

An oracle for pass tests: run a body before and after a transform on the
same inputs and compare what each returns and stores. Anything not modelled
raises rather than guessing.

Memory is bytes keyed by segment and 16-bit offset, as the machine keys
them; `_where` derives both for every reference. A far reference's segment
is its selector's value. Otherwise the frame is SS, with `bp` zero, and a
near reference is DS -- or SS where it says it points into the frame. With
the stack in data, SS is DS. A named segment in DGROUP is DS at its base
offset; any other named cell is its own segment.
"""

from dataclasses import field
from dataclasses import dataclass
from collections.abc import Mapping
from collections.abc import Callable

from qbopt.model import mir
from qbopt.objectfile.module import Space
from iced_x86 import Register

# DS and SS. A far selector is a number, so neither is ever mistaken for one.
DS, SS = "ds", "ss"


class ExecutionError(ValueError):
    """The body uses something this executor does not model."""


@dataclass(frozen=True, slots=True)
class _Flags:
    kind: mir.Kind
    left: int
    right: int
    result: int
    width: int


@dataclass(slots=True)
class State:
    values: dict[mir.Value, int | _Flags] = field(default_factory=dict)
    memory: dict[tuple[object, int], int] = field(default_factory=dict)
    steps: int = 0
    stack: str = SS
    # A DGROUP segment's index and where it starts in DS.
    dgroup: Mapping[int, int] = field(default_factory=dict)


@dataclass(frozen=True, slots=True)
class Result:
    returned: tuple[int, ...]
    memory: dict[tuple[object, int], int]
    steps: int


type Call = Callable[[mir.Op, tuple[int, ...]], tuple[int, ...]]


def _mask(n: int, width: int) -> int:
    return n & ((1 << 8 * width) - 1)


def _signed(n: int, width: int) -> int:
    n = _mask(n, width)
    return n - (1 << 8 * width) if n >> (8 * width - 1) else n


def _compare(test: mir.Kind, left: int, right: int, width: int) -> bool:
    a, b = _signed(left, width), _signed(right, width)
    u, v = _mask(left, width), _mask(right, width)
    match test:
        case mir.Kind.EQ:
            return u == v
        case mir.Kind.NE:
            return u != v
        case mir.Kind.LT:
            return a < b
        case mir.Kind.LE:
            return a <= b
        case mir.Kind.GT:
            return a > b
        case mir.Kind.GE:
            return a >= b
        case mir.Kind.BELOW:
            return u < v
        case mir.Kind.BELOW_EQ:
            return u <= v
        case mir.Kind.ABOVE:
            return u > v
        case mir.Kind.ABOVE_EQ:
            return u >= v
    raise ExecutionError(f"no comparison {test}")


def _taken(flags: _Flags, test: mir.Kind) -> bool:
    if flags.kind is mir.Kind.SUB:
        return _compare(test, flags.left, flags.right, flags.width)
    # Logic leaves carry and overflow clear: every test is the result against zero.
    if flags.kind in (mir.Kind.AND, mir.Kind.OR, mir.Kind.XOR) or test in (mir.Kind.EQ, mir.Kind.NE):
        return _compare(test, flags.result, 0, flags.width)
    raise ExecutionError(f"{test} on the flags of {flags.kind}")


def _where(ref: mir.MemRef, state: State) -> tuple[object, int]:
    """The segment and offset of the first byte `ref` names."""
    addr = ref.addr
    if addr is None:
        raise ExecutionError(f"no address for {ref}")
    offset = addr.disp + (_integer(state, ref.base) if ref.base is not None else 0)
    if ref.segment is not None:
        return _integer(state, ref.segment), _mask(offset, 2)
    match addr.space:
        case Space.FRAME:
            segment = state.stack
        case Space.LITERAL:
            framed = ref.space is Space.FRAME or addr.segment == Register.SS
            segment = state.stack if framed else DS
        case Space.SEGMENT if addr.index in state.dgroup:
            segment, offset = DS, offset + state.dgroup[addr.index]
        case Space.SEGMENT | Space.EXTERNAL:
            segment = (addr.space, addr.index)
        case _:
            raise ExecutionError(f"no segment for {ref}")
    return segment, _mask(offset, 2)


def _load(state: State, ref: mir.MemRef, width: int) -> int:
    region, offset = _where(ref, state)
    return sum(state.memory.get((region, _mask(offset + at, 2)), 0) << 8 * at for at in range(width))


def _store(state: State, ref: mir.MemRef, n: int, width: int) -> None:
    region, offset = _where(ref, state)
    for at in range(width):
        state.memory[(region, _mask(offset + at, 2))] = n >> 8 * at & 0xFF


def _integer(state: State, value: mir.Value) -> int:
    got = state.values.get(value)
    if got is None:
        raise ExecutionError(f"{value!r} read before it is defined")
    if isinstance(got, _Flags):
        raise ExecutionError(f"{value!r} is flags, not a number")
    return got


def _read(state: State, arg: mir.Arg) -> int:
    match arg:
        case mir.Held(value, width):
            return _mask(_integer(state, value), width)
        case mir.Const(n, width):
            return _mask(n, width)
        case mir.Cell(ref):
            return _load(state, ref, ref.width)
        case mir.FrameAddress(offset, width):
            return _mask(offset, width)  # bp is zero
    raise ExecutionError(f"cannot read {arg!r}")


def _width(op: mir.Op) -> int:
    for one in (*op.results, *op.args):
        if isinstance(one, (mir.Held, mir.Const)):
            return one.width
        if isinstance(one, mir.Cell):
            return one.ref.width
    raise ExecutionError(f"no width for {op.kind}")


def _computed(op: mir.Op, args: tuple[int, ...], width: int) -> tuple[int, ...]:
    kind = op.kind
    bits = 8 * width
    match kind, args:
        case mir.Kind.COPY | mir.Kind.CONVERT | mir.Kind.ZERO_EXTEND | mir.Kind.LOAD | mir.Kind.ADDRESS, (a,):
            return (a,)
        case mir.Kind.SIGN_EXTEND, (a,):
            source = op.args[0]
            return (_signed(a, source.width) if isinstance(source, (mir.Held, mir.Const)) else a,)
        case mir.Kind.ADD, (a, b):
            return (a + b,)
        case mir.Kind.SUB, (a, b):
            return (a - b,)
        case mir.Kind.MUL, (a, b):
            return (_signed(a, width) * _signed(b, width),)
        case mir.Kind.AND, (a, b):
            return (a & b,)
        case mir.Kind.OR, (a, b):
            return (a | b,)
        case mir.Kind.XOR, (a, b):
            return (a ^ b,)
        case mir.Kind.SHL, (a, b):
            return (a << (b & 31),)
        case mir.Kind.SHR, (a, b):
            return (_mask(a, width) >> (b & 31),)
        case mir.Kind.SAR, (a, b):
            return (_signed(a, width) >> (b & 31),)
        case mir.Kind.NEG, (a,):
            return (-a,)
        case mir.Kind.NOT, (a,):
            return (~a,)
        case mir.Kind.INCREMENT, (a,):
            return (a + 1,)
        case mir.Kind.DECREMENT, (a,):
            return (a - 1,)
        case mir.Kind.DIV | mir.Kind.REM | mir.Kind.DIVMOD, (a, b):
            a, b = _signed(a, width), _signed(b, width)
            if b == 0:
                raise ExecutionError("division by zero")
            quotient = abs(a) // abs(b) * (1 if (a < 0) == (b < 0) else -1)
            remainder = a - quotient * b
            return {mir.Kind.DIV: (quotient,), mir.Kind.REM: (remainder,)}.get(kind, (quotient, remainder))
        case mir.Kind.UDIVMOD, (a, b):
            if b == 0:
                raise ExecutionError("division by zero")
            return (a // b, a % b)
        case mir.Kind.EXTRACT, (a, offset):
            return (a >> offset,)
        case mir.Kind.CONCAT, (high, low):
            low_width = op.args[1].width if isinstance(op.args[1], (mir.Held, mir.Const)) else width // 2
            return (high << 8 * low_width | _mask(low, low_width),)
    if kind in mir.MIRRORED and len(args) == 2:
        source = op.args[0]
        compared = source.width if isinstance(source, (mir.Held, mir.Const)) else width
        return (int(_compare(kind, args[0], args[1], compared)),)
    raise ExecutionError(f"{kind} with {len(args)} operands (bits {bits})")


def _executed(op: mir.Op, state: State, call: Call | None) -> None:
    if op.kind is mir.Kind.ADDRESS and len(op.args) == 1 and isinstance(op.args[0], mir.Cell):
        args = (_where(op.args[0].ref, state)[1],)
    else:
        args = tuple(_read(state, arg) for arg in op.args)
    if op.kind is mir.Kind.STORE:
        (target,) = op.results
        if not isinstance(target, mir.Cell):
            raise ExecutionError("a store without a cell")
        _store(state, target.ref, args[0], target.ref.width)
        return
    if op.kind is mir.Kind.CALL:
        if call is None:
            raise ExecutionError(f"call {op.name}")
        answers = call(op, args)
    else:
        width = _width(op)
        answers = tuple(_mask(n, width) for n in _computed(op, args, width))
    flags = [value for value in op.defines if value.flags]
    if flags and op.kind is not mir.Kind.CALL:
        width = _width(op)
        result = answers[0] if answers else _mask(args[0] - args[1], width) if op.kind is mir.Kind.SUB else args[0]
        left, right = (args + (0, 0))[:2]
        for value in flags:
            state.values[value] = _Flags(op.kind, left, right, result, width)
    for target, n in zip(op.results, answers, strict=False):
        match target:
            case mir.Held(value, width):
                state.values[value] = _mask(n, width)
            case mir.Cell(ref):
                _store(state, ref, n, ref.width)


def run(
    body: mir.MirBody,
    values: dict[mir.Value, int] | None = None,
    memory: dict[tuple[object, int], int] | None = None,
    *,
    call: Call | None = None,
    limit: int = 1_000_000,
    dgroup: Mapping[int, int] | None = None,
) -> Result:
    """Execute `body` from its entry until it returns.

    `values` are the live-in values; `memory` the bytes it starts with,
    keyed as `_where` keys them; `dgroup` each DGROUP segment's base in DS.
    """
    stack = DS if body.stack_in_data else SS
    state = State(dict(values or {}), dict(memory or {}), stack=stack, dgroup=dict(dgroup or {}))
    blocks = {block.at: block for block in body.blocks}
    for ref, constant in body.initial:
        _store(state, ref, constant.n, ref.width)
    at, came = body.entry, None
    while True:
        block = blocks[at]
        if came is not None:
            incoming = {phi.result: state.values.get(phi.incoming[came]) for phi in block.phis if came in phi.incoming}
            for result, got in incoming.items():
                if got is None:
                    raise ExecutionError(f"phi {result!r} reads an undefined value from b{came}")
                state.values[result] = got
        following = block.succ[0] if block.succ else None
        for op in block.ops:
            state.steps += 1
            if state.steps > limit:
                raise ExecutionError("step limit")
            match op.kind:
                case mir.Kind.NOTHING:
                    continue
                case mir.Kind.JUMP:
                    following = op.target if op.target is not None else following
                    break
                case mir.Kind.RETURN:
                    return Result(tuple(_read(state, arg) for arg in op.args), state.memory, state.steps)
                case mir.Kind.BRANCH:
                    (flags,) = [value for value in op.uses if value.flags]
                    got = state.values.get(flags)
                    if not isinstance(got, _Flags) or op.test is None:
                        raise ExecutionError(f"branch on {flags!r}")
                    others = [one for one in block.succ if one != op.target]
                    following = op.target if _taken(got, op.test) else (others[0] if others else op.target)
                    break
                case mir.Kind.SWITCH | mir.Kind.ESCAPE | mir.Kind.OPAQUE | mir.Kind.ARG | mir.Kind.RESULT:
                    raise ExecutionError(f"{op.kind} is not modelled")
                case _:
                    if op.floating is not None or op.kind.value.startswith("f"):
                        raise ExecutionError(f"{op.kind} is not modelled")
                    _executed(op, state, call)
        if following is None:
            raise ExecutionError(f"b{at} ends without a successor")
        at, came = following, at
