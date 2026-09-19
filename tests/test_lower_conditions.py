"""A branch must consume its own comparison, not inserted arithmetic's flags."""

from dataclasses import replace

import pytest

from qbopt.model import ir
from qbopt.model import mir
from qbopt.backend import lower


@pytest.mark.parametrize(
    "kind,mnemonic",
    [
        (mir.Kind.SHL, "SHL"),
        (mir.Kind.SHR, "SHR"),
        (mir.Kind.SAR, "SAR"),
    ],
)
@pytest.mark.parametrize("width", [2, 4])
def test_unnamed_mir_shift_emits_machine_instruction(kind, mnemonic, width):
    """D_SURF refused 183a: hoisted cidx << 2 reached selection with an empty mnemonic."""
    from iced_x86 import Decoder
    from iced_x86 import Mnemonic
    from iced_x86 import Register

    from qbopt.backend import select

    value = mir.Value(1, 0x183A)
    op = mir.Op(
        0x183A,
        ir.Operation.BINARY,
        "",
        (value,),
        (value,),
        kind=kind,
        args=(mir.Held(value, width), mir.Const(2, 1)),
        results=(mir.Held(value, width),),
    )
    what = lower.semantics(op, place=lower.as_a_value)
    register = Register.AX if width == 2 else Register.EAX
    emitted = select.emit(what, held={value.id: register})
    assert emitted is not None
    decoded = list(Decoder(16, emitted.code))
    assert len(decoded) == 1
    assert decoded[0].mnemonic == getattr(Mnemonic, mnemonic)
    assert decoded[0].op0_register == register
    assert decoded[0].immediate(1) == 2


def test_semantic_compare_gets_its_machine_name_at_lowering() -> None:
    """The QB frontend already classified comparisons as COMPARE operations.

    That semantic classification must not make lowering mistake the otherwise
    unnamed MIR operation for a machine instruction.  Leaving its mnemonic
    empty prevented common post-allocation compare folds from recognizing it.
    """
    value = mir.Value(1, 0)
    flags = mir.Value(2, 0, flags=True)
    compare = mir.Op(
        0,
        ir.Operation.COMPARE,
        "",
        (flags,),
        (value,),
        kind=mir.Kind.SUB,
        args=(mir.Held(value, 4), mir.Const(0, 4)),
    )
    body = mir.MirBody(0, (mir.MirBlock(0, (), (compare,), ()),))

    (named,) = lower.named(body).blocks[0].ops

    assert (named.op, named.name) == (ir.Operation.COMPARE, "cmp")


@pytest.mark.parametrize("width", [2, 4])
def test_dead_and_result_uses_test_without_a_destination(width):
    """IVWORD emitted mov cx,bx / and cx,bx although only the condition was consumed."""
    source, result = mir.Value(1, 0), mir.Value(2, 0)
    condition = mir.Value(3, 0, flags=True)
    op = mir.Op(
        0,
        ir.Operation.BINARY,
        "and",
        (result, condition),
        (source,),
        kind=mir.Kind.AND,
        args=(mir.Held(source, width),) * 2,
        results=(mir.Held(result, width),),
    )
    built = mir.MirBody(0, (mir.MirBlock(0, (), (op,), ()),))
    emitted = lower.Lowering(built, {source.id, condition.id}, {}, (), {}).expand(op)
    assert len(emitted) == 1
    assert emitted[0].what == ir.Semantics(ir.Operation.COMPARE, "test", (), (ir.Held(source.id, width),) * 2)
    assert emitted[0].defines == ()


@pytest.mark.parametrize("observation", ["read", "exit", "merge"])
def test_and_keeps_an_observed_or_partial_result(observation):
    source, result = mir.Value(1, 0), mir.Value(2, 0)
    condition = mir.Value(3, 0, flags=True)
    op = mir.Op(
        0,
        ir.Operation.BINARY,
        "and",
        (result, condition),
        (source,),
        kind=mir.Kind.AND,
        args=(mir.Held(source, 2),) * 2,
        results=(mir.Held(result, 2),),
        merges={source: result} if observation == "merge" else {},
    )
    returned = mir.Op(
        1,
        ir.Operation.RETURN,
        "ret",
        (),
        (result,),
        kind=mir.Kind.RETURN,
        args=(mir.Held(result, 2),),
        exits=(result,),
    )
    operations = (op, returned) if observation == "exit" else (op,)
    built = mir.MirBody(0, (mir.MirBlock(0, (), operations, ()),))
    emitted = lower.Lowering(built, {result.id} if observation == "read" else set(), {}, (), {}).expand(op)
    assert emitted[0].what.name == "and"
    assert emitted[0].defines == (result.id,)


