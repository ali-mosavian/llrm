from pathlib import Path
from dataclasses import replace

import pytest

from qbopt.model import ir
from qbopt.model import mir
from qbopt.analysis import ssa
from qbopt.analysis import loops
from qbopt.optimize import strength
from qbopt.analysis import induction
from qbopt.optimize import transform
from qbopt.objectfile.module import Addr
from qbopt.objectfile.module import Space


@pytest.mark.parametrize("tag", ["p-g2", "q-O", "v-g3"])
def test_huge_loop_byte_offsets_are_induction_variables(tag):
    """HUGELP recomputed 32-bit array strides after every narrow index extension."""
    from qbopt import wholeseg
    states = []
    def watch(stage, name, state):
        if stage == "mir-widen":
            states.append(state)
    result = wholeseg.emitted(Path(f"fixtures/regressions/hugelp-{tag}.obj").read_bytes(), watch=watch)
    assert result.outcome is wholeseg.Emission.LIR, result.reason
    body = states[0]
    loop = loops.loops(body.blocks, body.entry)[0]
    offsets = [op.args[1] for block in body.blocks if block.at in loop.body
               for op in block.ops if op.kind is mir.Kind.PTR_OFFSET]
    assert len(offsets) == 2
    assert not any(op.kind in (mir.Kind.MUL, mir.Kind.SIGN_EXTEND)
                   for block in body.blocks if block.at in loop.body for op in block.ops)


@pytest.mark.parametrize("tag", ["p-g2", "q-O", "v-g3"])
def test_huge_loop_carries_whole_pointers(tag):
    """HUGELP rebuilt two pointers from a spilled base and byte offsets on every iteration."""
    from qbopt import wholeseg
    states = []
    def watch(stage, name, state):
        if stage == "mir-widen":
            states.append(state)
    result = wholeseg.emitted(Path(f"fixtures/regressions/hugelp-{tag}.obj").read_bytes(), watch=watch)
    assert result.outcome is wholeseg.Emission.LIR, result.reason
    body = states[0]
    loop = loops.loops(body.blocks, body.entry)[0]
    carried = {phi.result for block in body.blocks for phi in block.phis}
    stores = [ref for block in body.blocks if block.at in loop.body for op in block.ops
              for ref in op.stores if ref.pointer]
    assert len(stores) == 2 and all(ref.base in carried for ref in stores)


@pytest.mark.parametrize("offset, accepted", [(4, True), (-2, True), (32767, False), (-32769, False)])
def test_sign_extended_recurrence_requires_no_narrow_wrap(offset, accepted, monkeypatch):
    """A 16-bit index crossing 32767 must not become a steadily increasing 32-bit stride."""
    from qbopt import wholeseg
    from qbopt.analysis import consts
    monkeypatch.setattr(strength, "reduced", lambda body, *args: body)
    states = []
    def watch(stage, name, state):
        if stage == "mir-widen":
            states.append(state)
    wholeseg.emitted(Path("fixtures/regressions/hugelp-p-g2.obj").read_bytes(), watch=watch)
    built = states[0]
    loop = loops.loops(built.blocks, built.entry)[0]
    counters = induction.basics(built, loop)
    counter = next(iter(counters.values()))
    facts = consts.known(built)
    assert induction._last_counter(built, loop, counter, facts, 2) == 1
    source, result = mir.Value(9000, 0), mir.Value(9001, 0)
    extension = mir.Op(0, ir.Operation.EXTEND, "", (result,), (source,), kind=mir.Kind.SIGN_EXTEND,
                       args=(mir.Held(source, 2),), results=(mir.Held(result, 4),))
    form = (counter, 1, ((mir.Const(offset, 2), 1),))
    # -32769 has the valid 16-bit spelling 32767 and crosses the upper bound too.
    got = induction._extended(built, loop, extension, {source.id: form}, facts)
    assert (got is not None) == accepted


