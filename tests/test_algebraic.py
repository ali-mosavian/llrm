from pathlib import Path
from collections import Counter
from dataclasses import replace

import pytest

import corpus
from qbopt.model import ir
from qbopt.model import mir
from qbopt.optimize import algebraic
from qbopt.optimize import transform


def test_nbody_reuses_the_whole_signed_initialization_value():
    """NBODY sign-extended initialization values, extracted their high words, then rebuilt the same longs."""
    path = Path("fixtures/bench/nbody-v-g3.obj")
    body = mir.bodies(corpus.loaded(path), corpus.partitioned(path))[0][1]
    done = algebraic.simplified(body, set(), set())
    for at in (0x75, 0x9C):
        assert not any(op.at == at and op.kind is mir.Kind.CONCAT for block in done.blocks for op in block.ops)


@pytest.mark.parametrize("mismatch", ["source", "width", "kind", "offset"])
def test_signed_recombination_requires_the_exact_extension(mismatch):
    """Reusing NBODY's signed value must not join a sign word to an unrelated low word."""
    path = Path("fixtures/bench/nbody-v-g3.obj")
    body = mir.bodies(corpus.loaded(path), corpus.partitioned(path))[0][1]
    ops = [op for block in body.blocks for op in block.ops]
    definitions = {value: op for op in ops for value in op.defines}
    extension = next(op for op in ops if op.at == 0x72 and op.kind is mir.Kind.SIGN_EXTEND)
    extract = next(op for op in ops if op.at == 0x72 and op.kind is mir.Kind.EXTRACT)
    concat = next(op for op in ops if op.at == 0x75 and op.kind is mir.Kind.CONCAT)
    match mismatch:
        case "source":
            extension = replace(extension, args=(mir.Held(mir.Value(999999, 0), 2),))
        case "width":
            extension = replace(extension, results=(replace(extension.results[0], width=2),))
        case "kind":
            extension = replace(extension, kind=mir.Kind.COPY)
        case "offset":
            extract = replace(extract, args=(extract.args[0], mir.Const(0, 4)))
    definitions[extension.defines[0]] = extension
    definitions[extract.defines[0]] = extract
    assert algebraic._recombined(concat, definitions).kind is mir.Kind.CONCAT


def test_nbody_counter_comparison_joins_whole_values_before_the_loop():
    """NBODY rebuilt its long counter from two word phis for every loop comparison."""
    path = Path("fixtures/bench/nbody-v-g3.obj")
    body = mir.bodies(corpus.loaded(path), corpus.partitioned(path))[0][1]
    done = algebraic.simplified(body, set(), set())
    assert not any(op.at == 0x2FE and op.kind is mir.Kind.CONCAT for block in done.blocks for op in block.ops)
    assert any(
        op.at == 0x2FE and op.kind is mir.Kind.COPY and isinstance(op.args[0], mir.Held) and op.args[0].width == 4
        for block in done.blocks
        for op in block.ops
    )
    from qbopt.analysis import ssa

    existing = {value.variable for value in ssa.values(body)}
    added = [phi for block in done.blocks for phi in block.phis if phi.result.variable not in existing]
    assert added
    assert all(phi.result.variable == value.variable for phi in added for value in phi.incoming.values())
    header = next(block for block in done.blocks if block.at == 0x2F0)
    assert not any(phi.result.variable in (1, 3) for phi in header.phis)
    resolved = mir.resolved(done)
    assert isinstance(resolved, mir.MirBody), resolved


def test_nbody_counter_is_stored_as_one_whole_value():
    """NBODY split its whole counter into two stores and reloaded it on every backedge."""
    path = Path("fixtures/bench/nbody-v-g3.obj")
    body = mir.bodies(corpus.loaded(path), corpus.partitioned(path))[0][1]
    done = algebraic.simplified(body, set(), set())
    stores = [op for block in done.blocks for op in block.ops if op.at in (0x2F0, 0x2F3) and op.stores]
    assert len(stores) == 1
    assert stores[0].stores[0].width == stores[0].args[0].width == 4
    from qbopt.analysis import loops

    found = corpus.loaded(path)
    final = transform.applied(body, found.dgroup, found.calls, blocks=corpus.partitioned(path), found=found)
    counter = stores[0].stores[0].addr
    loop = next(loop for loop in loops.loops(final.blocks, final.entry) if loop.header == 0x2F0)
    assert not any(
        ref.addr == counter
        for block in final.blocks
        if block.at in loop.body
        for op in block.ops
        for ref in (*op.loads, *op.stores)
    )


@pytest.mark.parametrize("mismatch", ["address", "value", "barrier"])
def test_whole_store_requires_adjacent_matching_word_writes(mismatch):
    """NBODY's counter store is not permission to combine unrelated or observable writes."""
    from qbopt.optimize import wholephis
    from qbopt.optimize import wholestores

    path = Path("fixtures/bench/nbody-v-g3.obj")
    body = wholephis.joined(mir.bodies(corpus.loaded(path), corpus.partitioned(path))[0][1])
    header = next(block for block in body.blocks if block.at == 0x2F0)
    low = next(op for op in header.ops if op.at == 0x2F0 and op.stores)
    high = next(op for op in header.ops if op.at == 0x2F3 and op.stores)
    match mismatch:
        case "address":
            ref = high.stores[0]
            changed = replace(high, stores=(replace(ref, addr=ref.addr.plus(2)),))
        case "value":
            changed = replace(high, args=low.args)
        case "barrier":
            changed = replace(high, kind=mir.Kind.CALL)
    body = replace(
        body,
        blocks=tuple(
            replace(block, ops=tuple(changed if op is high else op for op in block.ops)) for block in body.blocks
        ),
    )
    done = wholestores.joined(body)
    assert sum(bool(op.stores) for block in done.blocks for op in block.ops if op.at in (0x2F0, 0x2F3)) == 2


