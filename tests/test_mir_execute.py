"""The MIR reference executor, the oracle loop transforms are checked against."""

from pathlib import Path
from dataclasses import replace

import pytest

from qbopt.model import mir
from qbopt.model import execute
from qbopt.backend import cpu as targets
from qbopt.frontend.modern import driver
from qbopt.frontend.modern import compile as modern_compile

ROOT = Path(__file__).resolve().parents[1]
SELECTOR = 0x1000


def slices(body: mir.MirBody, elements: int | tuple[int, ...]) -> tuple[dict, dict]:
    """`&[i16]` live-ins holding 1,2,.. then 10,20,.. then 100,200,.. of `elements` each."""
    defined = set(body.values)
    free = sorted(
        {value for block in body.blocks for op in block.ops for value in op.uses if not value.flags} - defined,
        key=lambda value: value.id,
    )
    region = ("selector", SELECTOR)
    values, memory = {}, {}
    for number, value in enumerate(free):
        length = elements if isinstance(elements, int) else elements[number]
        descriptor, data = 0x100 + 0x10 * number, 0x200 + 0x40 * number
        values[value] = SELECTOR << 16 | descriptor
        fields = length.to_bytes(2, "little") + bytes(2) + data.to_bytes(2, "little") + SELECTOR.to_bytes(2, "little")
        memory.update({(region, descriptor + at): byte for at, byte in enumerate(fields)})
        for index in range(length):
            element = (10**number * (index + 1)).to_bytes(2, "little")
            memory.update({(region, data + 2 * index + at): byte for at, byte in enumerate(element)})
    return values, memory


@pytest.fixture(scope="module")
def sum_three() -> tuple[mir.MirBody, mir.MirBody]:
    program = driver.parsed(ROOT / "frontends" / "modern" / "fixtures" / "sum_three.mod")
    function = next(one for module in program.modules for one in module.functions if one.name == "sum_three")
    semantic = next(one for one in modern_compile.semantic_lowered(program) if one.name.endswith("sum_three"))
    optimized = modern_compile.optimized(program, function, semantic, targets.profile("386"))
    return semantic.body, optimized.body


@pytest.mark.parametrize("elements", [0, 1, 4])
def test_frontend_mir_and_its_optimized_form_compute_the_same_sum(
    sum_three: tuple[mir.MirBody, mir.MirBody], elements: int
) -> None:
    expected = 111 * elements * (elements + 1) // 2
    for body in sum_three:
        values, memory = slices(body, elements)
        assert execute.run(body, values, memory).returned == (expected,)


def test_an_unmodelled_operation_raises_rather_than_guessing(sum_three: tuple[mir.MirBody, mir.MirBody]) -> None:
    semantic, _optimized = sum_three
    first = semantic.blocks[0]
    called = replace(first.ops[0], kind=mir.Kind.CALL, name="B$SOMETHING", args=(), results=())
    body = replace(semantic, blocks=(replace(first, ops=(called, *first.ops[1:])), *semantic.blocks[1:]))
    values, memory = slices(body, 1)

    with pytest.raises(execute.ExecutionError, match="call"):
        execute.run(body, values, memory)
    assert execute.run(body, values, memory, call=lambda op, args: ()).returned == (111,)
