"""
qbopt/analysis/observers.py and the dead-store walk that asks it: a store
nothing can read before the program ends, or before the frame is gone, is
dead whatever calls run in between.
"""

from pathlib import Path

import corpus
import pytest

from qbopt import wholeseg
from qbopt.model import ir
from qbopt.model import mir
from qbopt.objectfile import omf
from qbopt.analysis import avail
from qbopt.analysis import observers
from qbopt.frontend import blocks as split
from qbopt.objectfile import module
from qbopt.objectfile.module import Addr
from qbopt.objectfile.module import Space

X = mir.MemRef(Addr(Space.SEGMENT, 0x76, 5), 4)
NBODY = Path("fixtures/bench/nbody-v-g3.obj")


def _store(at: int, cell: mir.MemRef = X) -> mir.Op:
    value = mir.Value(at + 1, 0)
    return mir.Op(at, ir.Operation.MOVE, "mov", (), (value,), kind=mir.Kind.STORE,
                  args=(mir.Held(value, cell.width),), results=(mir.Cell(cell),), stores=(cell,))


def _load(at: int, cell: mir.MemRef = X) -> mir.Op:
    value = mir.Value(at + 1, 0)
    return mir.Op(at, ir.Operation.MOVE, "mov", (value,), (), kind=mir.Kind.LOAD,
                  args=(mir.Cell(cell),), results=(mir.Held(value, cell.width),), loads=(cell,))


def _call(at: int) -> mir.Op:
    return mir.Op(at, ir.Operation.CALL, "call", (), (), kind=mir.Kind.CALL)


def _dead(body: mir.MirBody, private=None) -> list[int]:
    return [op.at for op in avail.dead_stores(body, frozenset({5}), {}, private)]


def _everything(ref: mir.MemRef) -> bool:
    return True


def test_a_store_no_iteration_reads_is_dead_inside_its_loop() -> None:
    """nbody wrote deltaX on every inner pass: the back edge vetoed the proof.

    The loop's header and latch store nothing, and each waited on the other
    to say the cell was overwritten, so neither ever did.
    """
    body = mir.MirBody(0, (
        mir.MirBlock(0, (), (), (10,)),
        mir.MirBlock(10, (), (), (12, 14)),
        mir.MirBlock(12, (), (_store(12),), (14,)),
        mir.MirBlock(14, (), (), (10, 20)),
        mir.MirBlock(20, (), (), ()),
    ))
    assert _dead(body, _everything) == [12]


def test_a_store_the_next_iteration_reads_stays() -> None:
    body = mir.MirBody(0, (
        mir.MirBlock(0, (), (), (10,)),
        mir.MirBlock(10, (), (_load(10), _store(12)), (10, 20)),
        mir.MirBlock(20, (), (), ()),
    ))
    assert _dead(body, _everything) == []


def test_a_call_cannot_read_a_private_cell() -> None:
    body = mir.MirBody(0, (mir.MirBlock(0, (), (_store(0), _call(2)), ()),))
    assert _dead(body, _everything) == [0]
    assert _dead(body) == []


def test_an_unresolved_address_cannot_read_a_private_cell_but_its_name_can() -> None:
    unknown = mir.MemRef(None, 2)
    blind = mir.MirBody(0, (mir.MirBlock(0, (), (_store(0), _load(2, unknown)), ()),))
    named = mir.MirBody(0, (mir.MirBlock(0, (), (_store(0), _load(2)), ()),))
    assert _dead(blind, _everything) == [0]
    assert _dead(named, _everything) == []


def test_a_taken_frame_address_makes_no_frame_cell_private() -> None:
    found = module.of(omf.read(NBODY))
    blocks = split.partition(found, split.code_map(found))
    ((_, main), *_rest) = mir.bodies(found, blocks)
    slot = mir.MemRef(Addr(Space.FRAME, -0x18), 4)
    assert observers.private(main, found, blocks)(slot)
    push = mir.Op(0, ir.Operation.PUSH, "push", (), (), kind=mir.Kind.ARG, args=(mir.FrameAddress(-0x18, 2),))
    taken = mir.MirBody(main.entry, (mir.MirBlock(main.entry, (), (push,), ()), *main.blocks[1:]))
    assert not observers.private(taken, found, blocks)(slot)