@pytest.mark.parametrize("mismatch", ["edge", "half", "unknown"])
def test_whole_counter_phi_requires_every_matching_edge(mismatch):
    """NBODY's comparison must not combine unrelated words or guess a missing incoming value."""
    from qbopt.optimize import wholephis

    path = Path("fixtures/bench/nbody-v-g3.obj")
    body = mir.bodies(corpus.loaded(path), corpus.partitioned(path))[0][1]
    header = next(block for block in body.blocks if block.at == 0x2F0)
    low = next(phi for phi in header.phis if phi.result.variable == 1)
    high = next(phi for phi in header.phis if phi.result.variable == 3)
    incoming = dict(high.incoming)
    match mismatch:
        case "edge":
            incoming.pop(0x2E3)
        case "half":
            incoming[0x2E3] = low.incoming[0x2E3]
        case "unknown":
            incoming[0xC7] = mir.Value(999999, 0xC7, variable=999999)
    header = replace(header, phis=tuple(replace(phi, incoming=incoming) if phi is high else phi for phi in header.phis))
    body = replace(body, blocks=tuple(header if block.at == header.at else block for block in body.blocks))
    done = wholephis.joined(body)
    assert any(op.at == 0x2FE and op.kind is mir.Kind.CONCAT for block in done.blocks for op in block.ops)


@pytest.mark.parametrize("tag", ["p-g2", "q-O", "v-g3"])
def test_sixty_dimensional_zero_offset_needs_no_pointer_arithmetic(tag):
    """NDMAX printed 11,22 correctly but normalized a pointer advanced by zero bytes."""
    from qbopt import wholeseg
    from qbopt.analysis import consts

    states = []

    def watch(stage, name, state):
        if stage == "mir-widen":
            states.append(state)

    result = wholeseg.emitted(Path(f"fixtures/regressions/ndmax-{tag}.obj").read_bytes(), watch=watch)
    assert result.outcome is wholeseg.Emission.LIR, result.reason
    facts = consts.known(states[0])
    offsets = [op.args[1] for block in states[0].blocks for op in block.ops if op.kind is mir.Kind.PTR_OFFSET]
    assert offsets
    for arg in offsets:
        value = arg if isinstance(arg, mir.Const) else facts.get(arg.value)
        assert value is None or value.n != 0


@pytest.mark.parametrize("tag", ["p-g2", "q-O", "v-g3"])
def test_hotlpx_scales_by_twenty_without_a_second_multiply(tag):
    """HOTLPX's closed-form sum still used IMUL for the constant factor twenty."""
    from iced_x86 import Mnemonic

    from qbopt import wholeseg

    result = wholeseg.emitted(Path(f"fixtures/omf/hotlpx-{tag}.obj").read_bytes())
    assert result.outcome is wholeseg.Emission.LIR, result.reason
    insns = [one.insn for block in corpus.partitioned(result.data) for one in block.insns]
    assert sum(one.mnemonic == Mnemonic.IMUL for one in insns) == 1


@pytest.mark.parametrize("tag", ["p-g2", "q-O", "v-g3"])
def test_spill_folds_closed_loops_to_their_exact_final_constants(tag):
    """SPILL once added 150 then 70 per iteration; it now needs no loop or ADD.

    Requiring the intermediate ``add 220`` became stale when recurrence
    evaluation proved both printed answers. Check the stronger observable code
    shape: the independent answers 2200 and 220 are passed to PRINT directly,
    with no residual addition.
    """
    from iced_x86 import OpKind
    from iced_x86 import Mnemonic

    from qbopt import wholeseg

    result = wholeseg.emitted(Path(f"fixtures/omf/spill-{tag}.obj").read_bytes())
    assert result.outcome is wholeseg.Emission.LIR, result.reason
    instructions = [one.insn for block in corpus.partitioned(result.data) for one in block.insns]
    assert not any(one.mnemonic == Mnemonic.ADD for one in instructions)
    printed = [
        one.immediate(0)
        for one in instructions
        if one.mnemonic == Mnemonic.PUSH
        and one.op0_kind in (OpKind.IMMEDIATE8TO16, OpKind.IMMEDIATE16)
        and one.immediate(0)
    ]
    assert printed == [2200, 220]


@pytest.mark.parametrize("kind", [mir.Kind.ADD, mir.Kind.SUB])
@pytest.mark.parametrize("guard", ["none", "first_flags", "last_flags", "shared", "width", "merge", "memory"])
def test_offset_composition_preserves_modular_values_and_observers(kind, guard):
    """SPILL's combined increment must preserve wraparound and any observed intermediate."""
    from collections import Counter

    source, middle, result = (mir.Value(index, 0) for index in range(1, 4))
    flags = mir.Value(4, 0, flags=True)
    first = mir.Op(
        0,
        ir.Operation.BINARY,
        "add",
        (middle,),
        (source,),
        kind=mir.Kind.ADD,
        args=(mir.Held(source, 2), mir.Const(65530, 2)),
        results=(mir.Held(middle, 2),),
    )
    last = mir.Op(
        1,
        ir.Operation.BINARY,
        "add" if kind is mir.Kind.ADD else "sub",
        (result,),
        (middle,),
        kind=kind,
        args=(mir.Held(middle, 2), mir.Const(20, 2)),
        results=(mir.Held(result, 2),),
    )
    if guard == "first_flags":
        first = replace(first, defines=(middle, flags))
    if guard == "last_flags":
        last = replace(last, defines=(result, flags))
    if guard == "width":
        last = replace(last, results=(mir.Held(result, 4),))
    if guard == "merge":
        first = replace(first, merges={source: middle})
    if guard == "memory":
        first = replace(first, loads=(ir.Mem(None, 2),))
    done = algebraic._offset_chain(last, {middle: first}, {flags}, Counter({middle: 2 if guard == "shared" else 1}))
    if guard != "none":
        assert done == last
        return
    delta = 20 if kind is mir.Kind.ADD else -20
    assert done.args == (mir.Held(source, 2), mir.Const((65530 + delta) & 65535, 2))
    for value in (0, 1, 32767, 32768, 65535):
        assert (((value + 65530) & 65535) + delta) & 65535 == (value + done.args[1].n) & 65535


