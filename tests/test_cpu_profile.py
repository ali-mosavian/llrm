from pathlib import Path
from dataclasses import FrozenInstanceError

import pytest
from iced_x86 import Register

from qbopt import flow
from qbopt.model import ir
from qbopt.model import lir
from qbopt.model import mir
from qbopt.backend import cpu
from qbopt.backend import lower
from qbopt.cycles import cycles
from qbopt.optimize import unroll
from qbopt.backend import allocate
from qbopt.backend import schedule
from qbopt.optimize import strength
from qbopt.analysis import induction
from qbopt.backend import floatalloc
from qbopt.optimize import transform
from qbopt.cfront import compile as cfront
from qbopt.model.passes import AddressForm
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
        assert target.max_unroll_iterations == 16
        assert target.max_unrolled_operations == 200
    assert cpu.profile("P5").pentium_pairing
    assert not any(cpu.profile(name).pentium_pairing for name in cpu.names() if name != "P5")


def test_operation_costs_do_not_change_the_existing_profile_positional_shape() -> None:
    """Adding MIR costs must not reinterpret a caller's capacity arguments."""
    target = cpu.Profile("test", 1, True, 0, 0, 3, 1, frozenset({1}), (), ())

    assert target.register_capacity == 3
    assert target.call_register_capacity == 1
    assert target.address_scales == frozenset({1})
    assert target.operations == OperationCosts()
    assert not target.pentium_pairing


def test_medium_model_profiles_distinguish_native_and_67h_addressing() -> None:
    """Scaled addressing is legal through 67h, but is not a free native form.

    The former profile called scales 2/4/8 illegal to stop formula selection
    treating flat-i386 SIB addressing as free.  Preserve that preference while
    representing the real 386 fallback and all of its extra costs explicitly.
    """
    for target in map(cpu.profile, cpu.names()):
        native, secondary = target.address_forms
        assert target.address_scales == native.scales == frozenset({1})
        assert native.index_width == 2 and not native.secondary
        assert secondary.index_width == 4 and secondary.scales == frozenset({1, 2, 4, 8})
        assert secondary.secondary and secondary.extra_bytes == 1
        assert secondary.use_cost == target.prefix_cost + target.address_prefix_stall
        assert secondary.extension_cost == target.operations.extend

    assert cpu.profile("P6").address_prefix_stall == 6
    assert cpu.profile("Core").address_prefix_stall == 3
    assert not any(cpu.profile(name).address_prefix_stall for name in ("386", "486", "P5", "K5", "K6", "K7"))

    legacy = AddressForm(4, frozenset({1, 2, 4, 8}), fallback=True)
    assert legacy.secondary and legacy.fallback


def test_memory_pop_is_not_priced_as_a_register_pop() -> None:
    """Mandel's frame copy hid POP-memory cost behind the POP-register row."""
    assert cycles.classify("pop", "dword [bp-4]", "668f46fc") == "pop_m"
    assert cpu.profile("386").cost("pop_m") == 5
    assert all(cpu.profile(name).prices("pop_m") for name in cpu.names())


def test_memory_compare_is_priced_as_a_read_not_a_read_modify_write() -> None:
    """Folding lru_use's load/test raised its 386 score despite less work.

    CMP and TEST never write their memory operand.  They therefore use the
    read-only ALU-memory form, matching the allocator's fold discount, rather
    than the cost of an ADD/SUB that must also write the cell back.
    """
    assert cycles.classify("cmp", "word [bp-4],0", "837efc00") == "alu_rm"
    assert cycles.classify("test", "word [bp-4],1", "f746fc0100") == "alu_rm"
    assert cycles.classify("add", "word [bp-4],1", "8346fc01") == "alu_mr"


@pytest.mark.parametrize("mnemonic", ["shld", "shrd"])
def test_scalar_double_shifts_use_the_integer_shift_price(mnemonic: str) -> None:
    """DX:AX extraction became artificially cheap as an unknown/generic ALU.

    GCC classifies scalar SHLD/SHRD with its ordinary integer-shift type.  The
    report and scheduler must use that same explicit profile row rather than
    hiding the newly selected instruction behind an unknown/default cost.
    """
    assert cycles.classify(mnemonic, "edx,eax,16", "") == "shift_ri"
    what = ir.Semantics(
        ir.Operation.FUNNEL,
        mnemonic,
        (ir.Reg(Register.EDX, 4),),
        (ir.Reg(Register.EDX, 4), ir.Reg(Register.EAX, 4), ir.Imm(16, 1)),
    )
    one = lir.Insn(0, (0, 0), what, (), ())
    assert schedule._form(one) == "shift_ri"
    assert schedule._pair_class(one) == "np"


