from pathlib import Path
from dataclasses import replace

import pytest

import corpus
from qbopt.frontend.blocks import Ends
from qbopt.objectfile.module import Space
from qbopt.frontend.blocks import partition
from qbopt.frontend.blocks import dispatch_targets


def test_on_goto_does_not_reach_unrelated_relocated_labels(fixtures: Path) -> None:
    # jumptable's statement table at 0xea is not an ON GOTO destination.
    blocks = corpus.partitioned(fixtures / "jumptable.obj")
    dispatch = next(block for block in blocks if block.ends is Ends.TABLE)
    assert dispatch.succ == (0x46, 0x52, 0x5E)


@pytest.mark.parametrize(
    ("name", "targets"),
    [
        ("jumps-q-O.obj", (0x5C, 0x75, 0x8E)),
        ("jumps-p-g2.obj", (0x5E, 0x77, 0x90)),
        ("jumps-v-g2.obj", (0x5E, 0x77, 0x90)),
        ("jumps-v-g3.obj", (0x58, 0x71, 0x8A)),
        ("jumps-q-evt.obj", (0x86, 0xB3, 0xE0)),
        ("jumps-p-evt.obj", (0x7C, 0x9E, 0xC0)),
        ("jumps-v-evt.obj", (0x73, 0x92, 0xB1)),
    ],
)
def test_real_compiler_dispatch_destinations(fixtures: Path, name: str, targets: tuple[int, ...]) -> None:
    found = corpus.loaded(fixtures / name)
    assert found is not None
    dispatch = next(
        block for block in corpus.partitioned(fixtures / name) if found.calls.get(block.insns[-1].at) == "B$OGTA"
    )
    assert dispatch_targets(found, dispatch.insns[-1]) == targets


def test_dispatch_preserves_case_order_and_repeated_destinations(fixtures: Path) -> None:
    original = corpus.loaded(fixtures / "jumptable.obj")
    assert original is not None
    mapped = corpus.mapped(fixtures / "jumptable.obj")
    assert not isinstance(mapped, str)
    operands = dict(original.operands)
    operands[0x40], operands[0x42], operands[0x44] = operands[0x44], operands[0x40], operands[0x44]
    changed = replace(original, operands=operands)
    dispatch = next(block for block in partition(changed, mapped) if block.ends is Ends.TABLE)
    assert dispatch_targets(changed, dispatch.insns[-1]) == (0x5E, 0x46, 0x5E)
    assert dispatch.succ == (0x46, 0x5E)


@pytest.mark.parametrize("invalid", ["missing", "misaligned", "external", "segment", "outside"])
def test_unproved_table_keeps_conservative_edges(fixtures: Path, invalid: str) -> None:
    original = corpus.loaded(fixtures / "jumptable.obj")
    assert original is not None
    mapped = corpus.mapped(fixtures / "jumptable.obj")
    assert not isinstance(mapped, str)
    operands = dict(original.operands)
    match invalid:
        case "missing":
            del operands[0x42]
        case "misaligned":
            operands[0x43] = operands.pop(0x42)
        case "external":
            operands[0x42] = replace(operands[0x42], space=Space.EXTERNAL)
        case "segment":
            operands[0x42] = replace(operands[0x42], index=original.seg + 1)
        case "outside":
            operands[0x42] = replace(operands[0x42], disp=original.end)
    changed = replace(original, operands=operands)
    dispatch = next(block for block in partition(changed, mapped) if block.ends is Ends.TABLE)
    assert dispatch_targets(changed, dispatch.insns[-1]) is None
    assert dispatch.succ == (0x46, 0x52, 0x5E, 0xEA)