@pytest.mark.parametrize("tag", ["p-g2", "q-O", "v-g3"])
def test_matrix_reduced_stride_keeps_its_multiplier_address(tag):
    """MATRIX printed T=190 instead of T=380 after its stride read DS:0 instead of w."""
    from qbopt import wholeseg
    from qbopt.objectfile import module, omf
    from qbopt.frontend import blocks

    result = wholeseg.emitted(Path(f"fixtures/omf/matrix-{tag}.obj").read_bytes())
    assert result.outcome is wholeseg.Emission.LIR, result.reason
    found = module.of(omf.parse(result.data))
    for one in blocks.instructions(found):
        if one.disp_at is not None and one.disp_len == 2:
            assert found.code[one.disp_at:one.disp_at + 2] != b"\0\0" or one.disp_at in found.fixup_at


@pytest.mark.parametrize("tag", ["p-g2", "q-O", "v-g3"])
def test_harr_hoisted_descriptor_read_keeps_its_address(tag):
    """HARR's reduced pointer read DS:0 instead of the array-base descriptor field."""
    from qbopt import wholeseg
    from qbopt.objectfile import module, omf
    from qbopt.frontend import blocks

    result = wholeseg.emitted(Path(f"fixtures/omf/harr-{tag}.obj").read_bytes())
    assert result.outcome is wholeseg.Emission.LIR, result.reason
    found = module.of(omf.parse(result.data))
    bad = [one.at for one in blocks.instructions(found)
           if one.disp_at is not None and one.disp_len == 2
           and found.code[one.disp_at:one.disp_at + 2] == b"\0\0"
           and one.disp_at not in found.fixup_at]
    assert not bad, f"unrelocated zero displacements: {bad}"


def test_nbody_inner_counter_has_a_proven_upper_bound() -> None:
    """Nbody's conditional interaction body hid the 0..5 counter range from the two-block proof."""
    import corpus
    from qbopt.analysis import consts
    path = Path("fixtures/regressions/nbody-stack-p-g2.obj")
    found = corpus.loaded(path)
    body = mir.bodies(found, corpus.partitioned(path))[0][1]
    body = transform.applied(body, found.dgroup, found.calls, blocks=corpus.partitioned(path), found=found)
    loop = next(loop for loop in loops.loops(body.blocks, body.entry) if loop.header == 0x21c)
    counters = induction.basics(body, loop)
    assert len(counters) == 1
    assert induction._last_counter(body, loop, next(iter(counters.values())), consts.known(body), 2) == 5
    escaping = replace(body, blocks=tuple(replace(block, succ=(0x227,)) if block.at == 0x117 else block for block in body.blocks))
    assert induction._last_counter(escaping, loop, next(iter(counters.values())), consts.known(body), 2) is None


@pytest.mark.parametrize("tag", ["p-g2", "q-O", "v-g3"])
def test_harr_stored_row_plus_column_is_loop_carried(tag: str) -> None:
    """HARR recomputed row + column for every element instead of advancing its stored value."""
    import corpus

    path = Path(f"fixtures/omf/harr-{tag}.obj")
    found = corpus.loaded(path)
    partition = corpus.partitioned(path)
    built = mir.bodies(found, partition)[0][1]
    result = transform.applied(built, found.dgroup, found.calls, blocks=partition, found=found)
    store = next(op for block in result.blocks for op in block.ops if any(ref.allocation for ref in op.stores))
    source = next(arg.value for arg in store.args if isinstance(arg, mir.Held))
    carried = {phi.result for block in result.blocks for phi in block.phis}
    assert source in carried


@pytest.mark.parametrize("changed", [False, True])
def test_harr_descriptor_offset_is_read_before_inner_loop_unless_written(changed) -> None:
    """HARR rebuilt its full pointer with an invariant descriptor read on every iteration."""
    import corpus

    path = Path("fixtures/omf/harr-p-g2.obj")
    found = corpus.loaded(path)
    partition = corpus.partitioned(path)
    built = mir.bodies(found, partition)[0][1]
    field = mir.MemRef(Addr(Space.SEGMENT, 16, found.program_data), 2)
    if changed:
        built = replace(
            built,
            blocks=tuple(
                replace(
                    block,
                    ops=tuple(
                        replace(op, stores=(field,), results=(mir.Cell(field),)) if op.at == 0x78 else op
                        for op in block.ops
                    ),
                )
                for block in built.blocks
            ),
        )
    result = transform.applied(built, found.dgroup, found.calls, blocks=partition, found=found)
    inner = next(block for block in result.blocks if block.at == 0x58)
    assert any(mir.same_bytes(ref, field) for op in inner.ops for ref in op.loads) is changed
    if not changed:
        from qbopt.analysis import loops
        dominators = loops.dominators(list(result.blocks), result.entry)
        reads = [(block, op, ref) for block in result.blocks for op in block.ops for ref in op.loads
                 if mir.same_bytes(ref, field)]
        assert reads
        assert all(block.at in dominators[0x52] for block, op, ref in reads)
        assert all(ref.base in op.uses for block, op, ref in reads if ref.base is not None)