@pytest.mark.parametrize(
    ("kind", "first", "last", "combined"),
    [
        (mir.Kind.AND, 0xF0F3, 0x3FFF, 0x30F3),
        (mir.Kind.OR, 0xF003, 0x0F30, 0xFF33),
        (mir.Kind.XOR, 0xFFFF, 0x0031, 0xFFCE),
    ],
)
@pytest.mark.parametrize("guard", ["none", "first_flags", "last_flags", "shared", "width", "merge", "memory"])
def test_associative_bitwise_constants_combine_without_losing_observers(
    kind: mir.Kind, first: int, last: int, combined: int, guard: str
) -> None:
    """CRC began with two XOR immediates although their middle value was private.

    Associative bitwise chains may combine constants at an unchanged modular
    width, but not when flags, memory, merges, or the intermediate are visible.
    """
    source, middle, result = (mir.Value(index, 0) for index in range(1, 4))
    flags = mir.Value(4, 0, flags=True)
    first_op = mir.Op(
        0,
        ir.Operation.BINARY,
        kind.value,
        (middle,),
        (source,),
        kind=kind,
        args=(mir.Held(source, 2), mir.Const(first, 2)),
        results=(mir.Held(middle, 2),),
    )
    last_op = mir.Op(
        1,
        ir.Operation.BINARY,
        kind.value,
        (result,),
        (middle,),
        kind=kind,
        args=(mir.Held(middle, 2), mir.Const(last, 2)),
        results=(mir.Held(result, 2),),
    )
    if guard == "first_flags":
        first_op = replace(first_op, defines=(middle, flags))
    elif guard == "last_flags":
        last_op = replace(last_op, defines=(result, flags))
    elif guard == "width":
        last_op = replace(last_op, results=(mir.Held(result, 4),))
    elif guard == "merge":
        last_op = replace(last_op, merges={middle: result})
    elif guard == "memory":
        first_op = replace(first_op, loads=(mir.MemRef(None, 2),))

    uses = Counter({middle: 2 if guard == "shared" else 1})
    changed = algebraic._bitwise_chain(last_op, {middle: first_op}, {flags}, uses)
    if guard not in {"none", "last_flags"}:
        assert changed == last_op
        return
    assert changed.args == (mir.Held(source, 2), mir.Const(combined, 2))
    assert changed.defines == last_op.defines
    for value in (0, 1, 0x7FFF, 0x8000, 0xFFFF):
        if kind is mir.Kind.AND:
            expected = (value & first) & last
            actual = value & combined
        elif kind is mir.Kind.OR:
            expected = (value | first) | last
            actual = value | combined
        else:
            expected = (value ^ first) ^ last
            actual = value ^ combined
        assert expected == actual


@pytest.mark.parametrize("tag", ["p-g2", "q-O", "v-g3"])
def test_addrm_reuses_word_scale_for_long_address(tag):
    """ADDRM rebuilt i*4 after using i*2, paying another copy and a larger shift each iteration."""
    from iced_x86 import Mnemonic

    from qbopt import wholeseg
    from qbopt.objectfile import omf
    from qbopt.frontend import blocks
    from qbopt.objectfile import module

    result = wholeseg.emitted(Path(f"fixtures/omf/addrm-{tag}.obj").read_bytes())
    assert result.outcome is wholeseg.Emission.LIR, result.reason
    found = module.of(omf.parse(result.data))
    shifts = [one.insn for one in blocks.instructions(found) if one.insn.mnemonic == Mnemonic.SHL]
    assert len(shifts) == 1
    assert all(one.immediate(1) == 1 for one in shifts)


@pytest.mark.parametrize("guard", ["none", "first_flags", "last_flags", "shared", "width", "merge"])
def test_scaled_chain_preserves_modular_values_and_observed_intermediates(guard):
    from collections import Counter

    source, middle, result = (mir.Value(index, 0) for index in range(1, 4))
    flags = mir.Value(4, 0, flags=True)
    first = mir.Op(
        0,
        ir.Operation.BINARY,
        "mul",
        (middle,),
        (source,),
        kind=mir.Kind.MUL,
        args=(mir.Held(source, 2), mir.Const(32769, 2)),
        results=(mir.Held(middle, 2),),
    )
    last = mir.Op(
        1,
        ir.Operation.BINARY,
        "shl",
        (result,),
        (middle,),
        kind=mir.Kind.SHL,
        args=(mir.Held(middle, 2), mir.Const(1, 1)),
        results=(mir.Held(result, 2),),
    )
    if guard == "first_flags":
        first = replace(first, defines=(middle, flags))
    if guard == "last_flags":
        last = replace(last, defines=(result, flags))
    if guard == "width":
        last = replace(last, results=(mir.Held(result, 4),))
    if guard == "merge":
        last = replace(last, merges={source: result})
    done = algebraic._scaled_chain(last, {middle: first}, {flags}, Counter({middle: 2 if guard == "shared" else 1}))
    if guard != "none":
        assert done == last
        return
    assert done.kind is mir.Kind.MUL
    assert done.args == (mir.Held(source, 2), mir.Const(2, 2))
    for value in (0, 1, 32767, 32768, 65535):
        assert ((value * 32769 & 65535) << 1) & 65535 == value * 2 & 65535


@pytest.mark.parametrize("guard", ["none", "unused", "flags", "width", "block"])
def test_shared_shift_requires_available_same_width_value(guard):
    """ADDRM address reuse must preserve live flags, widths, and block availability."""
    source, middle, result, observed = (mir.Value(index, 0) for index in range(1, 5))
    flags = mir.Value(5, 0, flags=True)
    first = mir.Op(
        0,
        ir.Operation.BINARY,
        "shl",
        (middle,),
        (source,),
        kind=mir.Kind.SHL,
        args=(mir.Held(source, 2), mir.Const(1, 1)),
        results=(mir.Held(middle, 2),),
    )
    observe = mir.Op(
        1,
        ir.Operation.MOVE,
        "mov",
        (observed,),
        (middle,),
        kind=mir.Kind.COPY,
        args=(mir.Held(middle, 2),),
        results=(mir.Held(observed, 2),),
    )
    width = 4 if guard == "width" else 2
    last = mir.Op(
        2,
        ir.Operation.BINARY,
        "shl",
        (result,),
        (source,),
        kind=mir.Kind.SHL,
        args=(mir.Held(source, width), mir.Const(2, 1)),
        results=(mir.Held(result, width),),
    )
    if guard == "flags":
        last = replace(last, defines=(result, flags))
    prefix = (first,) if guard == "unused" else (first, observe)
    blocks = (mir.MirBlock(0, (), (*prefix, last), ()),)
    if guard == "block":
        blocks = (mir.MirBlock(0, (), prefix, (2,)), mir.MirBlock(2, (), (last,), ()))
    done = algebraic._shared_shifts(mir.MirBody(0, blocks), {flags}).blocks[-1].ops[-1]
    if guard != "none":
        assert done == last
        return
    assert done.args == (mir.Held(middle, 2), mir.Const(1, 1))
    for value in (0, 1, 16383, 16384, 32767, 32768, 65535):
        assert (((value << 1) & 65535) << 1) & 65535 == (value << 2) & 65535


