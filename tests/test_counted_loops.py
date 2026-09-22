"""`induction.counted`, the one trip-count proof, against what the loops it names actually do."""

import re
import functools
from pathlib import Path

import pytest

from qbopt.model import ir
from qbopt.model import mir
from qbopt.backend import masm
from qbopt.analysis import loops
from qbopt.analysis import induction
from qbopt.model import execute
from qbopt.optimize import rotate
from qbopt.frontend.qb import driver
from qbopt.frontend.qb import compile as qb_compile
from qbopt.objectfile.module import Space

ROOT = Path(__file__).resolve().parents[1]
SELECTOR = 0x2000


@pytest.fixture(scope="module")
def dynamic_sum(tmp_path_factory: pytest.TempPathFactory) -> Path:
    """sum_three.bas without STATIC: `FOR index = LBOUND(first) TO UBOUND(first)`, index dead after."""
    text = (ROOT / "bench" / "parity" / "sum_three.bas").read_text()
    source = tmp_path_factory.mktemp("qb") / "SUMDYN.BAS"
    source.write_text(text.replace("as integer static\n", "as integer\n"))
    return source


@pytest.mark.parametrize("static", [False, True])
def test_a_runtime_lower_bound_loop_ends_on_its_shared_offset(dynamic_sum: Path, static: bool) -> None:
    """QB's `lbound TO ubound` loop kept `lea bx,[edx+edx]` and `inc; cmp; jle` every trip.

    Only `0 ..< n` unsigned was a counted loop, so the element offset was
    never a recurrence and the index stayed. C and modern ended on `add bx, 2`.
    STATIC then kept the index for its exit store alone.
    """
    source = ROOT / "bench" / "parity" / "sum_three.bas" if static else dynamic_sum
    text = masm.text(qb_compile.assembled(driver.parsed(source)))
    procedure = text[text.index("SUMTHREE proc") : text.index("SUMTHREE endp")]
    loop = _backward_loop(procedure)

    assert re.search(r"add \w\w, 2\n", loop)
    assert not re.search(r"    (?:inc|dec|lea|cmp) ", loop)


def _backward_loop(procedure: str) -> str:
    """From the first label a later jump returns to, through that jump."""
    for jump in re.finditer(r"\bj\w+ (\w+)\n", procedure):
        start = procedure.find(jump.group(1) + ":\n")
        if 0 <= start < jump.start():
            return procedure[start : jump.end()]
    raise AssertionError("no loop")


def _arrays(body: mir.MirBody, low: int, count: int) -> tuple[dict, dict]:
    """Three allocated one-dimensional INTEGER arrays `low TO low+count-1`."""
    near = execute.DS
    free = sorted(
        {value for block in body.blocks for op in block.ops for value in op.uses if not value.flags} - set(body.values),
        key=lambda value: value.id,
    )
    values, memory = {}, {}

    def put(region: object, at: int, n: int, width: int) -> None:
        memory.update({(region, (at + byte) & 0xFFFF): n >> 8 * byte & 0xFF for byte in range(width)})

    for number, value in enumerate(free):
        descriptor, data = 0x100 + 0x20 * number, 0x400 + 0x100 * number
        values[value] = descriptor
        put(near, descriptor + 2, SELECTOR, 2)
        put(near, descriptor + 8, 1, 1)
        put(near, descriptor + 10, data - 2 * low, 2)
        put(near, descriptor + 14, count, 2)
        put(near, descriptor + 16, low, 2)
        for index in range(count):
            put(SELECTOR, data + 2 * index, 10**number * (index + 1), 2)
    return values, memory


def _entry_and_exit(op: mir.Op, args: tuple[int, ...]) -> tuple[int, ...]:
    if op.name not in ("B$ENRA", "B$EXSA"):
        raise execute.ExecutionError(op.name)
    return ()


@functools.cache
def _stages(source: Path) -> dict[str, mir.MirBody]:
    bodies = {}

    def observe(event: qb_compile.Stage) -> None:
        if event.name in ("source-mir", "optimized-mir") and event.function.name == "SUMTHREE":
            bodies[event.name] = event.value.body

    qb_compile.object_bytes(driver.parsed(source), source.name, observer=observe)
    return bodies


