"""Repeated provenance is not the order in which emitted instructions execute."""

from types import SimpleNamespace
from dataclasses import replace

import pytest
from iced_x86 import Register

from qbopt.backend import layout
from qbopt.model import ir, mir
from qbopt.backend import asm


@pytest.mark.parametrize("successors,expected", [
    (((20,), (), (10,)), [0, 20, 10]),
    (((20,), (), (0,)), [0, 10, 20]),
    (((10, 20), (), (10,)), [0, 10, 20]),
    (((10,), (), ()), [0, 10, 20]),
    (((30,), (), (10,)), [0, 10, 20]),
])
def test_linear_placement_requires_one_complete_acyclic_chain(successors, expected):
    blocks = tuple(mir.MirBlock(at, (), (mir.Op(at, ir.Operation.NOTHING, "", (), ()),), succ)
                   for at, succ in zip((0, 10, 20), successors))
    body = mir.MirBody(0, blocks)
    assert [op.at for op in layout._ordered(body, linear=True)] == expected
    assert [op.at for op in layout._ordered(body)] == [0, 10, 20]


def test_removed_floating_loop_is_emitted_in_execution_order():
    """FPCSE's removed loop still took three unconditional jumps through its old block layout."""
    from pathlib import Path
    from iced_x86 import Mnemonic
    import corpus
    from qbopt import wholeseg
    for tag in ("p-g2", "q-O", "v-g3"):
        result = wholeseg.emitted(Path(f"fixtures/omf/fpcse-{tag}.obj").read_bytes())
        assert result.outcome is wholeseg.Emission.LIR, result.reason
        assert not any(one.insn.mnemonic == Mnemonic.JMP
                       for block in corpus.partitioned(result.data) for one in block.insns), tag


def test_ordered_body_selects_ordered_object_layout(monkeypatch):
    """FPDEEP clones must not require a caller to remember the ordered-layout switch."""
    from qbopt.model import lir
    from qbopt.objectfile import objwrite
    requested = []
    def rebuild(*args, **kwargs):
        requested.append(kwargs["ordered"])
        return "captured"
    monkeypatch.setattr(layout, "rebuild", rebuild)
    body = lir.LirBody("cloned", 0, (), {}, {}, ordered=True)
    assert objwrite.written(None, [body], [], {}) == "captured"
    assert requested == [True]
    legacy = replace(body, ordered=False)
    assert "mixed ordered" in objwrite.written(None, [body, legacy], [], {})
    assert requested == [True]


def test_explicit_emission_order_does_not_group_clones_by_source_address():
    """FPDEEP's three iterations were interleaved by source address and timed out."""
    def move(at, number, covers):
        what = ir.Semantics(ir.Operation.MOVE, "mov", (ir.Reg(Register.AX, 2),), (ir.Imm(number, 2),))
        return mir.Op(at, ir.Operation.MOVE, "mov", (), (), made=what, covers=covers)

    first = move(0, 1, (0, 3))
    second = move(3, 2, (3, 6))
    clone = move(0, 3, (0, 0))
    body = mir.MirBody(0, (mir.MirBlock(0, (), (first, second, clone), ()),))
    found = SimpleNamespace(code=bytes.fromhex("b80100b80200"), end=6,
                            coverage={}, refs={}, calls={}, absorbed={}, fixup_at={}, float_protocols={})
    emitted = layout.rebuild(found, [("order", body)], ordered=True)
    assert not isinstance(emitted, str), emitted
    assert emitted.code == bytes.fromhex("b80100b80200b80300")
    assert emitted.moved[0] == 0
    assert emitted.moved[3] == 3


def test_cloned_far_call_keeps_its_relocation_without_claiming_input_bytes():
    """FPDEEP's second and third printed iterations emitted unrelocated call 0:0."""
    what = ir.Semantics(ir.Operation.CALL, "call")
    original = mir.Op(0, ir.Operation.CALL, "call", (), (), made=what,
                      covers=(0, 5), id=7, symbol=True)
    clone = replace(original, covers=(0, 0))
    found = SimpleNamespace(code=bytes.fromhex("9a00000000"), end=5,
                            coverage={7: ((0, 5),)}, refs={}, calls={0: "B$PSSD"}, absorbed={}, fixup_at={}, float_protocols={})
    assert asm._ranges_of(clone, found) == ((0, 0),)
    assert asm._length_of(clone, found) == 0
    emitted = asm.assemble([original, clone], 0, found, fields=frozenset({1}))
    assert not isinstance(emitted, str), emitted
    assert emitted.relocations == ((1, 1), (6, 1))
    assert asm._field_in(found, replace(clone, id=None), frozenset({1})) is None
    assert asm._field_in(found, replace(clone, symbol=False), frozenset({1})) is None


def test_block_entry_is_not_the_first_clone_of_its_source_address():
    """FPDEEP's entry jump landed in a cloned header before its first iteration."""
    def move(at, number, covers):
        return mir.Op(at, ir.Operation.MOVE, "mov", (), (), covers=covers,
                      made=ir.Semantics(ir.Operation.MOVE, "mov", (ir.Reg(Register.AX, 2),), (ir.Imm(number, 2),)))
    initial = move(0, 1, (0, 3))
    cloned_header = move(3, 99, (3, 3))
    header = move(3, 2, (3, 6))
    body = mir.MirBody(0, (mir.MirBlock(0, (), (initial, cloned_header), (3,)),
                          mir.MirBlock(3, (), (header,), ())))
    found = SimpleNamespace(code=bytes.fromhex("b80100b80200"), end=6,
                            coverage={}, refs={}, calls={}, absorbed={}, fixup_at={}, float_protocols={})
    emitted = layout.rebuild(found, [("labels", body)], ordered=True)
    assert not isinstance(emitted, str), emitted
    assert emitted.code == bytes.fromhex("b80100b86300b80200")
    assert emitted.moved[3] == 6