def test_nested_address_advances_instead_of_recomputing_row_plus_column() -> None:
    """NESTED rebuilt (row * width + column) * 2 on each of 30 inner iterations."""
    import corpus

    path = Path("fixtures/omf/nested-p-g2.obj")
    found = corpus.loaded(path)
    partition = corpus.partitioned(path)
    built = mir.bodies(found, partition)[0][1]
    result = transform.applied(built, found.dgroup, found.calls, blocks=partition, found=found)
    inner = next(block for block in result.blocks if block.at == 0x5A)
    assert not any(op.kind is mir.Kind.SHL for op in inner.ops)
    header = next(block for block in result.blocks if block.at == 0x86)
    assert any(
        op.kind is mir.Kind.ADD
        and mir.Const(2, 2) in op.args
        and any(phi.incoming.get(inner.at) in op.defines for phi in header.phis)
        for op in inner.ops
    )


@pytest.mark.parametrize("program", ["matrix", "addrm"])
def test_strength_does_not_spill_cheap_loop_work(monkeypatch: pytest.MonkeyPatch, program: str) -> None:
    """Matrix rose 11,916 -> 12,974 with an outer counter; ADDRM rose 2,308 -> 2,578 replacing shifts."""
    import opportunity

    obj = Path(f"fixtures/omf/{program}-p-g2.obj")
    applied = transform.applied
    monkeypatch.setattr(transform, "applied", lambda *args, **kwargs: applied(*args, **{**kwargs, "strength_": False}))
    baseline = opportunity.counted([obj])["cost"]

    def reduced(*args, **kwargs):
        return applied(*args, **{**kwargs, "strength_": True})

    monkeypatch.setattr(transform, "applied", reduced)
    cost = opportunity.counted([obj])["cost"]
    assert cost <= baseline
    if program == "matrix":
        assert cost < baseline


def body() -> tuple[mir.MirBody, loops.Loop]:
    start = mir.Value(10, 0, variable=7)
    counter = mir.Value(11, 1, variable=7)
    following = mir.Value(12, 1, variable=7)
    unrelated = mir.Value(13, 1, variable=7)
    answer = mir.Value(14, 1, variable=8)
    step = mir.Op(
        1,
        ir.Operation.UNARY,
        "",
        (following,),
        (counter,),
        kind=mir.Kind.INCREMENT,
        args=(mir.Held(counter, 2),),
        results=(mir.Held(following, 2),),
    )
    multiply = mir.Op(
        2,
        ir.Operation.BINARY,
        "",
        (answer,),
        (unrelated,),
        kind=mir.Kind.MUL,
        args=(mir.Held(unrelated, 2), mir.Const(2, 2)),
        results=(mir.Held(answer, 2),),
    )
    blocks = (
        mir.MirBlock(0, (), (), (1,)),
        mir.MirBlock(1, (mir.Phi(counter, {0: start, 1: following}),), (step, multiply), (1, 2)),
        mir.MirBlock(2, (), (), ()),
    )
    return mir.MirBody(0, blocks), loops.Loop(1, frozenset({1}), frozenset({1}))


def test_a_shared_variable_name_is_not_a_shared_recurrence() -> None:
    """harr's shifted address was mistaken for both loop counters because all shared a variable number."""
    built, loop = body()
    assert induction.basics(built, loop)
    assert not induction.derived(built, loop)
    header = built.blocks[1]
    counter = header.phis[0].result
    multiply = replace(header.ops[1], uses=(counter,), args=(mir.Held(counter, 2), mir.Const(2, 2)))
    positive = replace(built, blocks=(built.blocks[0], replace(header, ops=(header.ops[0], multiply)), built.blocks[2]))
    assert len(induction.derived(positive, loop)) == 1


