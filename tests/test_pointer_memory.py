"""Whole-pointer memory operands survive SSA and materialize only below MIR."""

from dataclasses import replace

import pytest
from iced_x86 import Decoder, Register

from qbopt.analysis import ssa
from qbopt.backend import allocate, lower, pointers, select
from qbopt.model import ir, mir, lir


def access(store=False):
    pointer, value = mir.Value(1, 0), mir.Value(2, 0)
    ref = mir.MemRef(None, 2, base=pointer, pointer=True)
    cell, held = mir.Cell(ref), mir.Held(value, 2)
    return mir.Op(0, ir.Operation.MOVE, "mov", () if store else (value,),
                  (pointer, value) if store else (pointer,),
                  kind=mir.Kind.STORE if store else mir.Kind.LOAD,
                  args=(held,) if store else (cell,), results=(cell,) if store else (held,),
                  loads=() if store else (ref,), stores=(ref,) if store else ())


def test_whole_pointer_is_an_ssa_dependency_not_a_register_pair():
    op = access()
    replacement = mir.Value(9, 4)
    changed = ssa.substituted(op, {1: replacement})
    assert changed.loads[0].pointer
    assert changed.loads[0].base == replacement
    assert changed.args[0].ref == changed.loads[0]
    assert changed.uses == (replacement,)
    assert changed.loads[0].segment is None


def test_pointer_identity_is_not_a_disjointness_proof():
    ref = access().loads[0]
    other = replace(ref, base=mir.Value(3, 0))
    assert mir.same_bytes(ref, ref)
    assert not mir.same_bytes(ref, other)
    assert not mir.same_bytes(ref, replace(ref, pointer=False))
    assert mir.overlapping(ref, other, frozenset())
    assert mir.overlapping(ref, mir.MemRef(None, 4), frozenset())


@pytest.mark.parametrize("store", [False, True])
def test_pointer_memory_encodes_and_restores_the_segment_and_stack(store):
    """Loading through a whole pointer must not wrap to DS or overwrite another access's ES."""
    op = access(store)
    body = mir.MirBody(0, (mir.MirBlock(0, (), (op,), ()),))
    parts = lower.Lowering(body, {1, 2}, {}, (), pointer_model=pointers.Model(12)).expand(op)
    assert len(parts) == 6
    assert parts[1].uses == (1,)
    assert parts[4].what.name == "mov"

    def placed(arg):
        match arg:
            case ir.Held(value=value, width=width):
                return ir.Reg(Register.EAX if value == 1 else Register.CX if value == 2 else Register.BX, width)
            case ir.Mem():
                return replace(arg, base=None, through=Register.BX, addr=replace(arg.addr, base=Register.BX))
        return arg

    code = b""
    for part in parts:
        what = replace(part.what, dests=tuple(map(placed, part.what.dests)), sources=tuple(map(placed, part.what.sources)))
        emitted = select.emit(what)
        assert emitted is not None, what
        code += emitted.code
    assert code.hex() == ("0666505b0726890f07" if store else "0666505b07268b0f07")
    decoded = list(Decoder(16, code))
    assert sum(instruction.stack_pointer_increment for instruction in decoded) == 0
    assert all(instruction.rflags_modified == 0 for instruction in decoded)


def test_pointer_memory_requires_an_established_abi():
    op = access()
    body = mir.MirBody(0, (mir.MirBlock(0, (), (op,), ()),))
    with pytest.raises(lower.Unlowered, match="pointer ABI"):
        lower.Lowering(body, {1, 2}, {}, ()).expand(op)


def test_generated_byte_value_cannot_be_allocated_to_edi():
    """HUGE2 refused emission when the runtime shift byte was allocated to nonexistent DIL."""
    what = ir.Semantics(ir.Operation.MOVE, "mov", (ir.Held(1, 1),), (ir.Imm(12, 1),))
    insn = lir.Insn(0, (0, 0), what, (1,), ())
    body = lir.LirBody("byte", 0, (lir.LirBlock(0, (insn,)),), {}, {})
    assert allocate.classes(body)[1] == frozenset({Register.AX, Register.BX, Register.CX, Register.DX})
