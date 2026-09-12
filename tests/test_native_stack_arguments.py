from pathlib import Path
from dataclasses import replace

import pytest
from iced_x86 import Register

from tests import corpus
from qbopt.model import ir
from qbopt.model import lir
from qbopt.backend import frame
from qbopt.frontend import blocks
from qbopt.frontend import extent
from qbopt.backend import prologue
from qbopt.backend import nativeframe


@pytest.mark.parametrize("expanded", [False, True])
def test_native_float_argument_moves_below_spills(expanded: bool) -> None:
    # PL_TRACE stores outgoing floats at BP-4c/BP-50; the fixed-frame guard rejected both.
    module = corpus.loaded(Path("fixtures/regressions/pl_trace-borland.obj"))
    assert module is not None
    partition = extent.partition(module)
    assert not isinstance(partition, str)
    mapped = blocks.code_map(module)
    assert not isinstance(mapped, str)
    procedure = next(body for body in partition.bodies if body.seed == 0x4E2)
    owned = tuple(
        block
        for block in blocks.partition(module, mapped)
        if any(start <= block.at < end for start, end in procedure.ranges)
    )
    plan = nativeframe.plan(owned, procedure.seed)
    assert plan is not None
    layout = nativeframe.checked(owned, plan, {0x57E: 0, 0x63D: 0})
    assert layout is not None
    lowered = lir.LirBody(
        "PL_TRACE",
        procedure.seed,
        tuple(
            lir.LirBlock(
                block.at,
                tuple(
                    lir.Insn(insn.at, (insn.at, insn.end), ir._instruction_node(module, insn).semantics, (), ())
                    for insn in block.insns
                ),
                block.succ,
            )
            for block in owned
        ),
        {},
        {},
    )
    lowered = nativeframe.bound(lowered, layout)
    if expanded:
        # PL_TRACE's recursive helper lowers its first CMP into a load and compare.
        lowered = replace(
            lowered,
            blocks=tuple(
                replace(
                    block,
                    insns=tuple(
                        part
                        for one in block.insns
                        for part in (
                            (one, replace(one, covers=(one.at, one.at), what=ir.Semantics(ir.Operation.NOTHING, "nop")))
                            if one.at == layout.entry.reserve_at
                            else (one,)
                        )
                    ),
                )
                for block in lowered.blocks
            ),
        )
    slots = frame.of(lowered, native=layout)
    slots.slot(12345, 4)
    result = prologue.reserved(lowered, slots)
    for at, expected in ((0x566, -0x50), (0x572, -0x54)):
        store = next(one for one in result.insns if one.at == at)
        assert store.what is not None
        (memory,) = store.what.dests
        assert isinstance(memory, ir.Mem) and memory.addr is not None
        assert memory.addr.disp == expected
        assert memory.through == Register.BP
    # Removing the first reservation makes its outgoing store unallocated.
    missing = tuple(replace(block, insns=tuple(insn for insn in block.insns if insn.at != 0x562)) for block in owned)
    assert nativeframe.checked(missing, plan, {0x57E: 0, 0x63D: 0}) is None