def test_the_backedge_must_step_the_exact_phi_value() -> None:
    built, loop = body()
    header = built.blocks[1]
    unrelated = header.ops[1].args[0]
    step = replace(header.ops[0], args=(unrelated,), uses=(unrelated.value,))
    built = replace(built, blocks=(built.blocks[0], replace(header, ops=(step, header.ops[1])), built.blocks[2]))
    assert not induction.basics(built, loop)


@pytest.mark.parametrize("copied", [False, True])
def test_long_recurrence_keeps_its_width(copied: bool) -> None:
    """LNGMXX's long accumulator was described with a truncated 16-bit start."""
    built, loop = body()
    header = built.blocks[1]
    update = header.ops[0]
    update = replace(update, args=(mir.Held(update.uses[0], 4),), results=(mir.Held(update.defines[0], 4),))
    ops = (update,)
    if copied:
        temporary = mir.Value(100, 1)
        original = update.defines[0]
        update = replace(update, defines=(temporary,), results=(mir.Held(temporary, 4),))
        copy = mir.Op(3, ir.Operation.MOVE, "", (original,), (temporary,), kind=mir.Kind.COPY,
                      args=(mir.Held(temporary, 4),), results=(mir.Held(original, 4),))
        ops = (update, copy)
    built = replace(built, blocks=(built.blocks[0], replace(header, ops=ops), built.blocks[2]))
    recurrence = induction.basics(built, loop)[header.phis[0].result.id]
    assert recurrence.start.width == 4
    assert recurrence.step == mir.Const(1, 4)


@pytest.mark.parametrize("tag", ["p-g2", "q-O", "v-g3"])
def test_lngmxx_accumulator_has_a_whole_long_start(tag: str, monkeypatch) -> None:
    """LNGMXX's 32-bit sum was reported as starting with only its low word."""
    import corpus
    from qbopt.optimize import loopexit

    # Inspect recurrence analysis before exit evaluation removes the loop.
    monkeypatch.setattr(loopexit, "evaluated", lambda body: body)

    path = Path(f"fixtures/omf/lngmxx-{tag}.obj")
    found = corpus.loaded(path)
    partition = corpus.partitioned(path)
    built = mir.bodies(found, partition)[0][1]
    result = transform.applied(built, found.dgroup, found.calls, blocks=partition, found=found)
    recurrences = [counter for loop in loops.loops(result.blocks, result.entry)
                   for counter in induction.basics(result, loop).values()]
    accumulator, = [counter for counter in recurrences if isinstance(counter.step, mir.Held)]
    assert accumulator.start.width == accumulator.step.width == 4


@pytest.mark.parametrize(("factor", "expected"), [(20, 42), (32767, 0)])
def test_composed_word_address_has_one_recurrence(factor: int, expected: int) -> None:
    """Matrix's (i * 20 + i) << 1 needs stride 42, not a separate stride-20 temporary."""
    built, loop = body()
    header = built.blocks[1]
    counter = header.phis[0].result
    product = header.ops[1].results[0].value
    summed = mir.Value(30, 1, variable=30)
    address = mir.Value(31, 1, variable=31)
    factor_value = mir.Value(32, 0, variable=32)
    constant = replace(
        header.ops[1],
        kind=mir.Kind.COPY,
        defines=(factor_value,),
        uses=(),
        args=(mir.Const(factor, 2),),
        results=(mir.Held(factor_value, 2),),
    )
    multiply = replace(
        header.ops[1], uses=(counter, factor_value), args=(mir.Held(counter, 2), mir.Held(factor_value, 2))
    )
    add = replace(
        multiply,
        kind=mir.Kind.ADD,
        defines=(summed,),
        uses=(product, counter),
        args=(mir.Held(product, 2), mir.Held(counter, 2)),
        results=(mir.Held(summed, 2),),
    )
    shift = replace(
        multiply,
        kind=mir.Kind.SHL,
        defines=(address,),
        uses=(summed,),
        args=(mir.Held(summed, 2), mir.Const(1, 2)),
        results=(mir.Held(address, 2),),
    )
    built = replace(
        built,
        blocks=(
            replace(built.blocks[0], ops=(constant,)),
            replace(header, ops=(*header.ops[:1], multiply, add, shift)),
            built.blocks[2],
        ),
    )
    derived = induction.derived(built, loop)
    assert next(one.by for one in derived if one.op is shift) == mir.Const(expected, 2)


