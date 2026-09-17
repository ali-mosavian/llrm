from pathlib import Path
from dataclasses import replace

import pytest

import corpus
from qbopt import wholeseg
from qbopt.model import ir
from qbopt.model import mir
from qbopt.analysis import loops
from qbopt.analysis import consts
from qbopt.frontend import blocks
from qbopt.optimize import transform
from qbopt.frontend import raising_dispatch


@pytest.mark.parametrize("tag", ["q-O", "p-g2", "v-g2", "v-g3"])
def test_real_dispatch_has_an_explicit_error_guard(fixtures: Path, tag: str) -> None:
    source = fixtures / f"jumps-{tag}.obj"
    found = corpus.loaded(source)
    assert found is not None
    bodies = mir.bodies(found, corpus.partitioned(source))
    body = bodies[0][1]
    normal = next(block for block in body.blocks if block.ops and block.ops[-1].kind is mir.Kind.SWITCH)
    dispatch = normal.ops[-1]
    guard_block = next(block for block in body.blocks if normal.at in block.succ)
    compare, guard = guard_block.ops[-2:]
    assert guard.test is mir.Kind.ABOVE
    assert compare.args == (dispatch.args[0], mir.Const(255, 2))
    assert guard.target is not None
    error = body.block(guard.target)
    assert error is not None
    assert found.calls[error.ops[-1].at] == "B$OGTA"
    assert error.ops[-1].kind is mir.Kind.CALL
    assert tuple(number for number, _ in dispatch.cases) == (1, 2, 3)
    assert {dispatch.target, *(target for _, target in dispatch.cases)} == set(normal.succ)
    predecessors = loops.predecessors(body.blocks)
    assert all(set(phi.incoming) == set(predecessors[block.at]) for block in body.blocks for phi in block.phis)


@pytest.mark.parametrize("tag", ["q-O", "p-g2", "v-g2", "v-g3"])
def test_guarded_dispatch_emits_instead_of_falling_back(fixtures: Path, tag: str) -> None:
    output = wholeseg.emitted((fixtures / f"jumps-{tag}.obj").read_bytes(), native_fpu=True)
    assert output.outcome is wholeseg.Emission.LIR, output.reason
    found = corpus.loaded(output.data)
    assert found is not None
    mapped = corpus.mapped(output.data)
    assert not isinstance(mapped, str)
    # JUMPS only dispatches 1..3: neither the error call nor its table is reachable.
    assert "B$OGTA" not in found.calls.values()
    declared = blocks.statement_table(found)
    assert mapped.tables == ((declared,) if declared is not None else ())


@pytest.mark.parametrize("invalid", ["unknown_input", "wide_input", "live_output"])
def test_unproved_dispatch_remains_a_call(fixtures: Path, monkeypatch: pytest.MonkeyPatch, invalid: str) -> None:
    source = fixtures / "jumps-p-g2.obj"
    found = corpus.loaded(source)
    assert found is not None
    machine = corpus.partitioned(source)
    with monkeypatch.context() as context:
        context.setattr(raising_dispatch, "raised", lambda body, found, machine: body)
        bodies = mir.bodies(found, machine)
        public = bodies[0][1]
        body = mir._with_raise_context(public, bodies.hints[public.entry], bodies.source)
    block = next(block for block in body.blocks if block.ops and found.calls.get(block.ops[-1].at) == "B$OGTA")
    op = block.ops[-1]
    match invalid:
        case "unknown_input":
            op = replace(op, args_known=False)
        case "wide_input":
            arg = op.args[0]
            assert isinstance(arg, mir.Held)
            op = replace(op, args=(replace(arg, width=4),))
        case "live_output":
            value = next(value for value in op.defines if not value.flags)
            target = body.block(block.succ[0])
            assert target is not None
            read = mir.Op(
                target.at,
                ir.Operation.PUSH,
                "",
                (),
                (value,),
                kind=mir.Kind.ARG,
                args=(mir.Held(value, 2),),
            )
            body = replace(
                body,
                blocks=tuple(replace(one, ops=(read, *one.ops)) if one.at == target.at else one for one in body.blocks),
            )
    body = replace(
        body, blocks=tuple(replace(one, ops=(*one.ops[:-1], op)) if one.at == block.at else one for one in body.blocks)
    )
    assert raising_dispatch.raised(body, found, machine) is body


def test_read_data_error_witness_emits_native_dispatch() -> None:
    source = Path("fixtures/regressions/dispatch-p-g2.obj")
    found = corpus.loaded(source)
    assert found is not None
    bodies = mir.bodies(found, corpus.partitioned(source))
    assert any(op.kind is mir.Kind.SWITCH for _, body in bodies for block in body.blocks for op in block.ops)
    result = wholeseg.emitted(source.read_bytes(), native_fpu=True)
    assert result.outcome is wholeseg.Emission.LIR, result.reason
    mapped = corpus.mapped(result.data)
    assert not isinstance(mapped, str)
    assert mapped.tables


@pytest.mark.parametrize("number", [0, 1, 2, 3, 4, 255, 256, -1])
def test_dispatch_boundary_reaches_the_required_path(number: int) -> None:
    # The real PDS READ witness raises Illegal function call for 256 and -1.
    source = Path("fixtures/regressions/dispatch-p-g2.obj")
    found = corpus.loaded(source)
    assert found is not None
    body = mir.bodies(found, corpus.partitioned(source))[0][1]
    normal = next(block for block in body.blocks if block.ops and block.ops[-1].kind is mir.Kind.SWITCH)
    dispatch = normal.ops[-1]
    selector = dispatch.args[0]
    assert isinstance(selector, mir.Held)
    facts = {selector.value: consts.Known(number & 65535, 2)}
    guard = next(block for block in body.blocks if normal.at in block.succ)
    successors = transform._executable_successors(guard, facts, {}, {})
    assert successors is not None and len(successors) == 1
    target = body.block(successors[0])
    assert target is not None
    if not 0 <= number <= 255:
        assert target.ops[-1].kind is mir.Kind.CALL
        assert found.calls[target.ops[-1].at] == "B$OGTA"
    else:
        assert target is normal
        expected = dict(dispatch.cases).get(number, dispatch.target)
        assert transform._executable_successors(target, facts, {}, {}) == (expected,)