@pytest.mark.parametrize("partial", [False, True])
def test_shared_shift_distinguishes_a_word_tie_from_a_partial_write(partial):
    """ADDRM's word shifts carried an allocator tie that hid a reusable scale."""
    source, middle, result = (mir.Value(index, 0) for index in range(1, 4))
    width = 1 if partial else 2
    first_result = mir.Held(middle, width)
    final_result = mir.Held(result, width)
    first = mir.Op(
        0,
        ir.Operation.BINARY,
        "shl",
        (middle,),
        (source,),
        kind=mir.Kind.SHL,
        args=(mir.Held(source, width), mir.Const(1, 1)),
        results=(first_result,),
        merges={source: middle},
    )
    last = mir.Op(
        1,
        ir.Operation.BINARY,
        "shl",
        (result,),
        (source,),
        kind=mir.Kind.SHL,
        args=(mir.Held(source, width), mir.Const(2, 1)),
        results=(final_result,),
        merges={source: result},
    )
    observe = mir.Op(
        2,
        ir.Operation.MOVE,
        "mov",
        (),
        (middle,),
        kind=mir.Kind.OPAQUE,
        args=(first_result,),
    )

    done = (
        algebraic._shared_shifts(
            mir.MirBody(0, (mir.MirBlock(0, (), (first, observe, last), ()),)),
            set(),
        )
        .blocks[0]
        .ops[-1]
    )

    if partial:
        assert done == last
    else:
        assert done.args == (first_result, mir.Const(1, 1))
        assert done.uses == (middle,)
        assert done.merges == {middle: result}


@pytest.mark.parametrize("tag", ["p-g2", "q-O", "v-g3"])
def test_nested_combines_row_scale_in_emitted_code(tag):
    """NESTED multiplied the row by six, then shifted it again to address word elements."""
    from iced_x86 import Code

    from qbopt import wholeseg
    from qbopt.objectfile import omf
    from qbopt.frontend import blocks
    from qbopt.objectfile import module

    result = wholeseg.emitted(Path(f"fixtures/omf/nested-{tag}.obj").read_bytes())
    assert result.outcome is wholeseg.Emission.LIR, result.reason
    found = module.of(omf.parse(result.data))
    factors = [
        one.insn.immediate8to16 for one in blocks.instructions(found) if one.insn.code == Code.IMUL_R16_RM16_IMM8
    ]
    strides = [one.insn.immediate8to16 for one in blocks.instructions(found) if one.insn.code == Code.ADD_RM16_IMM8]
    assert 12 in factors or 12 in strides
    assert 6 not in factors


@pytest.mark.parametrize(
    ("high_offset", "different_source", "recombined"), [(16, False, True), (0, False, False), (16, True, False)]
)
def test_extracted_halves_recombine_to_the_original_value(high_offset, different_source, recombined) -> None:
    """Nbody split a multiply result and rebuilt it before division, adding stack traffic."""
    source, other, low, high, result = (mir.Value(index, 0) for index in range(1, 6))

    def extract(value, original, offset):
        return mir.Op(
            0,
            mir.Synth.HALF_TO_LOW,
            "extract",
            (value,),
            (original,),
            kind=mir.Kind.EXTRACT,
            args=(mir.Held(original, 4), mir.Const(offset, 4)),
            results=(mir.Held(value, 2),),
        )

    concat = mir.Op(
        1,
        mir.Synth.CONCAT_LOW,
        "concat",
        (result,),
        (high, low),
        kind=mir.Kind.CONCAT,
        args=(mir.Held(high, 2), mir.Held(low, 2)),
        results=(mir.Held(result, 4),),
    )
    body = mir.MirBody(
        0,
        (
            mir.MirBlock(
                0,
                (),
                (extract(low, source, 0), extract(high, other if different_source else source, high_offset), concat),
                (),
            ),
        ),
    )
    done = algebraic.simplified(body, {result}, set()).blocks[0].ops[-1]
    if recombined:
        assert done.kind is mir.Kind.COPY
        assert done.args == (mir.Held(source, 4),)
    else:
        assert done == concat


@pytest.mark.parametrize("mode", ["halves", "ordered", "nonzero", "whole_use"])
def test_joined_halves_are_consumed_as_halves(mode) -> None:
    """A call's DX:AX long was joined only to be stored, tested for zero and
    pushed; lowered, the join is `push dx; push ax; pop eax`."""
    from qbopt.cfront.raise_hir import Addr
    from qbopt.cfront.raise_hir import Space

    low, high, whole, flags, other = (mir.Value(index, 0) for index in range(1, 6))
    flags = mir.Value(4, 0, True)
    K = mir.Kind
    ref = mir.MemRef(Addr(Space.SEGMENT, 8, 5), 4, space=Space.SEGMENT)
    ops = [
        mir.Op(
            1, ir.Operation.NOTHING, "", (low, high), (), kind=K.CALL, results=(mir.Held(low, 2), mir.Held(high, 2))
        ),
        mir.Op(
            2,
            ir.Operation.NOTHING,
            "",
            (whole,),
            (high, low),
            kind=K.CONCAT,
            args=(mir.Held(high, 2), mir.Held(low, 2)),
            results=(mir.Held(whole, 4),),
        ),
        mir.Op(
            3,
            ir.Operation.NOTHING,
            "",
            (),
            (whole,),
            kind=K.STORE,
            args=(mir.Held(whole, 4),),
            results=(mir.Cell(ref),),
            stores=(ref,),
        ),
        mir.Op(4, ir.Operation.NOTHING, "", (), (whole,), kind=K.ARG, args=(mir.Held(whole, 4),)),
        mir.Op(
            5,
            ir.Operation.NOTHING,
            "",
            (flags,),
            (whole,),
            kind=K.SUB,
            args=(mir.Held(whole, 4), mir.Const(5 if mode == "nonzero" else 0, 4)),
        ),
        mir.Op(
            6, ir.Operation.NOTHING, "", (), (flags,), kind=K.BRANCH, test=K.LT if mode == "ordered" else K.NE, target=0
        ),
    ]
    if mode == "whole_use":
        ops.insert(
            5,
            mir.Op(
                5,
                ir.Operation.NOTHING,
                "",
                (other,),
                (whole,),
                kind=K.SHR,
                args=(mir.Held(whole, 4), mir.Const(1, 1)),
                results=(mir.Held(other, 4),),
            ),
        )
    body = mir.MirBody(0, (mir.MirBlock(0, (), tuple(ops), ()),))
    done = algebraic.simplified(body, {other}, set()).blocks[0].ops
    readers = [op for op in done if whole in op.uses]
    if mode != "halves":
        assert len(readers) == len(ops) - 3
        return
    assert not readers
    stores = [(op.args, op.stores[0].addr, op.stores[0].width) for op in done if op.kind is K.STORE]
    assert stores == [((mir.Held(low, 2),), ref.addr, 2), ((mir.Held(high, 2),), ref.addr.plus(2), 2)]
    assert [op.args for op in done if op.kind is K.ARG] == [(mir.Held(high, 2),), (mir.Held(low, 2),)]
    test = next(op for op in done if flags in op.defines)
    assert test.kind is K.OR and set(test.args) == {mir.Held(high, 2), mir.Held(low, 2)}