@pytest.mark.parametrize("mismatch", ["start", "step", "unchanged", "none"])
def test_every_incoming_path_agrees_on_the_recurrence(mismatch: str) -> None:
    built, loop = body()
    header = built.blocks[1]
    phi = header.phis[0]
    start = phi.incoming[0]
    following = mir.Value(20, 3, variable=7)
    step = replace(header.ops[0], at=3, defines=(following,), results=(mir.Held(following, 2),))
    if mismatch == "step":
        step = replace(step, kind=mir.Kind.DECREMENT)
    incoming = {
        **phi.incoming,
        4: mir.Value(21, 4, variable=7) if mismatch == "start" else start,
        3: phi.result if mismatch == "unchanged" else following,
    }
    built = replace(
        built,
        blocks=(
            built.blocks[0],
            replace(header, phis=(replace(phi, incoming=incoming),), succ=(1, 2, 3)),
            built.blocks[2],
            mir.MirBlock(3, (), (step,), (1,)),
            mir.MirBlock(4, (), (), (1,)),
        ),
    )
    loop = replace(loop, body=frozenset({1, 3}), latches=frozenset({1, 3}))
    assert bool(induction.basics(built, loop)) == (mismatch == "none")


@pytest.mark.parametrize(("width", "count"), [(2, 1), (4, 0)])
def test_only_width_preserving_copies_carry_the_recurrence(width: int, count: int) -> None:
    built, loop = body()
    header = built.blocks[1]
    counter = header.phis[0].result
    copied = header.ops[1].args[0].value
    copy = mir.Op(
        1,
        ir.Operation.MOVE,
        "",
        (copied,),
        (counter,),
        kind=mir.Kind.COPY,
        args=(mir.Held(counter, width),),
        results=(mir.Held(copied, 2),),
    )
    built = replace(built, blocks=(built.blocks[0], replace(header, ops=(copy, *header.ops)), built.blocks[2]))
    assert len(induction.derived(built, loop)) == count


@pytest.mark.parametrize("shape", ["variable", "counter_count", "constant", "oversized"])
def test_a_shift_recurrence_requires_a_constant_count(shape: str) -> None:
    built, loop = body()
    header = built.blocks[1]
    counter = mir.Held(header.phis[0].result, 2)
    amount = mir.Const(3, 2) if shape == "constant" else mir.Held(header.phis[0].incoming[0], 2)
    if shape == "oversized":
        amount = mir.Const(32, 2)
    args = (mir.Const(3, 2), counter) if shape == "counter_count" else (counter, amount)
    shift = replace(
        header.ops[1],
        kind=mir.Kind.SHL,
        args=args,
        uses=tuple(one.value for one in args if isinstance(one, mir.Held)),
    )
    built = replace(built, blocks=(built.blocks[0], replace(header, ops=(header.ops[0], shift)), built.blocks[2]))
    derived = induction.derived(built, loop)
    assert len(derived) == (1 if shape == "constant" else 0)
    if derived:
        assert derived[0].by == mir.Const(8, 2)


