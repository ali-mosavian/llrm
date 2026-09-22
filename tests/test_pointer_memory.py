"""Whole-pointer memory operands survive SSA and materialize only below MIR."""

from dataclasses import replace

import pytest
from iced_x86 import Decoder
from iced_x86 import Register

from qbopt.model import ir
from qbopt.model import lir
from qbopt.model import mir
from qbopt.analysis import ssa
from qbopt.backend import lower
from qbopt.backend import select
from qbopt.backend import allocate
from qbopt.backend import pointers
from qbopt.optimize import transform
from qbopt.model.passes import Options
from qbopt.objectfile.module import Addr
from qbopt.objectfile.module import Space


def access(store=False):
    pointer, value = mir.Value(1, 0), mir.Value(2, 0)
    ref = mir.MemRef(None, 2, base=pointer, pointer=True)
    cell, held = mir.Cell(ref), mir.Held(value, 2)
    return mir.Op(
        0,
        ir.Operation.MOVE,
        "mov",
        () if store else (value,),
        (pointer, value) if store else (pointer,),
        kind=mir.Kind.STORE if store else mir.Kind.LOAD,
        args=(held,) if store else (cell,),
        results=(cell,) if store else (held,),
        loads=() if store else (ref,),
        stores=(ref,) if store else (),
    )


def test_whole_pointer_is_an_ssa_dependency_not_a_register_pair():
    op = access()
    replacement = mir.Value(9, 4)
    changed = ssa.substituted(op, {1: replacement})
    assert changed.loads[0].pointer
    assert changed.loads[0].base == replacement
    assert changed.args[0].ref == changed.loads[0]
    assert changed.uses == (replacement,)
    assert changed.loads[0].segment is None


def test_pointer_substitution_updates_known_memory_cells():
    """Composed pointer recurrence left CALL memory_values on its old base."""
    op = access()
    op = replace(op, memory_values=((op.loads[0], mir.Const(7, 2)),))
    replacement = mir.Value(9, 4)
    changed = ssa.substituted(op, {1: replacement})
    assert changed.memory_values[0][0].base == replacement


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
        what = replace(
            part.what, dests=tuple(map(placed, part.what.dests)), sources=tuple(map(placed, part.what.sources))
        )
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


def test_optimizer_splits_a_packed_far_pointer_before_its_access() -> None:
    """QB qrender rebuilt each dereference through six stack operations.

    A packed selector:offset value is program meaning, but a far-memory
    operand consumes its two words independently.  Expose those word values
    in MIR so ordinary value numbering can reuse them and allocation can keep
    the selector in any available segment register.
    """
    offset, selector, pointer = (mir.Value(index, 0, variable=index) for index in range(1, 4))
    offset_ref = mir.MemRef(Addr(Space.FRAME, 4), 2, space=Space.FRAME)
    selector_ref = mir.MemRef(Addr(Space.FRAME, 6), 2, space=Space.FRAME)
    packed_ref = mir.MemRef(None, 2, base=pointer, pointer=True)
    ops = (
        mir.Op(
            0,
            ir.Operation.MOVE,
            "mov",
            (offset,),
            (),
            loads=(offset_ref,),
            kind=mir.Kind.LOAD,
            args=(mir.Cell(offset_ref),),
            results=(mir.Held(offset, 2),),
        ),
        mir.Op(
            1,
            ir.Operation.MOVE,
            "mov",
            (selector,),
            (),
            loads=(selector_ref,),
            kind=mir.Kind.LOAD,
            args=(mir.Cell(selector_ref),),
            results=(mir.Held(selector, 2),),
        ),
        mir.Op(
            2,
            mir.Synth.CONCAT_LOW,
            "concat",
            (pointer,),
            (selector, offset),
            kind=mir.Kind.CONCAT,
            args=(mir.Held(selector, 2), mir.Held(offset, 2)),
            results=(mir.Held(pointer, 4),),
        ),
        mir.Op(
            3,
            ir.Operation.MOVE,
            "mov",
            (),
            (pointer,),
            stores=(packed_ref,),
            kind=mir.Kind.STORE,
            args=(mir.Const(0, 2),),
            results=(mir.Cell(packed_ref),),
        ),
    )
    body = mir.MirBody(0, (mir.MirBlock(0, (), ops, ()),))

    optimized = transform.applied(body, frozenset(), {}, options=Options(unroll=False, peel=False))
    operations = tuple(op for block in optimized.blocks for op in block.ops)
    stores = [ref for op in operations for ref in op.stores]

    assert stores == [
        mir.MemRef(
            Addr(Space.FAR, 0),
            2,
            base=offset,
            segment=selector,
            space=Space.FAR,
            base_width=2,
        )
    ]
    assert not any(ref.pointer for op in operations for ref in (*op.loads, *op.stores))
    assert not any(op.kind in (mir.Kind.CONCAT, mir.Kind.EXTRACT) for op in operations)

    lowered = lower.lowered("packed", optimized, {}, (), {})
    instructions = [one for block in lowered.blocks for one in block.insns if one.what is not None]
    assert not any(one.what.name in ("push", "pop") for one in instructions)
    written = next(one.what.dests[0] for one in instructions if one.op is not None and one.op.kind is mir.Kind.STORE)
    assert isinstance(written, ir.Mem)
    assert written.base == ir.Held(offset.id, 2)
    assert written.selector == ir.Held(selector.id, 2)


def test_generated_byte_value_cannot_be_allocated_to_edi():
    """HUGE2 refused emission when the runtime shift byte was allocated to nonexistent DIL."""
    what = ir.Semantics(ir.Operation.MOVE, "mov", (ir.Held(1, 1),), (ir.Imm(12, 1),))
    insn = lir.Insn(0, (0, 0), what, (1,), ())
    body = lir.LirBody("byte", 0, (lir.LirBlock(0, (insn,)),), {}, {})
    assert allocate.classes(body)[1] == frozenset({Register.AX, Register.BX, Register.CX, Register.DX})
