from collections import Counter
from types import SimpleNamespace

from qbopt.model import ir
from qbopt.model import lir
from qbopt.backend import rmw


def _insn(
    at: int,
    what: ir.Semantics,
    defines: tuple[int, ...] = (),
    uses: tuple[int, ...] = (),
    *,
    volatile: bool = False,
) -> lir.Insn:
    return lir.Insn(
        at=at,
        covers=(at, at),
        what=what,
        defines=defines,
        uses=uses,
        op=SimpleNamespace(volatile=volatile),
    )


def _users(insns: tuple[lir.Insn, ...]) -> Counter[int]:
    return Counter(value for one in insns for value in one.uses)


def _chain(name: str = "add", *, old_on_left: bool = True, volatile: bool = False) -> tuple[lir.Insn, ...]:
    base = ir.Held(1, 2)
    cell = ir.Mem(None, 4, base=base)
    old = ir.Held(2, 4)
    source = ir.Held(3, 4)
    result = ir.Held(4, 4)
    left, right = (old, source) if old_on_left else (source, old)
    return (
        _insn(0, ir.Semantics(ir.Operation.MOVE, "mov", (old,), (cell,)), (old.value,), (base.value,)),
        _insn(1, ir.Semantics(ir.Operation.MOVE, "mov", (source,), (ir.Mem(None, 4),)), (source.value,)),
        _insn(
            2,
            ir.Semantics(ir.Operation.BINARY, name, (result,), (left, right)),
            (result.value,),
            (left.value, right.value),
        ),
        _insn(3, ir.Semantics(ir.Operation.NOTHING, "")),
        _insn(
            4,
            ir.Semantics(ir.Operation.MOVE, "mov", (cell,), (result,)),
            uses=(result.value, base.value),
            volatile=volatile,
        ),
    )


def test_private_integer_update_selects_a_memory_destination() -> None:
    """nbody used a temporary for the old field and stored its sum back."""
    insns = _chain()

    selected = rmw.selected(insns, _users(insns))

    assert selected[-1].what == ir.Semantics(
        ir.Operation.BINARY,
        "add",
        (ir.Mem(None, 4, base=ir.Held(1, 2)),),
        (ir.Mem(None, 4, base=ir.Held(1, 2)), ir.Held(3, 4)),
    )
    assert selected[-1].defines == ()
    assert selected[-1].uses == (1, 3)
    assert selected[0].what is not None
    assert selected[2].what is not None
    assert selected[0].what.op is ir.Operation.NOTHING
    assert selected[2].what.op is ir.Operation.NOTHING


def test_noncommutative_update_requires_the_loaded_cell_on_the_left() -> None:
    insns = _chain("sub", old_on_left=False)

    assert rmw.selected(insns, _users(insns)) == insns


def test_volatile_update_retains_its_explicit_load_and_store() -> None:
    insns = _chain(volatile=True)

    assert rmw.selected(insns, _users(insns)) == insns