@pytest.mark.parametrize("address", [False, True])
def test_a_reduced_counter_has_its_own_loop_phi_and_fresh_variable(address: bool) -> None:
    """harr's experimental stride reused a promoted variable and read its initial value on every iteration."""
    built, _loop = body()
    header = built.blocks[1]
    counter = header.phis[0].result
    answer = header.ops[1].defines[0]
    multiply = replace(header.ops[1], uses=(counter,), args=(mir.Held(counter, 2), mir.Const(2, 2)), covers=(2, 4))
    livein = mir.Value(99, 0, variable=77)
    use = mir.Op(
        4,
        ir.Operation.PUSH,
        "",
        (),
        (answer, livein),
        kind=mir.Kind.ARG,
        args=(mir.Held(answer, 2), mir.Held(livein, 2)),
        covers=(4, 6),
    )
    if address:
        memory = mir.MemRef(Addr(Space.SEGMENT, 0x20, 1), 2, answer)
        use = replace(use, args=(mir.Cell(memory), mir.Held(livein, 2)), loads=(memory,))
    built = replace(
        built,
        blocks=(
            built.blocks[0],
            replace(header, ops=(replace(header.ops[0], covers=(1, 2)), multiply, use)),
            built.blocks[2],
        ),
    )
    result = strength.reduced(built)
    after = result.blocks[1]
    added = [phi for phi in after.phis if phi.result != counter]
    assert len(added) == 1
    phi = added[0]
    assert phi.result.variable > livein.variable
    consumer = next(op for op in after.ops if op.kind is mir.Kind.ARG)

    def source(value: mir.Value) -> mir.Value:
        definition = next((op for op in after.ops if value in op.defines), None)
        return definition.args[0].value if definition is not None and definition.kind is mir.Kind.COPY else value

    if address:
        assert source(consumer.args[0].ref.base) == phi.result
        assert source(consumer.loads[0].base) == phi.result
    else:
        assert source(consumer.args[0].value) == phi.result
    assert phi.incoming[0] != phi.incoming[1]
    step = next(op for op in after.ops if phi.incoming[1] in op.defines)
    assert step.args[0].value == phi.result


@pytest.mark.parametrize("use", ["low", "high", "both_through_phis"])
def test_reduction_preserves_every_live_product_result(use: str) -> None:
    built, _loop = body()
    header = built.blocks[1]
    low = header.ops[1].defines[0]
    high = mir.Value(30, 1, variable=9)
    middle = mir.Value(31, 2, variable=9)
    final = mir.Value(32, 3, variable=9)
    product = replace(header.ops[1], defines=(low, high), results=(mir.Held(low, 2), mir.Held(high, 2)))
    values = (low,) if use == "low" else (high,) if use == "high" else (low, final)
    consume = mir.Op(
        3, ir.Operation.PUSH, "", (), values, kind=mir.Kind.ARG, args=tuple(mir.Held(value, 2) for value in values)
    )
    built = replace(
        built,
        blocks=(
            built.blocks[0],
            replace(header, ops=(header.ops[0], product)),
            mir.MirBlock(2, (mir.Phi(middle, {1: high}),), (), (3,)),
            mir.MirBlock(3, (mir.Phi(final, {2: middle}),), (consume,), ()),
        ),
    )
    assert strength._answer(built, product) == (low if use == "low" else None)


@pytest.mark.parametrize("bypass", [False, True])
def test_reduction_does_not_speculate_on_a_loop_bypass(bypass: bool) -> None:
    built, _loop = body()
    header = built.blocks[1]
    counter = header.phis[0].result
    answer = header.ops[1].defines[0]
    memory = mir.MemRef(Addr(Space.SEGMENT, 0x20, 1), 2)
    product = replace(
        header.ops[1], uses=(counter,), args=(mir.Held(counter, 2), mir.Cell(memory)), loads=(memory,), covers=(2, 4)
    )
    consume = mir.Op(
        4, ir.Operation.PUSH, "", (), (answer,), kind=mir.Kind.ARG, args=(mir.Held(answer, 2),), covers=(4, 6)
    )
    built = replace(
        built,
        blocks=(
            replace(built.blocks[0], succ=(1, 2) if bypass else (1,)),
            replace(header, ops=(replace(header.ops[0], covers=(1, 2)), product, consume)),
            built.blocks[2],
        ),
    )
    assert (strength.reduced(built) == built) == bypass


