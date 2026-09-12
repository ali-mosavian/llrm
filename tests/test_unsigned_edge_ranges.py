from dataclasses import replace

import pytest
from test_edge_ranges import guarded_loop

from qbopt.model import mir
from qbopt.analysis import ranges


@pytest.mark.parametrize("kind", [mir.Kind.ABOVE, mir.Kind.ABOVE_EQ, mir.Kind.BELOW, mir.Kind.BELOW_EQ])
@pytest.mark.parametrize("span", [(1, 3), (255, 255), (256, 260), (-3, -1), (-2, 2)])
@pytest.mark.parametrize("successor", [30, 40])
@pytest.mark.parametrize("width", [2, 4])
def test_unsigned_edge_never_removes_a_possible_selector(
    kind: mir.Kind, span: tuple[int, int], successor: int, width: int
) -> None:
    block = guarded_loop().blocks[2]
    compare, branch = block.ops
    counter = compare.args[0].value
    block = replace(
        block,
        ops=(
            replace(compare, args=(replace(compare.args[0], width=width), mir.Const(255, width))),
            replace(branch, test=kind),
        ),
    )
    known = {counter: ranges.Interval(*span, width)}
    answers = {
        mir.Kind.ABOVE: lambda value: value > 255,
        mir.Kind.ABOVE_EQ: lambda value: value >= 255,
        mir.Kind.BELOW: lambda value: value < 255,
        mir.Kind.BELOW_EQ: lambda value: value <= 255,
    }
    possible = any(
        answers[kind](number & ((1 << (8 * width)) - 1)) == (successor == branch.target)
        for number in range(span[0], span[1] + 1)
    )
    result = ranges.on_edge(block, successor, known)
    assert (result is not None) == possible
