"""The LIR verifier rejects malformed values at the phase that made them."""

from dataclasses import replace

import pytest
from iced_x86 import Register

from qbopt import flow
from qbopt.model import ir
from qbopt.model import lir
from qbopt.backend import verify
from qbopt.backend import parcopy
from qbopt.backend import allocate
from qbopt.model.passes import LIRTransform


def test_a_read_value_must_be_defined_or_an_explicit_body_input() -> None:
    """C crc32 returned -1141145971 after an eliminated phi lost its definition.

    The edge copy still listed the stale value as a use, so the old verifier's
    operand bookkeeping considered it accounted for.  Allocation eventually
    turned it into a reload from a frame slot nothing had stored.
    """
    stale = lir.Insn(
        at=1,
        covers=(1, 1),
        what=ir.Semantics(ir.Operation.MOVE, "mov", (ir.Held(2, 4),), (ir.Held(1, 4),)),
        defines=(2,),
        uses=(1,),
    )
    body = lir.LirBody(
        name="undefined",
        entry=1,
        blocks=(lir.LirBlock(at=1, insns=(stale,)),),
        origin={1: 24},
        pins={},
    )

    said = verify.verify(body)

    assert any("value#1 is read but never defined or supplied" in complaint for complaint in said), said


def test_an_explicit_body_input_satisfies_the_definition_rule() -> None:
    incoming = lir.Insn(
        at=1,
        covers=(1, 1),
        what=ir.Semantics(ir.Operation.PUSH, "push", (), (ir.Held(1, 2),)),
        defines=(),
        uses=(1,),
    )
    body = lir.LirBody(
        name="input",
        entry=1,
        blocks=(lir.LirBlock(at=1, insns=(incoming,)),),
        origin={1: 24},
        pins={},
        inputs=frozenset({1}),
    )

    assert not verify.verify(body)


@pytest.mark.parametrize("covers", [None, (1, 1)])
def test_register_allocation_keeps_the_definition_of_an_elided_identity(
    covers: tuple[int, int] | None,
) -> None:
    """C sieve's return read value#98 after regalloc deleted its definition.

    The fixed-register copy became ``mov ax, ax`` after placement.  It emits
    no instruction, but its virtual definition must remain as an inert marker
    so later LIR analyses know why the return requirement's value exists.
    """
    identity = lir.Insn(
        at=1,
        covers=covers,
        what=ir.Semantics(ir.Operation.MOVE, "mov", (ir.Held(2, 2),), (ir.Held(1, 2),)),
        defines=(2,),
        uses=(1,),
    )
    returning = lir.Insn(
        at=2,
        covers=None,
        what=ir.Semantics(ir.Operation.RETURN, "", (), ()),
        defines=(),
        uses=(2,),
        requires=((ir.Held(2, 2), Register.AX),),
    )
    body = lir.LirBody(
        name="fixed-identity",
        entry=1,
        blocks=(lir.LirBlock(at=1, insns=(identity, returning)),),
        origin={},
        pins={},
        inputs=frozenset({1}),
    )
    assignment = allocate.Assignment({1: Register.AX, 2: Register.AX}, frozenset(), 0.0, True)

    placed = allocate.applied(body, assignment)

    assert not verify.verify(placed)


def test_parallel_copy_identity_keeps_its_virtual_definition() -> None:
    """C sieve read value#126 after its identity phi copy disappeared.

    Allocation must leave grouped copies for the parallel-copy scheduler.
    Once scheduled, a same-register copy emits nothing but still records the
    virtual definition on which later LIR safety checks depend.
    """
    identity = lir.Insn(
        at=1,
        covers=(1, 1),
        what=ir.Semantics(ir.Operation.MOVE, "mov", (ir.Held(2, 2),), (ir.Held(1, 2),)),
        defines=(2,),
        uses=(1,),
        group=7,
    )
    consumed = lir.Insn(
        at=2,
        covers=(2, 2),
        what=ir.Semantics(ir.Operation.PUSH, "push", (), (ir.Held(2, 2),)),
        defines=(),
        uses=(2,),
    )
    body = lir.LirBody(
        name="parallel-identity",
        entry=1,
        blocks=(lir.LirBlock(at=1, insns=(identity, consumed)),),
        origin={},
        pins={},
        inputs=frozenset({1}),
    )
    assignment = allocate.Assignment({1: Register.AX, 2: Register.AX}, frozenset(), 0.0, True)

    placed = allocate.applied(body, assignment)
    scheduled = parcopy.scheduled(placed)

    assert not verify.verify(placed)
    assert not verify.verify(scheduled)


def test_the_machine_phase_gate_names_the_phase_that_made_bad_lir() -> None:
    class LosesDefinition(LIRTransform):
        name = "loses-definition"

        def transform(self, body: lir.LirBody) -> lir.LirBody:
            broken = replace(body.blocks[0].insns[0], uses=(99,))
            return replace(body, blocks=(replace(body.blocks[0], insns=(broken,)),))

    source = lir.Insn(
        at=1,
        covers=(1, 1),
        what=ir.Semantics(ir.Operation.MOVE, "mov", (ir.Held(2, 2),), (ir.Held(1, 2),)),
        defines=(2,),
        uses=(1,),
    )
    body = lir.LirBody(
        name="phase",
        entry=1,
        blocks=(lir.LirBlock(at=1, insns=(source,)),),
        origin={},
        pins={},
        inputs=frozenset({1}),
    )

    with pytest.raises(verify.Malformed, match=r"loses-definition: value#99 is read"):
        flow.checked(body, LosesDefinition(), in_ssa=False)


def test_unreachable_byte_ownership_markers_are_not_executable_blocks() -> None:
    marker = lir.Insn(
        at=2,
        covers=(2, 4),
        what=ir.Semantics(ir.Operation.NOTHING, "", (), ()),
        defines=(),
        uses=(),
    )
    body = lir.LirBody(
        name="dead-ownership",
        entry=1,
        blocks=(lir.LirBlock(at=1, insns=()), lir.LirBlock(at=2, insns=(marker,))),
        origin={},
        pins={},
    )

    assert not verify.verify(body)


def test_unreachable_executable_work_is_still_malformed() -> None:
    work = lir.Insn(
        at=2,
        covers=(2, 4),
        what=ir.Semantics(ir.Operation.PUSH, "push", (), (ir.Imm(1, 2),)),
        defines=(),
        uses=(),
    )
    body = lir.LirBody(
        name="dead-work",
        entry=1,
        blocks=(lir.LirBlock(at=1, insns=()), lir.LirBlock(at=2, insns=(work,))),
        origin={},
        pins={},
    )

    assert any("block 0x0002 is not reachable" in complaint for complaint in verify.verify(body))
