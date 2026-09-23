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
def test_spill_keeps_the_invariant_cell_out_of_repeated_work(tag):
    """SPILL reloaded h3=7 a hundred times before its loops were folded away.

    The exact final constants are covered by the algebraic regression.  This
    test owns the memory fact: no emitted computation may reload h3, whether
    the consumer is hoisted, folded, or eliminated entirely.
    """
    from qbopt.objectfile import module, omf
    from qbopt import wholeseg
    result = wholeseg.emitted(Path(f"fixtures/omf/spill-{tag}.obj".lower()).read_bytes())
    assert result.outcome is wholeseg.Emission.LIR, result.reason
    found = module.of(omf.parse(result.data))
    body = mir.bodies(found, corpus.partitioned(result.data))[0][1]
    h3 = Addr(Space.SEGMENT, 10, 5)
    assert not any(ref.addr == h3 for block in body.blocks for op in block.ops for ref in op.loads)


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


def test_cells_builds_one_interval_epoch_for_all_memory_kills(monkeypatch: pytest.MonkeyPatch) -> None:
    """QB nbody rebuilt the complete known-value interval map 2.85m times.

    Known values cannot change while one cells() dataflow invocation runs.
    Every operation in that invocation must therefore share one interval
    epoch, including its fixed-point and final fact-recording walks.
    """
    address = Addr(Space.SEGMENT, 0, 5)
    value = mir.Value(1, 0, variable=1)
    operations = tuple(
        mir.Op(
            index,
            ir.Operation.MOVE,
            "mov",
            (),
            (),
            kind=mir.Kind.STORE,
            stores=(mir.MemRef(address.plus(index * 2), 2),),
        )
        for index in range(16)
    )
    body = mir.MirBody(0, (mir.MirBlock(0, (), operations, ()),))
    intervals = consts._intervals
    calls = 0

    def counted(known):
        nonlocal calls
        calls += 1
        return intervals(known)

    monkeypatch.setattr(consts, "_intervals", counted)
    consts.cells(body, frozenset({5}), {}, {value: consts.Known(3, 2)})

    assert calls == 1


def test_cells_reuses_overlap_answers_within_one_fact_epoch(monkeypatch: pytest.MonkeyPatch) -> None:
    """QB nbody repeated identical alias questions on every dataflow walk."""
    address = Addr(Space.SEGMENT, 0, 5)
    initial = {(address.plus(index * 2), 2): consts.Known(index, 2) for index in range(16)}
    unknown = mir.Op(
        0,
        ir.Operation.MOVE,
        "mov",
        (),
        (),
        kind=mir.Kind.STORE,
        stores=(mir.MemRef(None, 2),),
    )
    body = mir.MirBody(0, (mir.MirBlock(0, (), (unknown,), ()),))
    overlapping = consts.mir.overlapping
    calls = 0

    def counted(*args, **kwargs):
        nonlocal calls
        calls += 1
        return overlapping(*args, **kwargs)

    monkeypatch.setattr(consts.mir, "overlapping", counted)
    consts.cells(body, frozenset({5}), {}, initial=initial)

    assert calls == len(initial)


def test_a_call_reaching_nonlocal_keeps_an_uncaptured_static_constant() -> None:
    """A constant cell's key had no object, so it met every call's reach and died at each one.

    The key takes the object the body's own reference names, uncaptured
    static included, so a call reaching only NONLOCAL leaves it standing.
    """
    from qbopt.model import memory

    address = Addr(Space.SEGMENT, 6, 5)
    static = memory.Object(memory.Kind.GLOBAL, (Space.SEGMENT, 5), captured=False)
    ref = mir.MemRef(address, 2, provenance=memory.Provenance.one(static, 6, 8))
    reach = mir.MemRef(None, 0, provenance=memory.Provenance.one(memory.Object(memory.Kind.NONLOCAL)))
    store = mir.Op(0, ir.Operation.MOVE, "mov", (), (), kind=mir.Kind.STORE, args=(mir.Const(7, 2),), stores=(ref,))
    call = mir.Op(1, ir.Operation.CALL, "", (), (), kind=mir.Kind.CALL, stores=(reach,), memory_complete=True)
    read = mir.Op(2, ir.Operation.MOVE, "mov", (), (), kind=mir.Kind.LOAD, loads=(ref,))
    body = mir.MirBody(0, (mir.MirBlock(0, (), (store, call, read), ()),))

    before = consts.cells(body, frozenset({5}), {})

    assert consts._cell(before[(0, 2)], ref) == consts.Known(7, 2)
