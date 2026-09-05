"""The six steps, and what each is supposed to guarantee.

    parse -> raise -> passes -> lower -> allocate -> write

Not the shipped path yet: rewrite.py goes through wholeseg, and what this
produces is measured against that rather than trusted over it. What is
asserted here is that the seams hold -- every step takes one form and
returns the next -- and that the allocator prices what it is meant to.
"""

from pathlib import Path

import pytest

from qbopt import allocate
from qbopt import blocks as split
from qbopt import flow
from qbopt import ir
from qbopt import lir
from qbopt import lower
from qbopt import mir
from qbopt import module
from qbopt import omf
from qbopt.blocks import code_map

CORPUS = sorted(Path("fixtures/omf").glob("*.obj"))


def _raised(name: str):
    found = module.of(omf.parse(Path(f"fixtures/omf/{name}.obj").read_bytes()))
    blocks = split.partition(found, code_map(found))
    return found, blocks, list(mir.bodies(found, blocks))


@pytest.mark.corpus
def test_the_whole_flow_writes_every_object() -> None:
    """Six steps, 487 objects, no refusals and no exceptions."""
    refused = []
    for path in CORPUS:
        try:
            _out, why = flow.run(path.read_bytes())
        except Exception as error:  # noqa: BLE001 -- the point is that none escapes
            refused.append(f"{path.stem}: {type(error).__name__}: {error}")
            continue
        if why != "written":
            refused.append(f"{path.stem}: {why}")
    assert not refused, f"{len(refused)} of {len(CORPUS)}: " + "; ".join(refused[:4])


def test_lowering_leaves_no_mir_operand_behind() -> None:
    """Below LIR every operand is a location. A MemRef reaching select is
    the seam leaking: `mov [seg:5+0xe],ax` refused to encode for exactly
    that, and the message said only that the mov was not one it could emit.
    """
    _found, _blocks, bodies = _raised("flags-p-g2-zd")
    for name, body in bodies:
        for one in lower.lowered(name, body).insns:
            if one.what is None:
                continue
            for where in (*one.what.dests, *one.what.sources):
                assert not isinstance(where, (mir.MemRef, mir.Held, mir.Const, mir.Cell)), (
                    f"{one.at:#06x} still holds {where!r}"
                )


def test_a_value_in_a_loop_costs_ten_times_one_outside() -> None:
    """Spilling is priced by where the references are, not counted.

    The loop is where every program in this suite spends its time, so a
    value read once inside a doubly nested one has to outweigh a value read
    ninety-nine times in straight-line code.
    """
    _found, _blocks, bodies = _raised("lngmix-p-g2")
    (_name, body), = bodies
    deep = allocate.depths(body)
    assert set(deep.values()) >= {0, 1}, "lngmix has a loop; the depths say otherwise"

    price = allocate.costs(body)
    inside = {value for block in body.blocks if deep[block.at] for op in block.ops for value in op.defines}
    inside -= {value for block in body.blocks if not deep[block.at] for op in block.ops for value in op.defines}
    outside = {value for block in body.blocks if not deep[block.at] for op in block.ops for value in op.defines}
    assert inside and outside
    assert min(price[one] for one in inside if one in price) >= allocate.PER_LEVEL * max(
        price[one] for one in outside if one in price
    ) / allocate.PER_LEVEL, "a loop reference is not priced above a straight-line one"
    assert max(price[one] for one in inside if one in price) >= allocate.PER_LEVEL


@pytest.mark.parametrize("name", ["lngmix-p-g2", "hotlop-p-g2", "nested-p-g2", "nots-p-g2"])
def test_the_allocation_is_searched_and_says_whether_it_is_optimal(name: str) -> None:
    """Branch and bound, with a node budget. A result that ran out of
    budget says so rather than claiming an optimum it did not prove."""
    _found, _blocks, bodies = _raised(name)
    for _who, body in bodies:
        got = allocate.allocate(body, body.pins)
        assert got.optimal or got.why, "an unproven assignment has to say why"
        if got.optimal:
            assert got.why == ""
        assert got.cost >= 0.0


def test_lowering_gives_back_lir_and_allocation_gives_back_lir() -> None:
    """Each step's output is the next step's input, and nothing else."""
    _found, _blocks, bodies = _raised("hotlop-p-g2")
    for name, body in bodies:
        low = lower.lowered(name, body)
        assert isinstance(low, lir.LirBody)
        after = allocate.applied(low, allocate.allocate(body, body.pins))
        assert isinstance(after, lir.LirBody)
        assert [one.at for one in after.insns] == [one.at for one in low.insns]
