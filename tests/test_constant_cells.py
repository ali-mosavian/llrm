"""BC's split-word initialization must supply a complete long, never missing bytes."""

from pathlib import Path

import pytest

import corpus
from qbopt import mir
from qbopt import consts
from qbopt import transform
from qbopt.module import Addr
from qbopt.module import Space


def test_long_from_two_word_stores() -> None:
    """LNGMIX's 100000 initializer was invisible to its four-byte dividend read."""
    address = Addr(Space.SEGMENT, 6, 5)
    cells = {(address, 2): consts.Known(0x86A0, 2), (address.plus(2), 2): consts.Known(1, 2)}
    assert consts._cell(cells, mir.MemRef(address, 4)) == consts.Known(100000, 4)


@pytest.mark.parametrize("variant", ["missing", "other_segment", "short_fact", "indexed"])
def test_incomplete_long_stays_unknown(variant: str) -> None:
    address = Addr(Space.SEGMENT, 6, 5)
    cells = {(address, 2): consts.Known(0x86A0, 2)}
    ref = mir.MemRef(address, 4)
    match variant:
        case "other_segment":
            cells[(Addr(Space.SEGMENT, 8, 6), 2)] = consts.Known(1, 2)
        case "short_fact":
            cells[(address.plus(2), 2)] = consts.Known(1, 1)
        case "indexed":
            cells[(address.plus(2), 2)] = consts.Known(1, 2)
            ref = mir.MemRef(address, 4, mir.Value(1, 0))
    assert consts._cell(cells, ref) is None


def test_subword_read_from_wide_fact() -> None:
    address = Addr(Space.SEGMENT, 6, 5)
    cells = {(address, 4): consts.Known(0x12345678, 4)}
    assert consts._cell(cells, mir.MemRef(address.plus(1), 2)) == consts.Known(0x3456, 2)


def test_lngmix_dividend_is_known_after_production_passes() -> None:
    path = Path("fixtures/omf/lngmix-p-g2.obj")
    module = corpus.loaded(path)
    assert module is not None
    blocks = corpus.partitioned(path)
    body = transform.applied(mir.bodies(module, blocks)[0][1], module.dgroup, module.calls, blocks=blocks, found=module)
    facts = consts.known(body, module.dgroup, module.calls)
    memory = consts.cells(body, module.dgroup, module.calls, facts)
    divides = [
        (block.at, index, op)
        for block in body.blocks
        for index, op in enumerate(block.ops)
        if op.kind is mir.Kind.DIVMOD
    ]
    assert divides
    for at, index, op in divides:
        assert consts._operand(op, op.args[0], facts, memory[at, index]) == consts.Known(100000, 4)