def body() -> mir.MirBody:
    value = mir.Value(1, 0)
    condition = mir.Value(2, 0, flags=True)
    updated = mir.Value(3, 0)
    compare = mir.Op(
        0,
        ir.Operation.COMPARE,
        "cmp",
        (condition,),
        (value,),
        kind=mir.Kind.SUB,
        args=(mir.Held(value, 2), mir.Const(19, 2)),
    )
    increment = mir.Op(
        3,
        ir.Operation.BINARY,
        "",
        (updated,),
        (value,),
        kind=mir.Kind.ADD,
        args=(mir.Held(value, 2), mir.Const(42, 2)),
        results=(mir.Held(updated, 2),),
    )
    branch = mir.Op(6, ir.Operation.BRANCH, "jle", (), (condition,), kind=mir.Kind.BRANCH, target=0)
    return mir.MirBody(0, (mir.MirBlock(0, (), (compare, increment, branch), (0,)),))


def test_inserted_stride_cannot_replace_the_branch_condition() -> None:
    """A stride add between cmp and jle branches on the stride's flags instead of the loop bound."""
    built = body()
    result = lower.lowered("loop", built, {}, (), {})
    assert [one.op.at for one in result.blocks[0].insns] == [3, 0, 6]
    assert [one.at for one in built.blocks[0].ops] == [0, 3, 6]


@pytest.mark.parametrize("consumed", [False, True])
def test_dead_exit_condition_does_not_block_inserted_arithmetic(consumed):
    """Qrender V_UPDATE_CAMERA refused an address add because an unused flags phi looked live."""
    built = body()
    compare, increment, branch = built.blocks[0].ops
    condition = compare.defines[0]
    merged = mir.Value(9, 10, flags=True)
    exit_ops = (replace(branch, at=10, uses=(merged,)),) if consumed else ()
    built = replace(
        built,
        blocks=(
            replace(built.blocks[0], ops=(compare, increment), succ=(10,)),
            mir.MirBlock(10, (mir.Phi(merged, {0: condition}),), exit_ops, ()),
        ),
    )
    if consumed:
        with pytest.raises(lower.Unlowered, match="live condition"):
            lower.lowered("loop", built, {}, (), {})
    else:
        result = lower.lowered("loop", built, {}, (), {})
        assert any(one.what and one.what.name == "add" for one in result.blocks[0].insns)


@pytest.mark.parametrize("reason", ["memory", "second_reader", "data_result"])
def test_condition_scheduling_does_not_move_effects_or_other_results(reason: str) -> None:
    built = body()
    compare, increment, branch = built.blocks[0].ops
    if reason == "memory":
        from qbopt.objectfile.module import Addr
        from qbopt.objectfile.module import Space

        cell = mir.MemRef(Addr(Space.SEGMENT, 0, 1), 2)
        compare = replace(compare, loads=(cell,), args=(mir.Cell(cell), mir.Const(19, 2)))
    elif reason == "second_reader":
        increment = replace(increment, uses=(*increment.uses, compare.defines[0]))
    else:
        compare = replace(compare, defines=(*compare.defines, mir.Value(4, 0)))
    built = replace(built, blocks=(replace(built.blocks[0], ops=(compare, increment, branch)),))
    with pytest.raises(lower.Unlowered, match="crosses a live condition"):
        lower.lowered("loop", built, {}, (), {})


@pytest.mark.parametrize(
    "test,name",
    [
        (mir.Kind.EQ, "je"),
        (mir.Kind.NE, "jne"),
        (mir.Kind.LT, "jl"),
        (mir.Kind.LE, "jle"),
        (mir.Kind.GT, "jg"),
        (mir.Kind.GE, "jge"),
        (mir.Kind.BELOW, "jb"),
        (mir.Kind.BELOW_EQ, "jbe"),
        (mir.Kind.ABOVE, "ja"),
        (mir.Kind.ABOVE_EQ, "jae"),
    ],
)
def test_cloned_operandless_branch_selects_its_semantic_condition(test, name):
    """Peeled IVARM refused at 0x73: its cloned conditional jump was carried without semantics."""
    branch = mir.Op(
        0x73, ir.Operation.BRANCH, "", (), (), kind=mir.Kind.BRANCH, test=test, target=0x3100000001, raised=None
    )
    expected = ir.Semantics(ir.Operation.BRANCH, name, (), (), branch.target)
    assert lower.semantics(branch, place=lower.as_a_value) == expected
