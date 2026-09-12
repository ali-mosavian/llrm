from pathlib import Path
from dataclasses import replace

import pytest

import corpus
from qbopt.model import mir
from qbopt.abi import runtime
from qbopt.objectfile.module import Addr
from qbopt.objectfile.module import Space
from qbopt.frontend import raising_call_memory


def test_angle_compare_does_not_clobber_program_data() -> None:
    path = Path("fixtures/regressions/qrender-pl-move-v-g3.obj")
    found = corpus.loaded(path)
    assert found is not None and found.program_data is not None
    body = next(
        body
        for name, body in mir.bodies(found, corpus.partitioned(path), basic_semantics=True)
        if name.endswith(" MDL_ANGLEMOD")
    )
    calls = [
        op
        for block in body.blocks
        for op in block.ops
        if op.kind is mir.Kind.CALL and found.calls.get(op.at) == "B$FCMP"
    ]
    assert calls
    for op in calls:
        program = mir.MemRef(Addr(Space.SEGMENT, 6, found.program_data), 2)
        scratch = mir.MemRef(Addr(Space.LITERAL, 0), 2)
        for effects in (op.stores, op.loads):
            assert effects
            assert not any(mir.overlapping(program, ref, frozenset()) for ref in effects)
            assert any(mir.overlapping(scratch, ref, frozenset()) for ref in effects)


@pytest.mark.parametrize(
    "reads,writes", [(runtime.Memory.ANY, runtime.Memory.NONE), (runtime.Memory.NONE, runtime.Memory.ANY)]
)
def test_read_and_write_contracts_are_independent(reads: runtime.Memory, writes: runtime.Memory) -> None:
    path = Path("fixtures/regressions/qrender-pl-move-v-g3.obj")
    found = corpus.loaded(path)
    assert found is not None and found.program_data is not None
    contracts = {
        at: replace(contract, reads=reads, writes=writes) if contract.name == "B$FCMP" else contract
        for at, contract in runtime.for_module(found).items()
    }
    body = next(
        body
        for name, body in mir.bodies(found, corpus.partitioned(path), contracts=contracts, basic_semantics=True)
        if name.endswith(" MDL_ANGLEMOD")
    )
    calls = [
        op
        for block in body.blocks
        for op in block.ops
        if op.kind is mir.Kind.CALL and found.calls.get(op.at) == "B$FCMP"
    ]
    assert calls
    program = mir.MemRef(Addr(Space.SEGMENT, 6, found.program_data), 2)
    for op in calls:
        for access, effects in ((reads, op.loads), (writes, op.stores)):
            assert any(mir.overlapping(program, ref, frozenset()) for ref in effects) == (access is runtime.Memory.ANY)


def test_selected_unknown_contract_keeps_program_data_live() -> None:
    path = Path("fixtures/regressions/qrender-pl-move-v-g3.obj")
    found = corpus.loaded(path)
    assert found is not None and found.program_data is not None
    contracts = runtime.for_module(found)
    contracts = {
        at: runtime.worst(contract.name) if contract.name == "B$FCMP" else contract
        for at, contract in contracts.items()
    }
    body = next(
        body
        for name, body in mir.bodies(found, corpus.partitioned(path), contracts=contracts, basic_semantics=True)
        if name.endswith(" MDL_ANGLEMOD")
    )
    calls = [
        op
        for block in body.blocks
        for op in block.ops
        if op.kind is mir.Kind.CALL and found.calls.get(op.at) == "B$FCMP"
    ]
    assert calls
    assert all(ref.beyond is None for op in calls for ref in (*op.loads, *op.stores))


@pytest.mark.parametrize(
    "contract",
    [
        replace(runtime.contract("B$FCMP"), established=False),
        replace(runtime.contract("B$FCMP"), enters_user_code=True),
        replace(runtime.contract("B$FCMP"), error_handling=True),
        replace(runtime.contract("B$FCMP"), raises_error=True),
        replace(runtime.contract("B$FCMP"), control=runtime.Control.UNKNOWN),
        None,
    ],
)
def test_unproven_or_exceptional_call_keeps_unknown_effects(contract: runtime.Contract | None) -> None:
    assert raising_call_memory.reachable(contract, runtime.Memory.NONE, (5, frozenset())) is None


@pytest.mark.parametrize("access", list(runtime.Memory))
def test_access_reach_preserves_escape_and_unknown_information(access: runtime.Memory) -> None:
    escaped = (5, frozenset({(5, 6)}))
    result = raising_call_memory.reachable(runtime.contract("B$FCMP"), access, escaped)
    expected = (
        None if access is runtime.Memory.ANY else (5, frozenset()) if access <= runtime.Memory.ARGUMENTS else escaped
    )
    assert result == expected
    assert raising_call_memory.reachable(runtime.contract("B$FCMP"), access, None) is None


def test_error_capability_is_not_an_unconditional_callback() -> None:
    routine = runtime.contract("B$PSSD")
    assert routine.raises_error
    escaped = (5, frozenset({(5, 6)}))
    assert raising_call_memory.reachable(routine, routine.writes, escaped, handles_errors=False) == escaped
    assert raising_call_memory.reachable(routine, routine.writes, escaped, handles_errors=True) is None
