from pathlib import Path
from dataclasses import replace

import corpus
import pytest

from qbopt.abi import runtime
from qbopt.backend import allocate, frame, lower, prologue
from qbopt.model import mir
from qbopt.objectfile import omf


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
    bodies = {body.entry: body for _, body in mir.bodies(found, corpus.partitioned(path), contracts)}
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

    body = lower.lowered("main", bodies[0x30], found.calls, found.absorbed, contracts, noreturn=0x30 in proven)
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