def test_only_the_main_body_owns_a_variable_nothing_else_names() -> None:
    found = module.of(omf.read(NBODY))
    blocks = split.partition(found, split.code_map(found))
    bodies = dict(mir.bodies(found, blocks))
    main = observers.private(bodies["main (main)"], found, blocks)
    procedure = observers.private(bodies["procedure PITSNAP"], found, blocks)
    t_start = mir.MemRef(Addr(Space.SEGMENT, 0x9E, 5), 4)  # its address goes to PitSnap
    assert main(X)
    assert not main(t_start)
    assert not procedure(X)


def test_nbody_writes_no_scratch_variable_in_its_inner_loop() -> None:
    """deltaX, deltaY, dist2, falloff and a product slot: five dead writes per pass."""
    states = []

    def watch(stage, name, state):
        if stage == "mir-widen" and name.startswith("main"):
            states.append(state)

    result = wholeseg.emitted(NBODY.read_bytes(), watch=watch)
    assert result.outcome is wholeseg.Emission.LIR, result.reason
    written = {
        (cell.addr.space, cell.addr.disp)
        for block in states[0].blocks
        for op in block.ops
        if (cell := avail.stored_cell(op)) is not None and cell.addr is not None
    }
    scratch = {(Space.SEGMENT, disp) for disp in (0x76, 0x7A, 0x7E, 0x82)} | {(Space.FRAME, -0x18)}
    assert not written & scratch


def _loops(obj: Path) -> list[list]:
    """The rebuilt object's loops: the instructions from each backward branch's target to it."""
    from iced_x86 import FlowControl
    from qbopt.frontend import blocks

    result = wholeseg.emitted(obj.read_bytes())
    assert result.outcome is wholeseg.Emission.LIR, result.reason
    found = module.of(omf.parse(result.data))
    insns = [one for block in blocks.partition(found, blocks.code_map(found)) for one in block.insns]
    return [
        [one.insn for one in insns if back.insn.near_branch_target <= one.at <= back.at]
        for back in insns
        if back.insn.flow_control == FlowControl.CONDITIONAL_BRANCH and back.insn.near_branch_target < back.at
    ]


def _nbody_inner_loop():
    """The rebuilt nbody inner loop: the backward branch around the divide."""
    from iced_x86 import Mnemonic

    return next(loop for loop in _loops(NBODY) if any(one.mnemonic == Mnemonic.IDIV for one in loop))


def test_nbody_counts_its_inner_loop_in_one_register() -> None:
    """nbody's `other` went `mov cx,bx / inc cx / cmp cx,5 / mov bx,cx` on every pass.

    The dead writes kept their values live across the loop, and the counter's
    copy into its increment could not be coalesced.
    """
    from iced_x86 import Mnemonic
    from iced_x86 import FlowControl

    loop = _nbody_inner_loop()
    step = next(index for index, one in enumerate(loop) if one.mnemonic == Mnemonic.INC)
    counted, compared, branch = loop[step : step + 3]
    assert compared.mnemonic == Mnemonic.CMP and compared.op0_register == counted.op0_register
    assert branch.flow_control == FlowControl.CONDITIONAL_BRANCH


@pytest.mark.parametrize("tag", ["p-g2", "q-O", "v-g3"])
def test_a_long_handed_to_a_sub_keeps_both_halves_stored(tag) -> None:
    """PROCS printed TWICE= 3088. Report was handed &r, and r's high half, stored at
    +2 by its own operand, looked like a separate cell nobody could see: each call's
    DX store went, and only the low word of Twice& reached Report."""
    from iced_x86 import Register
    from iced_x86 import Mnemonic
    from iced_x86 import OpKind

    result = wholeseg.emitted(Path(f"fixtures/omf/procs-{tag}.obj").read_bytes())
    assert result.outcome is wholeseg.Emission.LIR, result.reason
    stores = [
        one.insn for block in corpus.partitioned(result.data) for one in block.insns
        if one.insn.mnemonic == Mnemonic.MOV and one.insn.op0_kind == OpKind.MEMORY
        and one.insn.memory_base == Register.NONE and one.insn.op1_kind == OpKind.REGISTER
    ]
    assert sum(one.op1_register == Register.DX for one in stores) == 3