def test_nbody_multiply_value_survives_into_scaled_division() -> None:
    """Nbody rebuilt the product from two halves before /512, paying a redundant stack round trip."""
    path = Path("fixtures/regressions/nbody-stack-p-g2.obj")
    found = corpus.loaded(path)
    blocks = corpus.partitioned(path)
    body = mir.bodies(found, blocks)[0][1]
    done = transform.applied(body, found.dgroup, found.calls, blocks=blocks, found=found)
    ops = [op for block in done.blocks for op in block.ops]
    product = next(op for op in ops if op.at == 0x1CD and op.kind is mir.Kind.MUL).results[0]
    sign = next(op for op in ops if op.at == 0x1D4 and op.kind is mir.Kind.SAR)
    assert sign.args[0] == product
    assert not any(op.at == 0x1D4 and op.kind is mir.Kind.CONCAT for op in ops)


@pytest.mark.parametrize("mode", ["copy", "width_change", "cycle"])
def test_recombination_follows_only_exact_word_copies(mode):
    """Whole-value recovery through copies must not accept a different width or a cyclic definition."""
    source, high, low, copied = (mir.Value(index, 0) for index in range(1, 5))
    definitions = {}
    for value, offset in ((high, 16), (low, 0)):
        definitions[value] = mir.Op(
            0,
            mir.Synth.HALF_TO_LOW,
            "extract",
            (value,),
            (source,),
            kind=mir.Kind.EXTRACT,
            args=(mir.Held(source, 4), mir.Const(offset, 4)),
            results=(mir.Held(value, 2),),
        )
    incoming = copied if mode == "cycle" else low
    definitions[copied] = mir.Op(
        1,
        ir.Operation.MOVE,
        "mov",
        (copied,),
        (incoming,),
        kind=mir.Kind.COPY,
        args=(mir.Held(incoming, 4 if mode == "width_change" else 2),),
        results=(mir.Held(copied, 2),),
    )
    answer = mir.extracted_whole(mir.Held(high, 2), mir.Held(copied, 2), definitions)
    assert answer == (mir.Held(source, 4) if mode == "copy" else None)


@pytest.mark.parametrize("mode", ["same", "sibling", "different", "width_change", "barrier"])
def test_signed_recombination_compares_copy_sources_symmetrically(mode):
    """PITSNAP's low word was a copy also used by sign extension; chasing only one lost the whole."""
    root, copied, sibling, whole, high = (mir.Value(index, 0) for index in range(1, 6))
    definitions = {}
    for value in (copied, sibling):
        definitions[value] = mir.Op(
            0,
            ir.Operation.MOVE,
            "mov",
            (value,),
            (root,),
            kind=mir.Kind.COPY,
            args=(mir.Held(root, 2),),
            results=(mir.Held(value, 2),),
        )
    if mode == "width_change":
        definitions[copied] = replace(definitions[copied], args=(mir.Held(root, 4),))
    if mode == "barrier":
        definitions[copied] = replace(definitions[copied], op=ir.Operation.BARRIER)
    definitions[whole] = mir.Op(
        1,
        ir.Operation.EXTEND,
        "sign_extend",
        (whole,),
        (copied,),
        kind=mir.Kind.SIGN_EXTEND,
        args=(mir.Held(copied, 2),),
        results=(mir.Held(whole, 4),),
    )
    definitions[high] = mir.Op(
        1,
        mir.Synth.HALF_TO_LOW,
        "extract",
        (high,),
        (whole,),
        kind=mir.Kind.EXTRACT,
        args=(mir.Held(whole, 4), mir.Const(16, 4)),
        results=(mir.Held(high, 2),),
    )
    low = copied if mode == "same" else sibling
    if mode == "different":
        low = mir.Value(99, 0)
    answer = mir.extracted_whole(mir.Held(high, 2), mir.Held(low, 2), definitions)
    assert answer == (mir.Held(whole, 4) if mode in {"same", "sibling"} else None)


def test_nbody_address_shifts_combine_without_an_extra_counter() -> None:
    """Nbody computed other*4 with two shifts; an extra induction counter increased spill cost."""
    path = Path("fixtures/regressions/nbody-stack-p-g2.obj")
    found = corpus.loaded(path)
    body = mir.bodies(found, corpus.partitioned(path))[0][1]
    done = transform.Algebraic().transform(body)
    shift = next(op for block in done.blocks for op in block.ops if op.at == 0x11B)
    assert shift.kind is mir.Kind.SHL and shift.args[1] == mir.Const(2, 1)