@pytest.mark.parametrize("mnemonic", ["les", "lfs", "lgs"])
def test_complete_far_pointer_loads_share_one_priced_form(mnemonic: str) -> None:
    """indexed.lru_use became unpriced when allocation selected LFS.

    LES, LFS and LGS are the same complete far-pointer load shape with a
    different selector destination.  The quality instrument must not report
    the FS/GS spellings as free or unknown work.
    """
    assert cycles.classify(mnemonic, "bx,[bp+6]", "") == "les"
    assert all(cpu.profile(name).prices("les") for name in cpu.names())


def test_x87_exchange_is_explicitly_priced_for_every_cpu() -> None:
    """Retaining C nbody's dx made the report unpriced as soon as it emitted FXCH."""
    expected = {"386": 18, "486": 4, "P5": 1, "P6": 0, "K5": 2, "K6": 2, "K7": 2, "Core": 0}

    assert cycles.classify("fxch", "st1", "d9c9") == "x87_exchange"
    assert {name: cpu.profile(name).cost("x87_exchange") for name in cpu.names()} == expected


def test_direct_mir_default_keeps_medium_model_address_legality(monkeypatch: pytest.MonkeyPatch) -> None:
    """A direct MIR caller priced ``index * 4`` as a free native address.

    ``transform.applied()`` is public test/tooling infrastructure as well as
    the common optimization boundary.  Omitting its optional CPU details must
    retain native 16-bit addressing, not silently use a costless 67h form
    without receiving the complete target profile.
    """
    observed = []
    real = strength.reduced

    def recording(*args, **kwargs):
        observed.append(args[4])
        return real(*args, **kwargs)

    monkeypatch.setattr(strength, "reduced", recording)
    body = mir.MirBody(0, (mir.MirBlock(0, (), (), ()),))

    transform.applied(body, frozenset(), {}, only="strength")

    assert observed == [frozenset({1})]


def test_direct_mir_default_has_a_bounded_complete_peel_budget(monkeypatch: pytest.MonkeyPatch) -> None:
    """The QB frontend expanded SC_INIT's 25 stores when it omitted two knobs.

    ``transform.applied`` is the shared public optimization boundary.  An
    omitted target tuning value must mean the default 386 policy, not
    unlimited code growth; a caller that deliberately wants no ceiling can
    still pass zero explicitly.
    """
    observed = []

    def recording(body, where, *, optimize, watch=None):
        observed.append((where.max_unroll_iterations, where.max_unrolled_operations))
        return body

    monkeypatch.setattr(unroll, "optimized", recording)
    body = mir.MirBody(0, (mir.MirBlock(0, (), (), ()),))

    transform.applied(body, frozenset(), {})

    assert observed
    assert set(observed) == {(16, 200)}


def test_unknown_cpu_is_rejected_at_the_shared_boundary() -> None:
    with pytest.raises(ValueError, match="unknown CPU target"):
        cpu.profile("pentium")


def test_machine_pipeline_gives_allocator_the_complete_cpu_profile() -> None:
    phases = flow.machine({}, cpu="P5")
    allocator = next(one for one in phases if isinstance(one, allocate.RegAlloc))
    floating = next(one for one in phases if isinstance(one, floatalloc.FloatAlloc))
    scheduler = next(one for one in phases if isinstance(one, schedule.Scheduler))
    assert allocator.cpu is cpu.profile("P5")
    assert floating.cpu is cpu.profile("P5")
    assert scheduler.cpu is cpu.profile("P5")


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
        observed.append(
            (
                kwargs.get("costs"),
                kwargs.get("index_scales"),
                kwargs.get("address_forms"),
                kwargs.get("max_unroll_iterations"),
            )
        )
        return real(*args, **kwargs)

    monkeypatch.setattr(transform, "applied", recording)
    cfront.compiled((FIXTURES / "halve.cgs").read_text(), "halve", optimise=True, cpu="P5")

    assert observed
    assert {costs for costs, _scales, _forms, _limit in observed} == {cpu.profile("P5").operations}
    assert {scales for _costs, scales, _forms, _limit in observed} == {cpu.profile("P5").address_scales}
    assert {forms for _costs, _scales, forms, _limit in observed} == {cpu.profile("P5").address_forms}
    assert {limit for _costs, _scales, _forms, limit in observed} == {cpu.profile("P5").max_unroll_iterations}


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