def test_existing_phi_inputs_follow_their_predecessor_versions() -> None:
    initial = mir.Value(10, 0, variable=7)
    updated = mir.Value(11, 1, variable=7)
    joined = mir.Value(12, 2, variable=8)
    define = mir.Op(
        0,
        ir.Operation.MOVE,
        "",
        (initial,),
        (),
        kind=mir.Kind.COPY,
        args=(mir.Const(1, 2),),
        results=(mir.Held(initial, 2),),
    )
    step = replace(
        define,
        at=1,
        defines=(updated,),
        uses=(initial,),
        kind=mir.Kind.ADD,
        args=(mir.Held(initial, 2), mir.Const(1, 2)),
        results=(mir.Held(updated, 2),),
    )
    built = mir.MirBody(
        0,
        (
            mir.MirBlock(0, (), (define,), (1, 2)),
            mir.MirBlock(1, (), (step,), (2,)),
            mir.MirBlock(2, (mir.Phi(joined, {0: initial, 1: initial}),), (), ()),
        ),
    )
    result = ssa.constructed(built, frozenset({7}))
    phi = result.blocks[2].phis[0]
    assert phi.result == joined
    assert phi.incoming[0] == result.blocks[0].ops[0].defines[0]
    assert phi.incoming[1] == result.blocks[1].ops[0].defines[0]
    assert [len(block.ops) for block in result.blocks] == [1, 1, 0]


def test_reduced_product_keeps_the_current_iteration_on_exit() -> None:
    built, _loop = body()
    header = built.blocks[1]
    counter = header.phis[0].result
    product = replace(header.ops[1], uses=(counter,), args=(mir.Held(counter, 2), mir.Const(3, 2)), covers=(2, 4))
    answer = product.defines[0]
    exit_value = mir.Value(40, 2, variable=10)
    consume = mir.Op(5, ir.Operation.PUSH, "", (), (exit_value,), kind=mir.Kind.ARG, args=(mir.Held(exit_value, 2),))
    built = replace(
        built,
        blocks=(
            built.blocks[0],
            replace(header, ops=(replace(header.ops[0], covers=(1, 2)), product)),
            mir.MirBlock(2, (mir.Phi(exit_value, {1: answer}),), (consume,), ()),
        ),
    )
    result = strength.reduced(built)
    after = result.blocks[1]
    phi = next(one for one in after.phis if one.result != counter)
    preserved = next((op for op in after.ops if answer in op.defines), None)
    assert preserved is not None
    assert preserved.kind is mir.Kind.COPY
    assert preserved.args == (mir.Held(phi.result, 2),)
    assert result.blocks[2].phis[0].incoming[1] == answer
    assert preserved.args[0].value != phi.incoming[1]


def test_inserted_counter_operations_own_their_insertion_location() -> None:
    built, _loop = body()
    header = built.blocks[1]
    counter = header.phis[0].result
    product = replace(header.ops[1], uses=(counter,), args=(mir.Held(counter, 2), mir.Const(3, 2)), covers=(2, 4))
    answer = product.defines[0]
    consume = mir.Op(
        4, ir.Operation.PUSH, "", (), (answer,), kind=mir.Kind.ARG, args=(mir.Held(answer, 2),), covers=(4, 6)
    )
    built = replace(
        built,
        blocks=(
            built.blocks[0],
            replace(header, ops=(replace(header.ops[0], covers=(1, 2)), product, consume)),
            built.blocks[2],
        ),
    )
    result = strength.reduced(built)
    setup = result.blocks[0].ops[0]
    update = result.blocks[1].ops[-1]
    assert setup.at == 0 and setup.covers == (0, 0)
    assert update.at == 4 and update.covers == (6, 6)


