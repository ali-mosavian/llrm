"""Propagate a store's scalar value without deleting its memory observation."""

from pathlib import Path

import pytest
import corpus

from qbopt import wholeseg
from qbopt.model import ir, mir
from qbopt.optimize import transform


@pytest.mark.parametrize("width", [2, 4])
@pytest.mark.parametrize("live_flags", [False, True])
def test_constant_memory_update_wraps_and_keeps_live_conditions(width, live_flags):
    from qbopt.analysis import consts
    from qbopt.objectfile.module import Addr, Space
    ref = mir.MemRef(Addr(Space.SEGMENT, 6, 5), width)
    flags = mir.Value(1, 0, flags=True)
    op = mir.Op(0, ir.Operation.UNARY, "inc", (flags,), (), kind=mir.Kind.INCREMENT,
                args=(mir.Cell(ref),), results=(mir.Cell(ref),), loads=(ref,), stores=(ref,))
    memory = {(ref.addr, width): consts.Known((1 << (width * 8)) - 1, width)}
    assert consts._cell(consts._kills(memory, op, {}, frozenset({5}), {}), ref) == consts.Known(0, width)
    result = transform._constant_update(op, {}, memory, {flags} if live_flags else set())
    if live_flags:
        assert result is op
    else:
        assert result.kind is mir.Kind.STORE and result.args == (mir.Const(0, width),)
        assert result.stores == (ref,) and result.results == (mir.Cell(ref),)
        assert not result.loads and not result.defines


def test_constant_memory_update_does_not_freeze_a_loop_counter():
    from qbopt.analysis import consts
    from qbopt.objectfile.module import Addr, Space
    ref = mir.MemRef(Addr(Space.SEGMENT, 6, 5), 2)
    initialize = mir.Op(0, ir.Operation.MOVE, "mov", (), (), kind=mir.Kind.STORE,
                        args=(mir.Const(0, 2),), results=(mir.Cell(ref),), stores=(ref,))
    increment = mir.Op(10, ir.Operation.UNARY, "inc", (), (), kind=mir.Kind.INCREMENT,
                       args=(mir.Cell(ref),), results=(mir.Cell(ref),), loads=(ref,), stores=(ref,))
    body = mir.MirBody(0, (mir.MirBlock(0, (), (initialize,), (10,)),
                          mir.MirBlock(10, (), (increment,), (10, 20)), mir.MirBlock(20, (), (), ())))
    memory = consts.cells(body, frozenset({5}), {})
    assert consts._cell(memory[(10, 0)], ref) is None
    assert transform.folded(body, frozenset({5}), {}).blocks[1].ops[0] == increment


@pytest.mark.parametrize("tag", ["p-g2", "q-O", "v-g3"])
def test_bools_known_accumulator_does_not_need_memory_arithmetic(tag):
    """BOOLS recomputed t=-1+1+2 through memory instead of storing its known answer."""
    result = wholeseg.emitted(Path(f"fixtures/omf/bools-{tag}.obj").read_bytes())
    assert result.outcome is wholeseg.Emission.LIR, result.reason
    instructions = [str(one.insn) for block in corpus.partitioned(result.data) for one in block.insns]
    assert not any(one.startswith(("add ", "inc ")) for one in instructions)


def test_fpemu_double_constant_is_not_mistaken_for_c_int64() -> None:
    """fpemu was refused as "64-bit integer lowering is not implemented".

    Operand width alone cannot identify a C integer: BASIC's folded DOUBLE
    initializer is also eight bytes, and generic lowering already emits that
    bit pattern as two dword stores.  C's frontend legalizes its actual int64
    values before it reaches this boundary.
    """
    result = wholeseg.emitted(Path("fixtures/omf/fpemu-p-g2.obj").read_bytes())
    assert result.outcome is wholeseg.Emission.LIR, result.reason


@pytest.mark.parametrize("width", [None, 2, 4])
@pytest.mark.parametrize("address_uses_value", [False, True])
def test_constant_store_requires_full_width_and_retains_address_uses(width, address_uses_value):
    """Replacing a stored value must not lose a same-valued address or invent its high half."""
    value = mir.Value(1, 0)
    ref = mir.MemRef(None, 4, base=value if address_uses_value else None)
    op = mir.Op(0, ir.Operation.MOVE, "mov", (), (value,), stores=(ref,),
                args=(mir.Held(value, 4),), results=(mir.Cell(ref),), kind=mir.Kind.STORE)
    facts = {} if width is None else {value: mir.Const(-1, width)}
    result = transform._constant_operands(op, facts)
    if width != 4:
        assert result is op
    else:
        assert result.args == (mir.Const(0xffffffff, 4),)
        assert result.stores == op.stores and result.results == op.results
        assert result.uses == ((value,) if address_uses_value else ())