def test_nbody_damping_keeps_negation_whole():
    """NBODY split both velocity negations into words, emitting push/pop traffic and paired stores."""
    from iced_x86 import Mnemonic
    from iced_x86 import Register

    from qbopt import wholeseg
    from qbopt.objectfile import omf
    from qbopt.frontend import blocks
    from qbopt.objectfile import module

    path = Path("fixtures/regressions/nbody-stack-p-g2.obj")
    body = mir.bodies(corpus.loaded(path), corpus.partitioned(path))[0][1]
    negated = [op for block in body.blocks for op in block.ops if op.kind is mir.Kind.NEG]
    assert len(negated) >= 2
    assert all(op.results[0].width == 4 for op in negated)
    result = wholeseg.emitted(path.read_bytes())
    assert result.outcome is wholeseg.Emission.LIR, result.reason
    found = module.of(omf.parse(result.data))
    negations = [one.insn for one in blocks.instructions(found) if one.insn.mnemonic == Mnemonic.NEG]
    assert all(
        one.op0_register in (Register.EAX, Register.EBX, Register.ECX, Register.EDX, Register.ESI, Register.EDI)
        for one in negations
    )


def test_nbody_damping_reverses_subtraction_without_negation():
    """NBODY paid for -(quotient-velocity) instead of one velocity-quotient subtraction."""
    from iced_x86 import Mnemonic

    from qbopt import wholeseg
    from qbopt.objectfile import omf
    from qbopt.frontend import blocks
    from qbopt.objectfile import module

    result = wholeseg.emitted(Path("fixtures/regressions/nbody-stack-p-g2.obj").read_bytes())
    assert result.outcome is wholeseg.Emission.LIR, result.reason
    found = module.of(omf.parse(result.data))
    assert not any(one.insn.mnemonic == Mnemonic.NEG for one in blocks.instructions(found))


@pytest.mark.parametrize("guard", ["none", "shared", "sub_flags", "neg_flags", "width", "merge"])
def test_reversed_difference_preserves_observed_values_and_flags(guard):
    """NBODY's negated subtraction can be reversed only without duplicating work or changing flags."""
    from collections import Counter

    left, right, middle, result = (mir.Value(index, 0) for index in range(1, 5))
    flags = mir.Value(5, 0, flags=True)
    difference = mir.Op(
        0,
        ir.Operation.BINARY,
        "sub",
        (middle,),
        (left, right),
        kind=mir.Kind.SUB,
        args=(mir.Held(left, 4), mir.Held(right, 4)),
        results=(mir.Held(middle, 4),),
    )
    negate = mir.Op(
        1,
        ir.Operation.UNARY,
        "neg",
        (result,),
        (middle,),
        kind=mir.Kind.NEG,
        args=(mir.Held(middle, 4),),
        results=(mir.Held(result, 4),),
    )
    match guard:
        case "sub_flags":
            difference = replace(difference, defines=(middle, flags))
        case "neg_flags":
            negate = replace(negate, defines=(result, flags))
        case "width":
            negate = replace(negate, results=(mir.Held(result, 2),))
        case "merge":
            difference = replace(difference, merges={left: middle})
    done = algebraic._negated_difference(
        negate, {middle: difference}, {flags}, Counter({middle: 2 if guard == "shared" else 1})
    )
    if guard != "none":
        assert done == negate
        return
    assert done.kind is mir.Kind.SUB and done.args == tuple(reversed(difference.args))
    for first in (0, 1, 0x7FFFFFFF, 0x80000000, 0xFFFFFFFF):
        for second in (0, 1, 0x7FFFFFFF, 0x80000000, 0xFFFFFFFF):
            assert (-((first - second) & 0xFFFFFFFF)) & 0xFFFFFFFF == (second - first) & 0xFFFFFFFF


@pytest.mark.parametrize(
    ("first_count", "last_count", "live_flags", "uses"),
    [(15, 1, False, 1), (32, 1, False, 1), (1, 1, True, 1), (1, 1, False, 2)],
)
def test_shift_combination_preserves_count_flag_and_use_boundaries(first_count, last_count, live_flags, uses) -> None:
    source, middle, result = (mir.Value(index, 0) for index in range(1, 4))
    flags = mir.Value(4, 0, flags=True)
    first = mir.Op(
        0,
        ir.Operation.BINARY,
        "shl",
        (middle,),
        (source,),
        kind=mir.Kind.SHL,
        args=(mir.Held(source, 2), mir.Const(first_count, 1)),
        results=(mir.Held(middle, 2),),
    )
    last = mir.Op(
        1,
        ir.Operation.BINARY,
        "shl",
        (result, flags),
        (middle,),
        kind=mir.Kind.SHL,
        args=(mir.Held(middle, 2), mir.Const(last_count, 1)),
        results=(mir.Held(result, 2),),
    )
    assert (
        algebraic._shift_chain(
            last,
            {middle: first},
            {flags} if live_flags else set(),
            Counter({middle: uses}),
        )
        == last
    )


@pytest.mark.parametrize(("width", "divisor"), [(4, 2), (4, 16), (4, 512), (4, 262144), (2, 2), (2, 16), (2, 16384)])
@pytest.mark.parametrize("immediate", [False, True])
def test_signed_power_division_preserves_quotient_and_remainder(width: int, divisor: int, immediate: bool) -> None:
    """Nbody paid for IDIV by fixed scales; negative deltas require truncation, not flooring.
    A word was left to IDIV: shellsort's `gap /= 2`."""
    bits = 8 * width
    source, constant, quotient, remainder = (mir.Value(index, 0) for index in range(1, 5))
    copy = mir.Op(
        0,
        ir.Operation.MOVE,
        "mov",
        (constant,),
        (),
        kind=mir.Kind.COPY,
        args=(mir.Const(divisor, width),),
        results=(mir.Held(constant, width),),
    )
    divide = mir.Op(
        1,
        ir.Operation.DIVIDE,
        "idiv",
        (quotient, remainder),
        (source, constant),
        kind=mir.Kind.DIVMOD,
        args=(mir.Held(source, width), mir.Held(constant, width)),
        results=(mir.Held(quotient, width), mir.Held(remainder, width)),
    )
    body = mir.MirBody(0, (mir.MirBlock(0, (), (copy, divide), ()),))
    if immediate:
        divide = replace(divide, args=(mir.Held(source, width), mir.Const(divisor, width)), uses=(source,))
        body = replace(body, blocks=(replace(body.blocks[0], ops=(divide,)),))
    done = algebraic._divisions(body)
    assert all(op.kind is not mir.Kind.DIVMOD for op in done.blocks[0].ops)
    lowest, highest = -(1 << (bits - 1)), (1 << (bits - 1)) - 1
    for number in [lowest, -divisor - 1, -divisor, -divisor + 1, -1, 0, 1, divisor - 1, divisor, highest]:
        values = {source: number}
        for op in done.blocks[0].ops:
            args = [arg.n if isinstance(arg, mir.Const) else values[arg.value] for arg in op.args]
            match op.kind:
                case mir.Kind.COPY:
                    answer = args[0]
                case mir.Kind.SAR:
                    answer = args[0] >> args[1]
                case mir.Kind.SHR:
                    answer = (args[0] & ((1 << bits) - 1)) >> args[1]
                case mir.Kind.SHL:
                    answer = args[0] << args[1]
                case mir.Kind.AND:
                    answer = args[0] & args[1]
                case mir.Kind.ADD:
                    answer = args[0] + args[1]
                case mir.Kind.SUB:
                    answer = args[0] - args[1]
                case _:
                    pytest.fail(str(op.kind))
            values[op.results[0].value] = ((answer & ((1 << bits) - 1)) ^ (1 << (bits - 1))) - (1 << (bits - 1))
        expected = abs(number) // divisor * (-1 if number < 0 else 1)
        assert values[quotient] == expected
        assert values[remainder] == number - expected * divisor