@pytest.mark.parametrize("second_variable", [7, 8])
@pytest.mark.parametrize("has_origin", [False, True])
@pytest.mark.parametrize("symbolic", [False, True])
def test_cse_replaces_phi_uses_of_a_deleted_initializer(second_variable: int, has_origin: bool, symbolic: bool) -> None:
    """matrix printed T=0 for T=380 after CSE deleted a zero still named by its loop phi."""
    from pathlib import Path

    import corpus

    node = next(
        node
        for body in corpus.bodies(Path("fixtures/omf/matrix-p-g2.obj"))
        for node in body.nodes
        if isinstance(node, ir.Opaque)
    )
    first = mir.Value(10, 0, variable=7)
    second = mir.Value(11, 2, variable=second_variable)
    result = mir.Value(12, 4, variable=7)
    define = mir.Op(
        0,
        ir.Operation.MOVE,
        "",
        (first,),
        (),
        kind=mir.Kind.COPY,
        args=(mir.Symbol(Space.SEGMENT, 5, 6, 2) if symbolic else mir.Const(0, 2),),
        results=(mir.Held(first, 2),),
        covers=(0, 2),
        node=node if has_origin else None,
    )
    duplicate = replace(define, at=2, defines=(second,), results=(mir.Held(second, 2),), covers=(2, 4))
    jump = mir.Op(4, ir.Operation.JUMP, "", (), (), kind=mir.Kind.JUMP, covers=(4, 6), target=6)
    use = mir.Op(6, ir.Operation.PUSH, "", (), (result,), kind=mir.Kind.ARG, args=(mir.Held(result, 2),))
    direct = replace(use, at=7, uses=(second,), args=(mir.Held(second, 2),))
    built = mir.MirBody(
        0,
        (
            mir.MirBlock(0, (), (define,), (2,)),
            mir.MirBlock(2, (), (duplicate, jump), (6,)),
            mir.MirBlock(6, (mir.Phi(result, {2: second}),), (use, direct), ()),
        ),
    )
    after = transform.subexpressions(built)
    assert all(second not in op.defines for block in after.blocks for op in block.ops)
    assert after.blocks[2].phis[0].incoming[2] == first
    assert after.blocks[2].ops[1].args == (mir.Held(first, 2),)


@pytest.mark.parametrize("other", [mir.Const(0, 2), mir.Symbol(Space.SEGMENT, 5, 8, 2)])
def test_cse_keeps_distinct_linker_addresses(other: mir.Arg) -> None:
    """Descriptor addresses encoded as zero must not become the same value."""
    first, second = mir.Value(100, 0), mir.Value(101, 2)
    define = mir.Op(
        0,
        ir.Operation.MOVE,
        "mov",
        (first,),
        (),
        kind=mir.Kind.COPY,
        args=(mir.Symbol(Space.SEGMENT, 5, 6, 2),),
        results=(mir.Held(first, 2),),
        covers=(0, 2),
    )
    different = replace(define, at=2, defines=(second,), args=(other,), results=(mir.Held(second, 2),), covers=(2, 4))
    use = mir.Op(4, ir.Operation.PUSH, "push", (), (second,), kind=mir.Kind.ARG, args=(mir.Held(second, 2),))
    built = mir.MirBody(0, (mir.MirBlock(0, (), (define, different, use), ()),))
    assert transform.subexpressions(built) == built


def test_dead_byte_transfer_cannot_span_a_surviving_jump() -> None:
    """matrix refused emission after dead assigned the live jump's nine bytes twice."""
    from pathlib import Path

    import corpus

    node = next(
        node
        for body in corpus.bodies(Path("fixtures/omf/matrix-p-g2.obj"))
        for node in body.nodes
        if isinstance(node, ir.Opaque)
    )
    first = mir.Op(0, ir.Operation.MOVE, "", (), (), kind=mir.Kind.COPY, node=node, covers=(0, 4))
    removed = replace(first, at=4, covers=(8, 10))
    jump = replace(first, at=4, kind=mir.Kind.JUMP, covers=(4, 8))
    result = transform._without([first, removed, jump], lambda op: op is removed)
    spans = sorted(op.covers for op in result if op.covers)
    assert all(left[1] <= right[0] for left, right in zip(spans, spans[1:]))


@pytest.mark.parametrize("tag", ["p-g2", "q-O", "v-g3"])
def test_nested_row_recurrences_remove_repeated_multiplication(tag):
    """NESTED recomputed both row scales because all outer-loop recurrences were disabled."""
    from pathlib import Path
    from iced_x86 import Mnemonic
    from qbopt import wholeseg
    from qbopt.objectfile import module, omf
    from qbopt.frontend import blocks
    result = wholeseg.emitted(Path(f"fixtures/omf/nested-{tag}.obj").read_bytes())
    assert result.outcome is wholeseg.Emission.LIR, result.reason
    found = module.of(omf.parse(result.data))
    assert not any(one.insn.mnemonic == Mnemonic.IMUL for one in blocks.instructions(found))
