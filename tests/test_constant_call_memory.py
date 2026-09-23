"""Printing must not erase constants in cells its raised effects cannot reach."""

from pathlib import Path

import pytest
import corpus

from qbopt.analysis import consts
from qbopt.model import mir
from qbopt.model import ir
from qbopt.objectfile.module import Addr, Space
from qbopt import wholeseg


@pytest.mark.parametrize("offset", [0, 1, 2, 3])
def test_call_invalidates_every_byte_of_escaped_long(offset):
    """A write through an escaped long retained its upper three constant bytes."""
    address = Addr(Space.SEGMENT, 6, index=5)
    cells = consts._fragments(mir.MemRef(address, 4), consts.Known(0x12345678, 4))
    effect = mir.MemRef(None, 0, beyond=(5, frozenset({(5, 6)})))
    call = mir.Op(10, ir.Operation.MOVE, "", (), (), kind=mir.Kind.CALL, stores=(effect,))
    after = consts._kills(cells, call, {}, frozenset({5}), {10: "B$PSSD"})
    assert consts._cell(after, mir.MemRef(address.plus(offset), 1)) is None


@pytest.mark.parametrize("escapes", [frozenset(), frozenset({(9, 0), (9, 6)})])
def test_call_preserves_constants_when_no_program_address_escapes(escapes):
    """PRINT discarded an input constant even with an empty proven escape set."""
    address = Addr(Space.SEGMENT, 6, index=5)
    cells = consts._fragments(mir.MemRef(address, 4), consts.Known(0x12345678, 4))
    effect = mir.MemRef(None, 0, beyond=(5, escapes))
    call = mir.Op(10, ir.Operation.MOVE, "", (), (), kind=mir.Kind.CALL, stores=(effect,))
    after = consts._kills(cells, call, {}, frozenset({5}), {10: "B$PSSD"})
    assert consts._cell(after, mir.MemRef(address, 4)) == consts.Known(0x12345678, 4)


def test_a_call_with_no_write_effects_invalidates_constants():
    """Missing write effects are not a proof the call writes nothing."""
    address = Addr(Space.SEGMENT, 6, index=5)
    cells = consts._fragments(mir.MemRef(address, 4), consts.Known(7, 4))
    call = mir.Op(10, ir.Operation.MOVE, "", (), (), kind=mir.Kind.CALL, stores=())
    assert consts._kills(cells, call, {}, frozenset({5}), {10: "B$PSSD"}) == {}


@pytest.mark.parametrize("name", ["UNKNOWN", "TWICE"])
def test_only_a_contracted_call_is_bounded_by_its_escapes(name):
    """A user SUB writes SHARED statics no pointer escaped to, so its call gets no write bound."""
    from qbopt.abi import runtime
    from qbopt.frontend import raising_call_memory

    contract = runtime.contract(name)
    assert raising_call_memory.reachable(contract, runtime.Memory.NONE, (5, frozenset())) is None


@pytest.mark.parametrize("tag", ["p-g2", "q-O", "v-g3"])
def test_negnot_does_not_reload_constant_inputs_after_print(tag):
    """NEGNOT recomputed constant expressions after PRINT despite no data-segment escapes."""
    result = wholeseg.emitted(Path(f"fixtures/omf/negnot-{tag}.obj".lower()).read_bytes())
    assert result.outcome is wholeseg.Emission.LIR, result.reason
    from qbopt.objectfile import module, omf
    found = module.of(omf.parse(result.data))
    bodies = mir.bodies(found, corpus.partitioned(result.data))
    inputs = {Addr(Space.SEGMENT, offset, 5) for offset in range(6, 14)}
    assert not any(ref.addr in inputs for _, body in bodies for block in body.blocks
                   for op in block.ops for ref in op.loads)
