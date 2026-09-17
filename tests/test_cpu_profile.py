from pathlib import Path
from dataclasses import FrozenInstanceError

import pytest

from qbopt import flow
from qbopt.model import ir
from qbopt.model import mir
from qbopt.analysis import induction
from qbopt.backend import cpu
from qbopt.backend import lower
from qbopt.backend import allocate
from qbopt.cfront import compile as cfront
from qbopt.optimize import transform
from qbopt.optimize import strength
from qbopt.model.passes import OperationCosts

FIXTURES = Path(__file__).resolve().parents[1] / "fixtures" / "c"


def test_every_public_cpu_name_has_one_immutable_profile() -> None:
    assert cpu.names() == ("386", "486", "P5", "P6", "K5", "K6", "K7", "Core")
    assert tuple(cpu.profile(name).name for name in cpu.names()) == cpu.names()
    with pytest.raises(FrozenInstanceError):
        cpu.profile("P5").name = "386"
    for name in cpu.names():
        target = cpu.profile(name)
        assert target.operations.add == target.cost("alu_rr")
        assert target.operations.address == target.cost("lea")
        assert target.operations.memory_update == target.cost("alu_mr")
        assert target.operations.prefix == target.prefix_cost
        assert target.operations.move == target.cost("mov_rr")
        assert target.operations.call == target.cost("call_far")
        assert target.operations.return_ == target.cost("ret_far")
        assert target.operations.float_add == target.cost("x87_add")
        assert target.operations.float_multiply == target.cost("x87_mul")
        assert target.operations.float_divide == target.cost("x87_div")
        assert target.operations.float_load == target.cost("x87_load")
        assert target.operations.float_store == target.cost("x87_store")


def test_operation_costs_do_not_change_the_existing_profile_positional_shape() -> None:
    """Adding MIR costs must not reinterpret a caller's capacity arguments."""
    target = cpu.Profile("test", 1, True, 0, 0, 3, 1, frozenset({1}), (), ())

    assert target.register_capacity == 3
    assert target.call_register_capacity == 1
    assert target.address_scales == frozenset({1})
    assert target.operations == OperationCosts()


def test_medium_model_profiles_only_offer_unscaled_index_addressing() -> None:
    """The old profile offered 386's flat 32-bit scales to 16-bit medium code.

    Our ABI has only the 16-bit ``[base+index]`` form.  Advertising scaled
    forms let MIR formula selection price encodings lowering cannot legally
    use, exactly the wrong reference model for the QCport loop audit.
    """
    assert {target.address_scales for target in map(cpu.profile, cpu.names())} == {frozenset({1})}


def test_unknown_cpu_is_rejected_at_the_shared_boundary() -> None:
    with pytest.raises(ValueError, match="unknown CPU target"):
        cpu.profile("pentium")


def test_machine_pipeline_gives_allocator_the_complete_cpu_profile() -> None:
    phases = flow.machine({}, cpu="P5")
    allocator = next(one for one in phases if isinstance(one, allocate.RegAlloc))
    assert allocator.cpu is cpu.profile("P5")


def test_c_frontend_threads_selected_cpu_to_every_procedure(monkeypatch: pytest.MonkeyPatch) -> None:
    observed = []
    real = lower.lowered

    def recording(*args, **kwargs):
        observed.append(kwargs.get("cpu", args[6] if len(args) > 6 else None))
        return real(*args, **kwargs)

    monkeypatch.setattr(lower, "lowered", recording)
    cfront.compiled((FIXTURES / "halve.cgs").read_text(), "halve", optimise=True, cpu="P5")
    assert observed and set(observed) == {cpu.profile("P5")}


def test_c_frontend_default_remains_386(monkeypatch: pytest.MonkeyPatch) -> None:
    observed = []
    real = lower.lowered

    def recording(*args, **kwargs):
        observed.append(kwargs.get("cpu", args[6] if len(args) > 6 else None))
        return real(*args, **kwargs)

    monkeypatch.setattr(lower, "lowered", recording)
    cfront.compiled((FIXTURES / "halve.cgs").read_text(), "halve", optimise=True)
    assert observed and set(observed) == {cpu.profile("386")}


def test_c_frontend_threads_machine_neutral_cpu_costs_to_mir(monkeypatch: pytest.MonkeyPatch) -> None:
    """CPU selection reached lowering but loop profitability still saw no costs."""
    observed = []
    real = transform.applied

    def recording(*args, **kwargs):
        observed.append((kwargs.get("costs"), kwargs.get("index_scales")))
        return real(*args, **kwargs)

    monkeypatch.setattr(transform, "applied", recording)
    cfront.compiled((FIXTURES / "halve.cgs").read_text(), "halve", optimise=True, cpu="P5")

    assert observed
    assert {costs for costs, _scales in observed} == {cpu.profile("P5").operations}
    assert {scales for _costs, scales in observed} == {cpu.profile("P5").address_scales}


def test_formula_selection_prices_complete_sibling_groups() -> None:
    """A largest-group heuristic chose three costly address recomputations.

    When only one recurrence slot must be recovered, collapsing the smaller
    sibling group is cheaper if every retained child address is expensive.
    The selected set must still be complete: parent or all children, never a
    mixture from one group.
    """

    def group(start: int, count: int) -> list[induction.Derived]:
        counter = mir.Value(start, 0)
        product = mir.Value(start + 1, 1)
        affine = induction.Affine(counter.id, mir.Const(0, 2), mir.Const(1, 2), 1)
        multiply = mir.Op(
            1,
            ir.Operation.MULTIPLY,
            "imul",
            (product,),
            (counter,),
            kind=mir.Kind.MUL,
            args=(mir.Held(counter, 2), mir.Const(8, 2)),
            results=(mir.Held(product, 2),),
        )
        out = [induction.Derived(multiply, affine, mir.Const(8, 2))]
        for index in range(count):
            answer = mir.Value(start + 2 + index, 2 + index)
            add = mir.Op(
                2 + index,
                ir.Operation.BINARY,
                "add",
                (answer,),
                (product,),
                kind=mir.Kind.ADD,
                args=(mir.Held(product, 2), mir.Const(index * 16, 2)),
                results=(mir.Held(answer, 2),),
            )
            out.append(induction.Derived(add, affine, mir.Const(8, 2), ((mir.Const(index * 16, 2), 1),)))
        return out

    small = group(10, 2)
    large = group(100, 3)
    candidates = [*small, *large]
    costly_addresses = OperationCosts(add=1, address=100, load=1, memory_update=1)

    selected = strength._formula_set(candidates, room=4, costs=costly_addresses)

    assert selected == [small[0], *large[1:]]
