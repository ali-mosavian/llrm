from pathlib import Path
from dataclasses import replace

import corpus
import pytest

from qbopt.abi import runtime
from qbopt.backend import allocate, frame, lower, prologue
from qbopt.model import mir
from qbopt.objectfile import omf


def test_closed_local_terminal_scc_is_noreturn():
    """The object path missed a→b→a because its no-return summary started empty."""
    from qbopt.analysis import noreturn
    from qbopt.model import ir

    call_b = mir.Op(2, ir.Operation.NOTHING, "", (), (), kind=mir.Kind.CALL)
    call_a = mir.Op(4, ir.Operation.NOTHING, "", (), (), kind=mir.Kind.CALL)
    first = mir.MirBody(0x30, (mir.MirBlock(0x30, (), (call_b,), ()),), sealed=True)
    second = mir.MirBody(0x40, (mir.MirBlock(0x40, (), (call_a,), ()),), sealed=True)

    assert noreturn.inferred({0x30: first, 0x40: second}, {2: 0x40, 4: 0x30}, frozenset()) == frozenset(
        {0x30, 0x40}
    )


def test_terminal_call_inerts_newly_unreachable_successor() -> None:
    """A terminal CALL left its detached successor executable after object lowering.

    Cutting a CFG edge is not enough when this cleanup runs after the ordinary
    optimizer: the successor still owns source bytes, but cannot retain a
    store or other executable MIR operation that lowering would emit.
    """
    from qbopt.analysis import noreturn
    from qbopt.model import ir

    terminal = mir.Op(2, ir.Operation.NOTHING, "", (), (), kind=mir.Kind.CALL)
    store = mir.Op(10, ir.Operation.MOVE, "mov", (), (), kind=mir.Kind.STORE)
    body = mir.MirBody(
        0,
        (
            mir.MirBlock(0, (), (terminal,), (10,)),
            mir.MirBlock(10, (), (store,), ()),
        ),
        sealed=True,
    )

    trimmed = noreturn.after_terminal_calls(body, frozenset({2}))

    assert not trimmed.block(0).succ
    orphan = trimmed.block(10)
    assert not orphan.succ
    assert all(op.kind is mir.Kind.NOTHING and not op.stores for op in orphan.ops)


@pytest.mark.parametrize("terminal", [True, False])
def test_qrender_main_spill_uses_shutdown_control_proof(terminal: bool) -> None:
    # MAIN crashed reserving two spill bytes: HOST_SHUTDOWN ends via B$CEND,
    # but the frame pass recognized only direct runtime termination calls.
    from qbopt.analysis import noreturn

    path = Path("fixtures/regressions/qrender-main-v-g3.obj")
    found = corpus.loaded(path)
    # These two external BASIC procedures enter B$ENRA before reading flags.
    external = {
        name: replace(
            runtime.worst(name),
            inputs=frozenset(
                {runtime.Reg.AX, runtime.Reg.BX, runtime.Reg.CX, runtime.Reg.DX, runtime.Reg.SI, runtime.Reg.DI}
            ),
        )
        for name in ("MOD_TEX_DUMP", "SB_DUMP")
    }
    contracts = runtime.for_module(found, external=external)
    raised = mir.bodies(found, corpus.partitioned(path), contracts)
    bodies = {body.entry: body for _, body in raised}
    symbols = {name: at for at, name in omf.pubdef_names(found.records, found.seg).items()}
    local = {at: symbols[name] for at, name in found.calls.items() if name in symbols}
    exits = frozenset(
        at
        for at, contract in contracts.items()
        if contract.established and contract.control is runtime.Control.NEVER and (terminal or at != 0x17F7)
    )
    proven = noreturn.inferred(bodies, local, exits)
    assert (0x30 in proven) is terminal
    assert (symbols["HOST_SHUTDOWN"] in proven) is terminal
    assert symbols["HOST_INIT"] not in proven  # Has END arms and a returning arm.

    handler = bodies[symbols["HOST_SHUTDOWN"]]
    terminal_sites = exits | frozenset(at for at, target in local.items() if target in proven)
    # The object path used the summary only for its prologue: after B$CEND
    # at 0x17f7 it still lowered a dead call and return in this same block.
    assert [op.at for op in handler.block(0x17BF).ops[-3:]] == [0x17F7, 0x17FC, 0x1801]
    trimmed = noreturn.after_terminal_calls(handler, terminal_sites)
    if terminal:
        assert [op.at for op in trimmed.block(0x17BF).ops[-1:]] == [0x17F7]
        assert not trimmed.block(0x17BF).succ
    else:
        assert trimmed is handler
    assert not mir.verify(trimmed)

    body = lower.lowered(
        "main",
        bodies[0x30],
        found.calls,
        found.absorbed,
        contracts,
        noreturn=0x30 in proven,
        nodes=raised.source.nodes,
    )
    assert (
        allocate.applied(replace(body, blocks=()), allocate.Assignment({}, frozenset(), 0, True)).noreturn is terminal
    )
    slots = frame.Frame(0)
    slots.slot(23, 2)
    if not terminal:
        with pytest.raises(prologue.Refused):
            prologue.reserved(body, slots, found.calls)
        return
    result = prologue.reserved(body, slots, found.calls)
    assert result.insns[0].what.name == "sub"
    assert result.insns[0].what.sources[-1].value == 2
    assert sum(one.frame_adjust for one in result.insns) == 1
    assert any(one.at == 0x10D for one in result.insns)