@pytest.mark.parametrize(
    ("high", "low", "answer"), [(4, 0, 262144), (0, 512, 512), (-1, -1, 0xFFFFFFFF), (1, -1, 0x1FFFF)]
)
def test_constant_word_concatenation(high: int, low: int, answer: int) -> None:
    result = mir.Value(1, 0)
    op = mir.Op(
        0,
        mir.Synth.CONCAT_LOW,
        "concat",
        (result,),
        (),
        kind=mir.Kind.CONCAT,
        args=(mir.Const(high, 2), mir.Const(low, 2)),
        results=(mir.Held(result, 4),),
    )
    body = mir.MirBody(0, (mir.MirBlock(0, (), (op,), ()),))
    done = algebraic.simplified(body, {result}, set()).blocks[0].ops[0]
    assert done.kind is mir.Kind.COPY
    assert done.args == (mir.Const(answer, 4),)


@pytest.mark.parametrize("width", [2, 4])
@pytest.mark.parametrize(
    ("kind", "constant", "answer"),
    [
        (mir.Kind.ADD, 0, None),
        (mir.Kind.SUB, 0, None),
        (mir.Kind.MUL, 1, None),
        (mir.Kind.OR, 0, None),
        (mir.Kind.XOR, 0, None),
        (mir.Kind.AND, -1, None),
        (mir.Kind.AND, 0, 0),
        (mir.Kind.MUL, 0, 0),
        (mir.Kind.OR, -1, -1),
        (mir.Kind.SHL, 0, None),
        (mir.Kind.SHR, 0, None),
        (mir.Kind.SAR, 0, None),
    ],
)
def test_integer_identities(kind: mir.Kind, constant: int, answer: int | None, width: int) -> None:
    source, result = mir.Value(10, 0), mir.Value(11, 1)
    count_width = 1 if kind in (mir.Kind.SHL, mir.Kind.SHR, mir.Kind.SAR) else width
    op = mir.Op(
        1,
        ir.Operation.BINARY,
        "",
        (result,),
        (source,),
        kind=kind,
        args=(mir.Held(source, width), mir.Const(constant, count_width)),
        results=(mir.Held(result, width),),
    )
    changed = algebraic._simplified(op, {result}, set())
    assert changed.kind is mir.Kind.COPY
    expected = mir.Held(source, width) if answer is None else mir.Const(answer & ((1 << (width * 8)) - 1), width)
    assert changed.args == (expected,)
    assert changed.defines == (result,)
    if width == 2:
        assert algebraic._simplified(op, {result}, {result}) == op
    flags = mir.Value(12, 1, flags=True)
    observed_flags = replace(op, defines=(result, flags))
    assert algebraic._simplified(observed_flags, {result, flags}, set()) == observed_flags


def test_algebraic_pass_runs_without_constant_propagation_facts() -> None:
    source, result = mir.Value(10, 0), mir.Value(11, 1)
    op = mir.Op(
        1,
        ir.Operation.BINARY,
        "add",
        (result,),
        (source,),
        kind=mir.Kind.ADD,
        args=(mir.Held(source, 2), mir.Const(0, 2)),
        results=(mir.Held(result, 2),),
    )
    use = mir.Op(2, ir.Operation.PUSH, "push", (), (result,), kind=mir.Kind.ARG, args=(mir.Held(result, 2),))
    body = mir.MirBody(0, (mir.MirBlock(0, (), (op, use), ()),))
    assert transform.Algebraic().transform(body).blocks[0].ops[0].kind is mir.Kind.COPY


@pytest.mark.parametrize("tag", ["p-g2", "q-O", "v-g3"])
def test_harr_only_needs_the_low_product(tag: str) -> None:
    """HARR paid for a widening product although its high answer and flags were unused."""
    obj = Path("fixtures/omf") / f"harr-{tag}.obj"
    found = corpus.loaded(obj)
    assert found is not None
    before = mir.bodies(found, corpus.partitioned(obj))[0][1]
    after = transform.Algebraic().transform(before)
    products = [op for block in after.blocks for op in block.ops if op.kind is mir.Kind.MUL]
    assert products and all(len(op.results) == 1 for op in products)


@pytest.mark.parametrize("observed", ["high", "flags", "upper", "none"])
def test_product_projection_retains_observed_outputs(observed: str) -> None:
    source, low, high, flags = mir.Value(1, 0), mir.Value(2, 1), mir.Value(3, 1), mir.Value(4, 1, flags=True)
    op = mir.Op(
        1,
        ir.Operation.MULTIPLY,
        "imul",
        (flags, low, high),
        (source,),
        kind=mir.Kind.MUL,
        args=(mir.Held(source, 2), mir.Const(20, 2)),
        results=(mir.Held(low, 2), mir.Held(high, 2)),
        merges={source: high},
    )
    wanted = {low} | ({high} if observed == "high" else {flags} if observed == "flags" else set())
    changed = algebraic._product(op, wanted, {low} if observed == "upper" else set())
    if observed == "none":
        assert changed.results == (mir.Held(low, 2),)
        assert changed.defines == (low,)
        assert changed.uses == (source,)
    else:
        assert changed == op


