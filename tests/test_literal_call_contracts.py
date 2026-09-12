from pathlib import Path
from dataclasses import replace

import pytest

import corpus
from qbopt.model import mir
from qbopt.abi import runtime


@pytest.mark.parametrize("mode", ["unknown", "writes-any", "missing", "error-handler"])
def test_unproven_print_contract_can_clobber_numeric_literals(mode: str) -> None:
    path = Path("fixtures/omf/fpdeep-p-g2.obj")
    found = corpus.loaded(path)
    assert found is not None
    contracts = runtime.for_module(found)
    for at, contract in tuple(contracts.items()):
        if mode == "error-handler" and contract.name == "B$PEI4":
            contracts[at] = replace(contract, error_handling=True)
        if contract.name != "B$PSSD":
            continue
        match mode:
            case "unknown":
                contracts[at] = runtime.worst(contract.name)
            case "writes-any":
                contracts[at] = replace(contract, writes=runtime.Memory.ANY)
            case "missing":
                del contracts[at]
    body = mir.bodies(found, corpus.partitioned(path), contracts=contracts)[0][1]
    assert body.initial
    calls = [
        op
        for block in body.blocks
        for op in block.ops
        if op.kind is mir.Kind.CALL and found.calls.get(op.at) == "B$PSSD"
    ]
    assert calls
    for op in calls:
        for literal, _ in body.initial:
            assert any(mir.overlapping(literal, ref, found.dgroup) for ref in op.stores)