def test_formula_selection_recomputes_a_cheap_scaled_index_under_pressure() -> None:
    """farloadloop carried ``i * 2`` in a spilled recurrence.

    A power-of-two product lowers to one shift.  With no recurrence slot
    available, its shift is cheaper than a memory update plus the indexed
    use's reload; a genuine multiply remains worth carrying under the same
    pressure.
    """
    counter = mir.Value(10, 0)
    answer = mir.Value(11, 1)
    affine = induction.Affine(counter.id, mir.Const(0, 2), mir.Const(1, 2), 1)

    def formula(by: mir.Const | mir.Held) -> induction.Derived:
        multiply = mir.Op(
            1,
            ir.Operation.MULTIPLY,
            "imul",
            (answer,),
            (counter,),
            kind=mir.Kind.MUL,
            args=(mir.Held(counter, 2), by),
            results=(mir.Held(answer, 2),),
        )
        return induction.Derived(multiply, affine, by)

    costs = OperationCosts(add=2, multiply=22, shift=3, load=4, memory_update=8)

    assert strength._formula_set([formula(mir.Const(2, 2))], room=0, costs=costs, references={answer.id: 1}) == []
    assert strength._formula_set(
        [formula(mir.Held(mir.Value(12, 0), 2))], room=0, costs=costs, references={answer.id: 1}
    )


def test_formula_selection_prices_a_complete_affine_formula_under_pressure() -> None:
    """Mandelbrot rebuilt ``24*x - 128 + seed`` in its inner loop.

    The leaf operation is an addition, but the recurrence candidate denotes
    the complete affine formula.  Pricing only that leaf made recomputation
    appear cheaper than a spilled recurrence by omitting the multiply and the
    other invariant addition.
    """
    counter = mir.Value(10, 0)
    answer = mir.Value(11, 1)
    seed = mir.Value(12, 0)
    affine = induction.Affine(counter.id, mir.Const(0, 4), mir.Const(1, 4), 1)
    leaf = mir.Op(
        1,
        ir.Operation.BINARY,
        "add",
        (answer,),
        (counter, seed),
        kind=mir.Kind.ADD,
        args=(mir.Held(counter, 4), mir.Held(seed, 4)),
        results=(mir.Held(answer, 4),),
    )
    complete = induction.Derived(
        leaf,
        affine,
        mir.Const(24, 4),
        ((mir.Const(-128, 4), 1), (mir.Held(seed, 4), 1)),
    )
    costs = OperationCosts(add=2, multiply=22, shift=3, address=2, load=4, memory_update=8)

    assert strength._formula_set([complete], room=0, costs=costs, references={answer.id: 1}) == [complete]


def test_formula_selection_uses_67h_before_spilling_or_recomputing() -> None:
    """A scaled far index overflowed the register budget and was recomputed.

    In 16-bit mode the 386 address-size override is a legal SIB form.  It
    costs a byte and a target-specific prefix penalty, but no loop-carried
    register; select it before either a frame-backed recurrence or rebuilding
    the scale and address in every iteration.
    """
    counter = mir.Value(10, 0)
    answer = mir.Value(11, 1)
    affine = induction.Affine(counter.id, mir.Const(0, 2), mir.Const(1, 2), 1)
    multiply = mir.Op(
        1,
        ir.Operation.MULTIPLY,
        "imul",
        (answer,),
        (counter,),
        kind=mir.Kind.MUL,
        args=(mir.Held(counter, 2), mir.Const(2, 2)),
        results=(mir.Held(answer, 2),),
    )
    formula = induction.Derived(multiply, affine, mir.Const(2, 2))
    secondary = AddressForm(
        4,
        frozenset({1, 2, 4, 8}),
        extra_bytes=1,
        use_cost=1,
        extension_cost=4,
        secondary=True,
    )

    activated = strength._secondary_indexes(
        [formula],
        room=0,
        native=frozenset(),
        secondary={id(multiply): (2, secondary)},
        references={answer.id: 1},
    )

    assert activated == {id(multiply)}


def test_secondary_address_form_is_ranked_against_reload_and_spill_work() -> None:
    """indexed.lru_use improved on Core but regressed P5/P6 when every legal
    67h form was treated as equally profitable.

    Legality is common; selection is per profile.  The audited extension,
    prefix, native address, scale and spill prices admit the form on 386,
    K5/K6/K7 and Core while retaining native reloads on 486, P5 and P6.
    """
    selected = {
        name for name in cpu.names() if cpu.profile(name).address_forms[1].before_spill(cpu.profile(name).operations)
    }

    assert selected == {"386", "K5", "K6", "K7", "Core"}