def test_dead_phis_do_not_keep_matrix_product_halves() -> None:
    """Matrix retained widening multiplies solely for unused loop phi results."""
    obj = Path("fixtures/omf/matrix-p-g2.obj")
    found = corpus.loaded(obj)
    assert found is not None
    blocks = corpus.partitioned(obj)
    body = mir.bodies(found, blocks)[0][1]
    after = transform.applied(body, found.dgroup, found.calls, blocks=blocks, found=found, strength_=False)
    products = [op for block in after.blocks for op in block.ops if op.kind is mir.Kind.MUL]
    assert products and all(len(op.results) == 1 for op in products)


def test_a_symbol_plus_zero_is_the_symbol() -> None:
    """`0 + offset flags`, a fill's first cell, stayed an add: only numbers and
    values were operands of an identity, and it was spilled as a variable."""
    from qbopt.objectfile.module import Space

    symbol = mir.Symbol(Space.SEGMENT, 2, 0, 2)
    result = mir.Value(1, 0)
    add = mir.Op(
        0,
        ir.Operation.BINARY,
        "add",
        (result,),
        (),
        kind=mir.Kind.ADD,
        args=(mir.Const(0, 2), symbol),
        results=(mir.Held(result, 2),),
    )
    done = algebraic._simplified(add, set(), set())
    assert (done.kind, done.args) == (mir.Kind.COPY, (symbol,))


def test_reextending_an_already_zero_extended_low_byte_is_a_copy() -> None:
    """C CRC32 emitted `movzx cx,al; movzx cx,cl` while filling its buffer.

    The first value already has zeroes above the byte. Viewing that same
    value through its low byte and extending it to the same width changes
    nothing, so the allocator should see a copy it can coalesce.
    """
    source, middle, result = (mir.Value(index, 0) for index in range(1, 4))
    first = mir.Op(
        1,
        ir.Operation.EXTEND,
        "",
        (middle,),
        (source,),
        kind=mir.Kind.ZERO_EXTEND,
        args=(mir.Held(source, 1),),
        results=(mir.Held(middle, 2),),
    )
    second = mir.Op(
        2,
        ir.Operation.EXTEND,
        "",
        (result,),
        (middle,),
        kind=mir.Kind.ZERO_EXTEND,
        args=(mir.Held(middle, 1),),
        results=(mir.Held(result, 2),),
    )
    use = mir.Op(3, ir.Operation.PUSH, "", (), (result,), kind=mir.Kind.ARG, args=(mir.Held(result, 1),))
    body = mir.MirBody(0, (mir.MirBlock(0, (), (first, second, use), ()),))

    done = algebraic.simplified(body, set(), set())
    changed = done.blocks[0].ops[1]

    assert changed.kind is mir.Kind.COPY
    assert changed.args == (mir.Held(middle, 2),)


@pytest.mark.parametrize("guard", ["signedness", "discarded_bits", "unknown_upper", "extra_result"])
def test_redundant_extension_requires_every_output_bit_to_be_known(guard: str) -> None:
    source, middle, result, flags = (mir.Value(index, 0, flags=index == 4) for index in range(1, 5))
    first = mir.Op(
        1,
        ir.Operation.EXTEND,
        "",
        (middle,),
        (source,),
        kind=mir.Kind.ZERO_EXTEND,
        args=(mir.Held(source, 1),),
        results=(mir.Held(middle, 2),),
    )
    second = mir.Op(
        2,
        ir.Operation.EXTEND,
        "",
        (result,),
        (middle,),
        kind=mir.Kind.ZERO_EXTEND,
        args=(mir.Held(middle, 1),),
        results=(mir.Held(result, 2),),
    )
    match guard:
        case "signedness":
            second = replace(second, kind=mir.Kind.SIGN_EXTEND)
        case "discarded_bits":
            first = replace(first, args=(mir.Held(source, 2),))
        case "unknown_upper":
            second = replace(second, results=(mir.Held(result, 4),))
        case "extra_result":
            second = replace(second, defines=(result, flags))

    assert algebraic._redundant_extension(second, {middle: first}) == second


def test_subtracting_from_a_copied_zero_is_negation() -> None:
    """C CRC32 copied an invariant zero before `0 - (crc & 1)` in every bit iteration."""
    zero, source, result, flags = (mir.Value(index, 0, flags=index == 4) for index in range(1, 5))
    constant = mir.Op(
        1, ir.Operation.MOVE, "", (zero,), (), kind=mir.Kind.COPY, args=(mir.Const(0, 4),), results=(mir.Held(zero, 4),)
    )
    subtract = mir.Op(
        2,
        ir.Operation.BINARY,
        "",
        (result, flags),
        (zero, source),
        kind=mir.Kind.SUB,
        args=(mir.Held(zero, 4), mir.Held(source, 4)),
        results=(mir.Held(result, 4),),
    )

    changed = algebraic._zero_difference(subtract, {zero: constant})

    assert changed.kind is mir.Kind.NEG
    assert changed.args == (mir.Held(source, 4),)
    assert changed.defines == (result, flags)
    assert changed.uses == (source,)


@pytest.mark.parametrize("guard", ["nonzero", "width", "effect", "extra_result", "untracked_use", "cycle"])
def test_zero_difference_requires_a_complete_pure_value(guard: str) -> None:
    zero, source, result, extra = (mir.Value(index, 0) for index in range(1, 5))
    constant = mir.Op(
        1, ir.Operation.MOVE, "", (zero,), (), kind=mir.Kind.COPY, args=(mir.Const(0, 4),), results=(mir.Held(zero, 4),)
    )
    subtract = mir.Op(
        2,
        ir.Operation.BINARY,
        "",
        (result,),
        (zero, source),
        kind=mir.Kind.SUB,
        args=(mir.Held(zero, 4), mir.Held(source, 4)),
        results=(mir.Held(result, 4),),
    )
    match guard:
        case "nonzero":
            constant = replace(constant, args=(mir.Const(1, 4),))
        case "width":
            subtract = replace(subtract, results=(mir.Held(result, 2),))
        case "effect":
            constant = replace(constant, op=ir.Operation.BARRIER)
        case "extra_result":
            subtract = replace(subtract, defines=(result, extra))
        case "untracked_use":
            subtract = replace(subtract, uses=(source,))
        case "cycle":
            constant = replace(constant, args=(mir.Held(zero, 4),), uses=(zero,))

    assert algebraic._zero_difference(subtract, {zero: constant}) == subtract
