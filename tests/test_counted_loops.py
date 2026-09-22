"""`induction.counted` over any unit loop: runtime start, `<` or `<=`, signed or not."""

import re
from pathlib import Path

import pytest

from qbopt.model import mir
from qbopt.backend import masm
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


def test_a_runtime_lower_bound_loop_ends_on_its_shared_offset(dynamic_sum: Path) -> None:
    """QB's `lbound TO ubound` loop kept `lea bx,[edx+edx]` and `inc; cmp; jle` every trip.

    Only `0 ..< n` unsigned was a counted loop, so the element offset was
    never a recurrence and the index stayed. C and modern ended on `add bx, 2`.
    """
    text = masm.text(qb_compile.assembled(driver.parsed(dynamic_sum)))
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
    near = (Space.LITERAL, 0)
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
            put(("selector", SELECTOR), data + 2 * index, 10**number * (index + 1), 2)
    return values, memory


def _entry_and_exit(op: mir.Op, args: tuple[int, ...]) -> tuple[int, ...]:
    if op.name not in ("B$ENRA", "B$EXSA"):
        raise execute.ExecutionError(op.name)
    return ()


@pytest.mark.parametrize(("low", "count"), [(0, 4), (5, 4), (-2, 3), (1, 1), (7, 0), (-1, 0)])
def test_a_runtime_lower_bound_loop_runs_its_source_trips(dynamic_sum: Path, low: int, count: int) -> None:
    """Source, optimized and rotated MIR sum the same elements, an empty range included."""
    bodies = {}

    def observe(event: qb_compile.Stage) -> None:
        if event.name in ("source-mir", "optimized-mir") and event.function.name == "SUMTHREE":
            bodies[event.name] = event.value.body

    qb_compile.object_bytes(driver.parsed(dynamic_sum), dynamic_sum.name, observer=observe)
    optimized = bodies["optimized-mir"]
    expected = 111 * count * (count + 1) // 2
    for body in (bodies["source-mir"], optimized, rotate.entered(optimized)):
        got = execute.run(body, *_arrays(body, low, count), call=_entry_and_exit)
        assert got.returned == (expected,)
