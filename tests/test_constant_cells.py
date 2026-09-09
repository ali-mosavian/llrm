"""BC's split-word initialization must supply a complete long, never missing bytes."""

from pathlib import Path

import pytest

import corpus
from qbopt.model import mir
from qbopt.analysis import consts
from qbopt.model import ir
from qbopt.objectfile.module import Addr
from qbopt.objectfile.module import Space


@pytest.mark.parametrize("indirect", ["base", "segment"])
def test_indirect_store_does_not_invent_a_direct_constant(indirect: str) -> None:
    """A store through a pointer reported 7 at the bare displacement, ignoring the pointer."""
    address = Addr(Space.SEGMENT, 6, 5)
    pointer = mir.Value(1, 0)
    ref = mir.MemRef(address, 2, **{indirect: pointer})
    store = mir.Op(0, ir.Operation.MOVE, "mov", (), (), kind=mir.Kind.STORE, args=(mir.Const(7, 2),), stores=(ref,))
    cells = consts._kills({}, store, {}, frozenset({5}), {})
    assert consts._cell(cells, mir.MemRef(address, 2)) is None


def test_proven_symbolic_store_and_read_share_constant() -> None:
    """HARR-style symbolic field references should retain the proven address, not displacement 2."""
    address = Addr(Space.SEGMENT, 8, 5)
    ref = mir.MemRef(Addr(Space.LITERAL, 2), 2, mir.Value(1, 0), symbolic=mir.Symbol(Space.SEGMENT, 5, 8, 2))
    store = mir.Op(0, ir.Operation.MOVE, "mov", (), (), kind=mir.Kind.STORE, args=(mir.Const(21, 2),), stores=(ref,))
    cells = consts._kills({}, store, {}, frozenset({5}), {})
    assert consts._cell(cells, mir.MemRef(address, 2)) == consts.Known(21, 2)
    assert consts._cell(cells, ref) == consts.Known(21, 2)
    assert consts._cell(cells, mir.MemRef(Addr(Space.LITERAL, 2), 2)) is None


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
    body = mir.bodies(module, blocks)[0][1]
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


@pytest.mark.parametrize("tag", ["p-g2", "q-O", "v-g3"])
def test_spill_uses_the_known_seven_as_an_immediate(tag):
    """SPILL reloaded invariant h3=7 on each of its hundred inner iterations."""
    from qbopt.frontend import blocks
    from qbopt.objectfile import module, omf
    from qbopt import wholeseg
    from iced_x86 import Code
    result = wholeseg.emitted(Path(f"fixtures/omf/spill-{tag}.obj").read_bytes())
    assert result.outcome is wholeseg.Emission.LIR, result.reason
    found = module.of(omf.parse(result.data))
    assert any(one.insn.code == Code.ADD_RM16_IMM8 and one.insn.immediate8to16 == 7
               for one in blocks.instructions(found))


def test_partial_write_keeps_the_untouched_initializer_bytes():
    """SPILL forgot h3=7 when updating adjacent o1, because their initializer was one dword store."""
    address = Addr(Space.SEGMENT, 10, 5)
    whole = mir.MemRef(address, 4)
    initializer = mir.Op(0, ir.Operation.MOVE, "mov", (), (), kind=mir.Kind.STORE,
                         args=(mir.Const(7, 4),), stores=(whole,))
    overwrite = mir.Op(1, ir.Operation.MOVE, "mov", (), (), kind=mir.Kind.STORE,
                       stores=(mir.MemRef(address.plus(2), 2),))
    before = consts._kills({}, initializer, {}, frozenset({5}), {})
    after = consts._kills(before, overwrite, {}, frozenset({5}), {})
    assert consts._cell(after, mir.MemRef(address, 2)) == consts.Known(7, 2)
    assert consts._cell(after, whole) is None
    assert consts._cell(after, mir.MemRef(address.plus(2), 2)) is None


def test_memory_fact_meet_is_independent_of_initializer_width():
    """Equivalent word and dword stores must agree at a control-flow join."""
    address = Addr(Space.SEGMENT, 10, 5)
    def store(where, width, number):
        return mir.Op(0, ir.Operation.MOVE, "mov", (), (), kind=mir.Kind.STORE,
                      args=(mir.Const(number, width),), stores=(mir.MemRef(where, width),))
    wide = consts._kills({}, store(address, 4, 0x12345678), {}, frozenset({5}), {})
    words = consts._kills({}, store(address, 2, 0x5678), {}, frozenset({5}), {})
    words = consts._kills(words, store(address.plus(2), 2, 0x1234), {}, frozenset({5}), {})
    assert wide == words
    unknown = mir.Op(1, ir.Operation.MOVE, "mov", (), (), kind=mir.Kind.STORE,
                     stores=(mir.MemRef(None, 2),))
    assert consts._kills(wide, unknown, {}, frozenset({5}), {}) == {}
