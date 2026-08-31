"""
qbopt/layout.py: a whole body emitted, and everything that moves with it.

The claim is not that the bytes match. select.py may choose an encoding BC
did not -- it emits the near form of every branch where BC often wrote the
short one -- so the body is a different length and every address in it
shifts. The claim is that it is the same program: the same instructions in
the same order, and every branch pointing at the instruction it pointed at
before rather than at whatever now sits at the old address.
"""

from pathlib import Path
from collections.abc import Iterator

import pytest
from iced_x86 import OpKind
from iced_x86 import Decoder
from iced_x86 import Instruction

import corpus
from qbopt import mir
from qbopt import layout
from qbopt.declen import BITNESS
from qbopt import blocks as split
from qbopt.rewrite import code_map

FIXTURES = sorted(Path("fixtures/omf").glob("*.obj"))


def original(op: mir.Op) -> Instruction:
    """The instruction an op came from.

    Narrowed here rather than at each use: only nodes that carry one reach
    these tests, since a body holding anything else does not lay out.
    """
    node = op.node
    assert node is not None
    found = getattr(node, "insn", None)
    assert found is not None
    return found.insn


def laid(obj: Path) -> Iterator[tuple]:
    """Every body of the object that lays out, with what it produced."""
    found = corpus.loaded(obj)
    if found is None:
        return
    mapped = code_map(found)
    if isinstance(mapped, str):
        return
    for _, body in mir.bodies(found, split.partition(found, mapped)):
        got = layout.lay_out(body, body.entry, found)
        if not isinstance(got, str):
            yield body, got


@pytest.mark.parametrize("obj", FIXTURES, ids=lambda p: p.stem)
def test_a_laid_out_body_is_the_same_instructions_in_the_same_order(obj: Path) -> None:
    for body, got in laid(obj):
        ops = layout._ordered(body)
        back = list(Decoder(BITNESS, got.code, ip=body.entry))
        assert len(back) == len(ops), f"{obj.stem}: {len(back)} instructions from {len(ops)} ops"
        for op, made in zip(ops, back, strict=True):
            want = original(op)
            assert made.mnemonic == want.mnemonic, f"{obj.stem} {op.at:#x}: {made} != {want}"


@pytest.mark.parametrize("obj", FIXTURES, ids=lambda p: p.stem)
def test_every_branch_points_where_its_target_went(obj: Path) -> None:
    """The whole reason layout exists.

    An instruction whose encoding differs in length from BC's moves
    everything after it, and a branch left with its old number then points
    at whatever now sits there -- which is a working program that does
    something else, the worst kind of wrong.
    """
    for body, got in laid(obj):
        ops = layout._ordered(body)
        back = list(Decoder(BITNESS, got.code, ip=body.entry))
        for op, made in zip(ops, back, strict=True):
            want = original(op)
            if want.op0_kind != OpKind.NEAR_BRANCH16:
                continue
            landed = got.moved.get(want.near_branch16)
            assert landed is not None, f"{obj.stem} {op.at:#x}: target left this body"
            assert made.near_branch16 == landed, f"{obj.stem} {op.at:#x}: {made} should reach {landed:#x}"


@pytest.mark.parametrize("obj", FIXTURES, ids=lambda p: p.stem)
def test_every_relocation_points_at_a_field_the_module_really_has(obj: Path) -> None:
    """A relocated displacement goes out as zero, so the fixup naming it has
    to move. A relocation naming a field that is not one leaves the value
    reading a bare zero at run time -- silent, and the shape tools/mutate.py
    calls bridged-fixup-dropped."""
    found = corpus.loaded(obj)
    assert found is not None
    for _, got in laid(obj):
        for where, field in got.relocations:
            assert 0 <= where < len(got.code), f"{obj.stem}: relocation past the end"
            assert got.code[where : where + 2] == bytes(2), "a relocated field is not a number"
            known = field in found.fixup_at or field - 1 in found.calls
            assert known, f"{obj.stem}: {field:#x} is not a field this module relocates"


def test_a_body_is_refused_whole_or_not_at_all() -> None:
    """Half this pass's code and half BC's is not something anything
    downstream could reason about, so one op it cannot emit refuses the
    body. Measured: 50 of the corpus's 171 bodies lay out, and the rest name
    the operation that stopped them."""
    total = done = 0
    for obj in FIXTURES:
        found = corpus.loaded(obj)
        if found is None:
            continue
        mapped = code_map(found)
        if isinstance(mapped, str):
            continue
        for _, body in mir.bodies(found, split.partition(found, mapped)):
            total += 1
            got = layout.lay_out(body, body.entry, found)
            if isinstance(got, str):
                assert ":" in got, f"a refusal should say which op: {got}"
            else:
                done += 1
    assert (total, done) == (171, 50)