@pytest.mark.parametrize("static", [False, True])
@pytest.mark.parametrize(("low", "count"), [(0, 4), (5, 4), (-2, 3), (1, 1), (7, 0), (-1, 0)])
def test_a_runtime_lower_bound_loop_runs_its_source_trips(
    dynamic_sum: Path, static: bool, low: int, count: int
) -> None:
    """Source, optimized and rotated MIR sum the same elements and leave the same statics.

    STATIC keeps `index` after the loop: `ubound + 1` after a trip, `lbound` after none.
    """
    bodies = _stages(ROOT / "bench" / "parity" / "sum_three.bas" if static else dynamic_sum)
    optimized = bodies["optimized-mir"]
    runs = [
        execute.run(body, *_arrays(body, low, count), call=_entry_and_exit)
        for body in (bodies["source-mir"], optimized, rotate.entered(optimized))
    ]
    statics = [
        {key: byte for key, byte in run.memory.items() if isinstance(key[0], tuple) and key[0][0] is Space.SEGMENT}
        for run in runs
    ]

    assert [run.returned for run in runs] == [(111 * count * (count + 1) // 2,)] * 3
    assert statics[1:] == statics[:1] * 2
    assert bool(statics[0]) is static


def _unit_loop(start: int, bound: int, exit_test: mir.Kind) -> mir.MirBody:
    """`i = start; while not (i exit_test bound): i += 1`, the counter otherwise unread."""
    seed = mir.Value(1, 0, variable=1, version=1)
    counter = mir.Value(2, 1, variable=1, version=2)
    following = mir.Value(3, 2, variable=1, version=3)
    flags = mir.Value(4, 1, flags=True, variable=2, version=1)
    initialize = mir.computed(0, mir.Kind.COPY, seed, (mir.Const(start, 2),), 2)
    compare = mir.Op(
        1, ir.Operation.COMPARE, "cmp", (flags,), (counter,),
        kind=mir.Kind.SUB, args=(mir.Held(counter, 2), mir.Const(bound, 2)),
    )  # fmt: skip
    branch = mir.Op(1, ir.Operation.BRANCH, "", (), (flags,), kind=mir.Kind.BRANCH, test=exit_test, target=3)
    increment = mir.computed(2, mir.Kind.INCREMENT, following, (mir.Held(counter, 2),), 2)
    jump = mir.Op(2, ir.Operation.JUMP, "", (), (), kind=mir.Kind.JUMP, target=1)
    returned = mir.Op(3, ir.Operation.RETURN, "", (), (), kind=mir.Kind.RETURN)
    return mir.MirBody(
        0,
        (
            mir.MirBlock(0, (), (initialize,), (1,)),
            mir.MirBlock(1, (mir.Phi(counter, {0: seed, 2: following}),), (compare, branch), (2, 3)),
            mir.MirBlock(2, (), (increment, jump), (1,)),
            mir.MirBlock(3, (), (returned,), ()),
        ),
        sealed=True,
    )


def _loop(
    start: int, bound: int, test: mir.Kind, step: int, shape: str, mirrored: bool = False, width: int = 1
) -> mir.MirBody:
    """`i = start` stepping by `step` while `i test bound`, returning trips and the last trip's `i`.

    `shape` is "pre" (tested before each trip), or "post" or "post-stepped"
    (after each, reading `i` or `i + step`). `mirrored` compares `bound` to `i`.
    """
    serial = iter(range(1, 100))

    def value(at: int, **flags: bool) -> mir.Value:
        n = next(serial)
        return mir.Value(n, at, variable=n, version=1, **flags)

    posttested = shape != "pre"
    body_at, exit_at = (1, 2) if posttested else (2, 3)
    seed, zero, counter, trips, seen = value(0), value(0), value(1), value(1), value(1)
    following, counted_up, saw, flags = value(body_at), value(body_at), value(body_at), value(1, flags=True)
    tested = mir.Held(following if shape == "post-stepped" else counter, width)
    args = (mir.Const(bound, width), tested) if mirrored else (tested, mir.Const(bound, width))
    compare = mir.Op(
        1, ir.Operation.COMPARE, "cmp", (flags,), (tested.value,), kind=mir.Kind.SUB, args=args
    )  # fmt: skip
    continuing = mir.MIRRORED[test] if mirrored else test
    branch = mir.Op(
        1, ir.Operation.BRANCH, "", (), (flags,), kind=mir.Kind.BRANCH,
        test=continuing if posttested else mir.NEGATED[continuing], target=1 if posttested else exit_at,
    )  # fmt: skip
    trip = (
        mir.computed(body_at, mir.Kind.ADD, counted_up, (mir.Held(trips, 2), mir.Const(1, 2)), 2),
        mir.computed(body_at, mir.Kind.COPY, saw, (mir.Held(counter, width),), width),
        mir.computed(body_at, mir.Kind.ADD, following, (mir.Held(counter, width), mir.Const(step, width)), width),
    )
    phis = (
        mir.Phi(counter, {0: seed, body_at: following}),
        mir.Phi(trips, {0: zero, body_at: counted_up}),
        mir.Phi(seen, {0: seed, body_at: saw}),
    )
    left = (
        (mir.Held(trips, 2), mir.Held(seen, width))
        if not posttested
        else (mir.Held(counted_up, 2), mir.Held(saw, width))
    )
    returned = mir.Op(exit_at, ir.Operation.RETURN, "", (), (), kind=mir.Kind.RETURN, args=left)
    entry = mir.MirBlock(
        0,
        (),
        (
            mir.computed(0, mir.Kind.COPY, seed, (mir.Const(start, width),), width),
            mir.computed(0, mir.Kind.COPY, zero, (mir.Const(0, 2),), 2),
        ),
        (1,),
    )
    if posttested:
        blocks = (
            entry,
            mir.MirBlock(1, phis, (*trip, compare, branch), (1, 2)),
            mir.MirBlock(2, (), (returned,), ()),
        )
    else:
        jump = mir.Op(2, ir.Operation.JUMP, "", (), (), kind=mir.Kind.JUMP, target=1)
        blocks = (
            entry,
            mir.MirBlock(1, phis, (compare, branch), (2, 3)),
            mir.MirBlock(2, (), (*trip, jump), (1,)),
            mir.MirBlock(3, (), (returned,), ()),
        )
    return mir.MirBody(0, blocks, sealed=True)


def _signed(n: int, width: int) -> int:
    return n - (1 << 8 * width) if n >> 8 * width - 1 else n


TESTS = (*induction._ASCENDING, *induction._DESCENDING, mir.Kind.NE)
ENDS = (0, 1, 0x7F, 0x80, 0xFE, 0xFF)


@pytest.mark.parametrize("shape", ["pre", "post", "post-stepped"])
@pytest.mark.parametrize("step", [1, -1, 3, -3])
def test_every_counted_loop_runs_its_proved_trips(shape: str, step: int) -> None:
    """The proof, over every byte loop of these ends and tests, against running it.

    A loop the executor sees end has its exact count, or no proof; one that
    never ends has no proof. The last trip's counter is `last` wherever given.
    """
    counted = []
    for test in TESTS:
        for start in ENDS:
            for bound in ENDS:
                for mirrored in (False, True):
                    body = _loop(start, bound, test, step, shape, mirrored)
                    (loop,) = loops.loops(body.blocks, body.entry)
                    proofs = induction.counted(body, loop)
                    try:
                        trips, seen = execute.run(body, limit=1_700).returned
                    except execute.ExecutionError:
                        assert not proofs, (test, start, bound, mirrored)
                        continue
                    if not proofs:
                        continue
                    (proof,) = proofs
                    where = (test, start, bound, mirrored, trips)
                    assert proof.count == trips and proof.test is test, where
                    if proof.last is not None:
                        assert (proof.first, proof.last) == (_signed(start, 1), _signed(seen, 1)), where
                    counted.append(test)
    # Every test whose direction the step can end is proved somewhere.
    ending = {test for test in TESTS if test is mir.Kind.NE or (test in induction._ASCENDING) == (step > 0)}
    assert set(counted) == ending


@pytest.mark.parametrize(
    ("start", "step", "bound", "last"),
    [
        (7, 3, 37, 34),
        (37, -3, 7, 10),
        (0, 1, 32767, 32766),
        (0, -1, -32768, -32767),
        (7, 3, 38, None),  # reached only after wrapping
        (7, -3, 37, None),
        (7, 3, 7, None),  # no trip
        (32767, 1, -32768, None),  # its exit value wraps
    ],
)
def test_a_not_equal_loop_knows_its_last_trip_only_without_wrapping(
    start: int, step: int, bound: int, last: int | None
) -> None:
    """IVARM lost its ten-trip proof when IndVarSimplify changed `<= 10` to `!= 37`."""
    body = _loop(start, bound, mir.Kind.NE, step, "pre", width=2)
    (loop,) = loops.loops(body.blocks, body.entry)
    (proof,) = induction.counted(body, loop)

    assert proof.last == last


@pytest.mark.parametrize(
    ("start", "bound", "exit_test", "trips"),
    [
        (0, 0x7FFF, mir.Kind.GT, None),  # signed <= its maximum never fails
        (0, 0x7FFE, mir.Kind.GT, 0x7FFF),
        (0, 0xFFFF, mir.Kind.ABOVE, None),  # unsigned <= its maximum never fails
        (1, 0xFFFE, mir.Kind.ABOVE, 0xFFFE),
        (-3, 2, mir.Kind.GE, 5),
    ],
)
def test_an_inclusive_test_at_its_types_maximum_is_not_counted(
    start: int, bound: int, exit_test: mir.Kind, trips: int | None
) -> None:
    """`i = 0; while i <= 32767` was proved to run 32768 trips: `i + 1` wraps and it never ends."""
    body = _unit_loop(start, bound, exit_test)
    (loop,) = loops.loops(body.blocks, body.entry)
    proofs = induction.counted(body, loop)

    assert [proof.maximum for proof in proofs] == ([] if trips is None else [trips])


PAIRS = """\
long pairs(unsigned short steps)
{
    short x[4], v[4];
    unsigned short step, i, j;

    x[0] = 3; x[1] = -5; x[2] = 7; x[3] = 11;
    v[0] = 0; v[1] = 1; v[2] = 0; v[3] = -1;
    for (step = 0; step < steps; ++step) {
        for (i = 0; i < 4; ++i)
            for (j = (unsigned short)(i + 1); j < 4; ++j) {
                short d = x[j] - x[i];
                v[i] += d; v[j] -= d;
            }
        for (i = 0; i < 4; ++i)
            x[i] += v[i];
    }
    return (long)x[0] * 1000 + x[1] * 100 + x[2] * 10 + x[3];
}
"""


@pytest.fixture(scope="module")
def pairs(tmp_path_factory: pytest.TempPathFactory) -> tuple[mir.MirBody, mir.MirBody]:
    """PAIRS's MIR as raised and as optimized."""
    from qbopt import flow
    from qbopt.cfront import compile as cfront

    source = tmp_path_factory.mktemp("c") / "pairs.c"
    source.write_text(PAIRS)
    seen = []
    optimized = flow.optimized

    def watched(body: mir.MirBody, *args: object, **kwargs: object) -> mir.MirBody:
        seen.append((body, optimized(body, *args, **kwargs)))
        return seen[-1][1]

    with pytest.MonkeyPatch.context() as patch:
        patch.setattr(flow, "optimized", watched)
        cfront.compiled(cfront.recorded(source, []), "pairs", optimise=True)
    return seen[0]


@pytest.mark.parametrize("steps", [0, 1, 2, 5])
def test_a_c_loop_from_a_runtime_start_runs_its_source_trips(
    pairs: tuple[mir.MirBody, mir.MirBody], steps: int
) -> None:
    """nbody's `for (j = i + 1; j < 4; ++j)`: counted from a start the outer loop computes."""
    before, after = pairs
    argument = {(execute.SS, 6 + byte): steps >> 8 * byte & 0xFF for byte in range(2)}

    got = [execute.run(body, {}, argument).returned for body in (before, after, rotate.entered(after))]
    assert got[1:] == got[:1] * 2
