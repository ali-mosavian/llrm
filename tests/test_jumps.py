"""Jumps the block order made redundant are gone from the printed procedure.

Over qcport: 335 `jcc` over a `jmp`, 262 `jmp` to the next label, 69 jumps
to a block that only jumps.
"""

from pathlib import Path
from dataclasses import replace

from iced_x86 import Register

from qbopt import flow
from qbopt.model import ir
from qbopt.model import lir
from qbopt.backend import masm
from qbopt.backend import frame
from qbopt.backend import jumps
from qbopt.cfront import compile as cfront

AX, BX, CX = (ir.Reg(one, 2) for one in (Register.AX, Register.BX, Register.CX))


def _insn(at, op, name, dests=(), sources=(), target=None):
    return lir.Insn(at, (at, 1), ir.Semantics(op, name, dests, sources, target), (), ())


def _compare(at):
    return _insn(at, ir.Operation.COMPARE, "cmp", (), (AX, BX))


def _branch(at, name, target):
    return _insn(at, ir.Operation.BRANCH, name, target=target)


def _jump(at, target):
    return _insn(at, ir.Operation.JUMP, "jmp", target=target)


def _move(at, source):
    return _insn(at, ir.Operation.MOVE, "mov", (AX,), (source,))


def _return(at):
    return _insn(at, ir.Operation.RETURN, "ret")


def _inserted(one: lir.Insn) -> lir.Insn:
    return replace(one, covers=(one.at, one.at))


def _printed(*blocks):
    body = lir.LirBody("f", blocks[0].at, blocks, {}, {})
    procedure = masm.Procedure("_f", True, False, jumps.threaded(body), 0, {})
    return [line.strip() for line in masm._procedure(procedure, {}, 0)][1:-1]


def test_branch_to_a_block_that_only_jumps_goes_to_its_target():
    assert _printed(
        lir.LirBlock(1, (_compare(1), _branch(2, "je", 7)), (7, 4)),
        lir.LirBlock(4, (_move(4, CX), _return(5)), ()),
        lir.LirBlock(7, (_jump(7, 9),), (9,)),
        lir.LirBlock(8, (_move(8, BX), _return(8)), ()),
        lir.LirBlock(9, (_return(9),), ()),
    ) == ["L0_1:", "cmp ax, bx", "je L0_9", "L0_4:", "mov ax, cx", "ret", "L0_9:", "ret"]


def test_branch_over_a_jump_is_inverted():
    assert _printed(
        lir.LirBlock(1, (_compare(1), _branch(2, "je", 9)), (9, 4)),
        lir.LirBlock(4, (_jump(4, 12),), (12,)),
        lir.LirBlock(9, (_move(9, CX), _return(10)), ()),
        lir.LirBlock(12, (_move(12, BX), _return(13)), ()),
    ) == ["L0_1:", "cmp ax, bx", "jne L0_12", "L0_9:", "mov ax, cx", "ret", "L0_12:", "mov ax, bx", "ret"]


def test_branch_then_jump_in_one_block_is_inverted():
    assert _printed(
        lir.LirBlock(1, (_compare(1), _branch(2, "je", 4), _jump(3, 9)), (4, 9)),
        lir.LirBlock(4, (_move(4, CX), _return(5)), ()),
        lir.LirBlock(9, (_move(9, BX), _return(10)), ()),
    ) == ["L0_1:", "cmp ax, bx", "jne L0_9", "L0_4:", "mov ax, cx", "ret", "L0_9:", "mov ax, bx", "ret"]


def test_conditional_assignment_arm_is_placed_before_its_join() -> None:
    """QB qlight left a jump after each clamp assignment.

    When one conditional edge performs straight-line work and then joins the
    other edge, its complete trace belongs between the test and the join.
    This is a CFG property independent of source branch orientation.
    """
    body = lir.LirBody(
        "clamp",
        1,
        (
            lir.LirBlock(1, (_compare(1), _branch(2, "jg", 4)), (4, 3)),
            lir.LirBlock(3, (_inserted(_jump(3, 7)),), (7,)),
            lir.LirBlock(4, (_move(4, CX), _jump(5, 7)), (7,)),
            lir.LirBlock(7, (_return(7),), ()),
        ),
        {},
        {},
    )
    procedure = masm.Procedure("_clamp", True, False, jumps.threaded(jumps.placed(body)), 0, {})

    assert [line.strip() for line in masm._procedure(procedure, {}, 0)][1:-1] == [
        "L0_1:",
        "cmp ax, bx",
        "jle L0_7",
        "L0_4:",
        "mov ax, cx",
        "L0_7:",
        "ret",
    ]


def test_shared_machine_pipeline_threads_the_final_branch_pair() -> None:
    """Fresh QB D_SURF retained 189 ``jcc body; jmp exit; body`` pairs.

    The C driver happened to invoke the threader after the shared machine
    pipeline.  A frontend that consumes that pipeline directly therefore
    encoded both edges.  Final allocated control-flow cleanup is a backend
    phase, not a responsibility each frontend must remember independently.
    """
    body = lir.LirBody(
        "f",
        1,
        (
            lir.LirBlock(1, (_compare(1), _branch(2, "je", 4)), (4, 9)),
            lir.LirBlock(4, (_return(4),), ()),
            lir.LirBlock(9, (_return(9),), ()),
        ),
        {},
        {},
    )

    result = flow.machine({}, frame.Frame(0), {})[-1].transform(body)

    real = [one.what for one in result.blocks[0].insns if one.what is not None]
    assert [(one.op, one.name, one.target) for one in real] == [
        (ir.Operation.COMPARE, "cmp", None),
        (ir.Operation.BRANCH, "je", 4),
    ]
    assert tuple(block.at for block in result.blocks) == (1, 9, 4)
    emitted = [line.strip() for line in masm._procedure(masm.Procedure("_f", True, False, result, 0, {}), {}, 0)]
    assert not any(line.startswith("jmp ") for line in emitted), emitted


def test_loop_placement_ignores_non_emitting_instruction_markers() -> None:
    """Optimized QB nbody crashed after scheduling instead of reaching timing.

    Allocation and scheduling may retain ownership markers with no machine
    semantics. They are not loop tests and must not be dereferenced while
    identifying a loop header's final branch pair.
    """
    marker = lir.Insn(2, (2, 2), None, (), ())
    body = lir.LirBody(
        "marker-loop",
        1,
        (
            lir.LirBlock(1, (_jump(1, 2),), (2,)),
            lir.LirBlock(2, (marker, _compare(2), _branch(2, "je", 4), _jump(2, 3)), (4, 3)),
            lir.LirBlock(3, (_jump(3, 2),), (2,)),
            lir.LirBlock(4, (_return(4),), ()),
        ),
        {},
        {},
    )

    result = jumps.placed(body)

    assert {block.at for block in result.blocks} == {1, 2, 3, 4}


def test_jump_to_the_next_block_is_dropped():
    assert _printed(
        lir.LirBlock(1, (_move(1, BX), _jump(2, 4)), (4,)),
        lir.LirBlock(4, (_return(4),), ()),
    ) == ["L0_1:", "mov ax, bx", "L0_4:", "ret"]


def test_source_owned_jump_to_fallthrough_becomes_an_anchor() -> None:
    """SCALAR lost the three decoded bytes of its preheader jump after unrolling."""
    jump = lir.Insn(
        1,
        (1, 4),
        ir.Semantics(ir.Operation.JUMP, "jmp", (), (), 4),
        (),
        (),
    )
    body = lir.LirBody(
        "f",
        1,
        (
            lir.LirBlock(1, (jump,), (4,)),
            lir.LirBlock(4, (_return(4),), ()),
        ),
        {},
        {},
    )

    result = jumps.threaded(body)

    kept = result.blocks[0].insns
    assert len(kept) == 1
    assert kept[0].covers == (1, 4)
    assert kept[0].what is not None and kept[0].what.op is ir.Operation.NOTHING


def test_jump_over_a_block_nothing_reaches_is_dropped():
    """cfg_trim kept `jne L0_18; jmp L0_13` over an empty block 12 nothing enters:
    unreachable blocks went only once some other rule had fired."""
    assert _printed(
        lir.LirBlock(1, (_move(1, BX), _jump(2, 9)), (9,)),
        lir.LirBlock(5, (), ()),
        lir.LirBlock(9, (_return(9),), ()),
    ) == ["L0_1:", "mov ax, bx", "L0_9:", "ret"]


def test_unreachable_inert_source_ownership_survives_threading() -> None:
    """SCALAR unrolled to 1789, then final threading lost the old loop bytes.

    The unreachable block contains no machine work, but its nonempty source
    spans prove that optimization deliberately replaced the decoded region.
    Layout must still receive those anchors for fresh OMF emission.
    """
    owned = lir.Insn(
        5,
        (5, 9),
        ir.Semantics(ir.Operation.NOTHING, "", (), ()),
        (),
        (),
    )
    body = lir.LirBody(
        "f",
        1,
        (
            lir.LirBlock(1, (_return(1),), ()),
            lir.LirBlock(5, (owned,), ()),
        ),
        {},
        {},
    )

    result = jumps.threaded(body)

    assert tuple(block.at for block in result.blocks) == (1, 5)
    assert result.blocks[1].insns == (owned,)


def test_unreachable_inert_carrier_without_a_byte_span_does_not_crash_threading() -> None:
    """A carried, non-generated LIR occurrence can have no contiguous ``covers`` span."""
    carrier = lir.Insn(
        5,
        None,
        ir.Semantics(ir.Operation.NOTHING, "", (), ()),
        (),
        (),
    )
    body = lir.LirBody(
        "f",
        1,
        (
            lir.LirBlock(1, (_return(1),), ()),
            lir.LirBlock(5, (carrier,), ()),
        ),
        {},
        {},
    )

    result = jumps.threaded(body)

    assert tuple(block.at for block in result.blocks) == (1,)


def test_reachable_inert_source_ownership_is_not_a_transparent_passage() -> None:
    """PARITY's fully unrolled BASIC loop left only source-map anchors.

    Jump threading treated that reachable block as empty, redirected its
    predecessor around it, and then discarded 53 bytes of deliberate source
    ownership.  Fresh OMF emission consequently refused the apparent hole.
    An inert block is transparent only when it owns no decoded source bytes.
    """
    owned = lir.Insn(
        5,
        (5, 9),
        ir.Semantics(ir.Operation.NOTHING, "", (), ()),
        (),
        (),
    )
    body = lir.LirBody(
        "f",
        1,
        (
            lir.LirBlock(1, (_jump(1, 5),), (5,)),
            lir.LirBlock(5, (owned,), (9,)),
            lir.LirBlock(9, (_return(9),), ()),
        ),
        {},
        {},
    )

    result = jumps.threaded(body)

    assert tuple(block.at for block in result.blocks) == (1, 5, 9)
    assert result.blocks[0].succ == (5,)
    assert result.blocks[1].insns == (owned,)


def test_threading_preserves_an_exact_empty_loop_header() -> None:
    """Mandel's rewound coordinate made its outer header emit no bytes.

    Threading redirected the 24-row backedge through that block into the
    32-column header, merging two natural loops in the measurement CFG.  The
    executable bytes were sound, but the dynamic estimate fell implausibly
    from 74,839 to 1,065 instructions.  A proved header costs no bytes and
    must remain as a zero-length CFG anchor.
    """
    from qbopt.analysis import loops

    body = lir.LirBody(
        "nested",
        1,
        (
            lir.LirBlock(1, (_jump(1, 8),), (8,)),
            lir.LirBlock(8, (), (10,)),
            lir.LirBlock(10, (_compare(10), _branch(11, "je", 20)), (14, 20)),
            lir.LirBlock(14, (_move(14, CX), _jump(15, 10)), (10,)),
            lir.LirBlock(20, (_compare(20), _branch(21, "jne", 8)), (8, 30)),
            lir.LirBlock(30, (_return(30),), ()),
        ),
        {},
        {},
        loop_trip_counts=((8, 24), (10, 32)),
    )

    result = jumps.threaded(body)

    assert any(block.at == 8 for block in result.blocks)
    assert {loop.header for loop in loops.loops(result.blocks, result.entry)} == {8, 10}


def test_a_block_that_jumps_to_itself_stays():
    """`for (;;);` -- following jumps must not go round forever."""
    assert _printed(lir.LirBlock(1, (_jump(1, 1),), (1,))) == ["L0_1:", "jmp L0_1"]


def test_identical_result_tails_are_merged() -> None:
    """qglsurf emitted the same zero-result tail from two failure arms."""
    body = lir.LirBody(
        "f",
        1,
        (
            lir.LirBlock(1, (_compare(1), _branch(2, "je", 20)), (20, 10)),
            lir.LirBlock(10, (_inserted(_move(10, ir.Imm(0, 2))), _inserted(_jump(11, 30))), (30,)),
            lir.LirBlock(20, (_inserted(_move(20, ir.Imm(0, 2))), _inserted(_jump(21, 30))), (30,)),
            lir.LirBlock(30, (_return(30),), ()),
        ),
        {},
        {},
    )

    result = jumps.merged(jumps.placed(body))
    physical = [
        one.what
        for block in result.blocks
        for one in block.insns
        if one.what is not None and one.what.op is not ir.Operation.NOTHING
    ]

    assert sum(what.op is ir.Operation.MOVE for what in physical) == 1
    assert sum(what.op is ir.Operation.BRANCH for what in physical) == 0


def test_identical_source_owned_tails_keep_their_distinct_anchors() -> None:
    """Decoded tails cannot share one copy without reconciling source maps."""
    body = lir.LirBody(
        "f",
        1,
        (
            lir.LirBlock(1, (_compare(1), _branch(2, "je", 20)), (20, 10)),
            lir.LirBlock(10, (_move(10, ir.Imm(0, 2)), _jump(11, 30)), (30,)),
            lir.LirBlock(20, (_move(20, ir.Imm(0, 2)), _jump(21, 30)), (30,)),
            lir.LirBlock(30, (_return(30),), ()),
        ),
        {},
        {},
    )

    result = jumps.merged(jumps.placed(body))

    assert (
        sum(one.what is not None and one.what.op is ir.Operation.MOVE for block in result.blocks for one in block.insns)
        == 2
    )


def test_shared_machine_pipeline_merges_fresh_identical_tails() -> None:
    """Fresh frontends inherited C's two identical failure-result tails.

    Tail sharing used to be called only by the C driver after the common
    machine pipeline.  The allocated shape is frontend-independent, while
    source-owned decoded instructions still need their distinct anchors.
    """
    body = lir.LirBody(
        "f",
        1,
        (
            lir.LirBlock(1, (_compare(1), _branch(2, "je", 20)), (20, 10)),
            lir.LirBlock(10, (_inserted(_move(10, ir.Imm(0, 2))), _inserted(_jump(11, 30))), (30,)),
            lir.LirBlock(20, (_inserted(_move(20, ir.Imm(0, 2))), _inserted(_jump(21, 30))), (30,)),
            lir.LirBlock(30, (_return(30),), ()),
        ),
        {},
        {},
    )

    result = flow.machine({}, frame.Frame(0), {})[-1].transform(body)
    physical = [
        one.what
        for block in result.blocks
        for one in block.insns
        if one.what is not None and one.what.op is not ir.Operation.NOTHING
    ]

    assert sum(what.op is ir.Operation.MOVE for what in physical) == 1
    assert sum(what.op is ir.Operation.BRANCH for what in physical) == 0


def test_tail_sharing_rejects_a_static_saving_that_adds_hot_work() -> None:
    """Unpriced tail sharing grew C sieve from 54 to 55 instructions.

    The regression is the cost decision, not sieve's total instruction count:
    unrelated loop optimization later reduced that total from 46 to 40 while
    preserving the decision.  Model a smaller static result whose extra loop
    instruction makes it dynamically dearer and require the original body.
    """

    def body(entry: tuple[lir.Insn, ...], loop: tuple[lir.Insn, ...]) -> lir.LirBody:
        return lir.LirBody(
            "f",
            1,
            (
                lir.LirBlock(1, entry, (10,)),
                lir.LirBlock(10, loop, (30, 20)),
                lir.LirBlock(20, (_jump(20, 10),), (10,)),
                lir.LirBlock(30, (_return(30),), ()),
            ),
            {},
            {},
        )

    before = body(
        (_move(1, BX), _move(2, CX), _move(3, ir.Imm(0, 2)), _jump(4, 10)),
        (_compare(10), _branch(11, "je", 30)),
    )
    after = body(
        (_jump(1, 10),),
        (_move(9, CX), _compare(10), _branch(11, "je", 30)),
    )

    assert jumps._work(after)[0] < jumps._work(before)[0]
    assert jumps._work(after)[1] > jumps._work(before)[1]
    assert jumps.preferred(before, after) is before


def test_byte_neutral_return_duplication_removes_a_join_jump() -> None:
    """choose's folded arms joined through a two-byte ``jmp`` to ``pop bp; retf``.

    With the one-byte implicit frame pop included, copying the two-byte return
    sequence into both arms is byte-neutral and removes the executed jump.
    """
    body = lir.LirBody(
        "choose",
        1,
        (
            lir.LirBlock(1, (_compare(1), _branch(2, "je", 20)), (20, 10)),
            lir.LirBlock(10, (_move(10, ir.Imm(8, 2)),), (30,)),
            lir.LirBlock(30, (_inserted(_return(30)),), ()),
            lir.LirBlock(20, (_move(20, ir.Imm(10, 2)), _inserted(_jump(21, 30))), (30,)),
        ),
        {},
        {},
    )

    result = jumps.duplicated_returns(body, return_overhead=1)
    physical = [one.what for block in result.blocks for one in block.insns if one.what is not None]

    assert all(block.at != 30 for block in result.blocks)
    assert sum(one.op is ir.Operation.RETURN for one in physical) == 2
    assert all(one.op is not ir.Operation.JUMP for one in physical)


def test_return_duplication_rejects_growth_and_source_owned_tails() -> None:
    """A larger return tail, or one owning source bytes, stays shared."""

    def candidate(tail: tuple[lir.Insn, ...]) -> lir.LirBody:
        return lir.LirBody(
            "f",
            1,
            (
                lir.LirBlock(1, (_compare(1), _branch(2, "je", 20)), (20, 10)),
                lir.LirBlock(10, (_move(10, AX),), (30,)),
                lir.LirBlock(30, tail, ()),
                lir.LirBlock(20, (_move(20, BX), _inserted(_jump(21, 30))), (30,)),
            ),
            {},
            {},
        )

    large = candidate((_inserted(_move(30, ir.Imm(1234, 2))), _inserted(_return(31))))
    owned = candidate((_return(30),))

    assert jumps.duplicated_returns(large, return_overhead=1) is large
    assert jumps.duplicated_returns(owned, return_overhead=1) is owned


def test_qglsurf_shares_all_three_zero_result_tails() -> None:
    """QGL surface failure exits share one zero result and emit at most 56 instructions.

    The zero can be materialised as either `xor r,r` or `mov r,0`; the tail
    sharing is the property this regression protects, not that local encoding
    choice.  Narrowing the synthetic high-word return reload removed the 57th
    instruction, and eliminating a redundant low-word return shuttle removed
    the 56th.  Future general improvements may remove more; growth is the
    regression this bound catches.
    """
    source = Path("fixtures/c/qglsurf.cgs")
    module = cfront.assembled(source.read_text(), source.stem, optimise=True)
    (procedure,) = module.procedures
    physical = [
        one
        for block in procedure.body.blocks
        for one in block.insns
        if one.what is not None and one.what.op is not ir.Operation.NOTHING
    ]

    assert len(physical) <= 56
    shared_zero_tails = []
    for block in procedure.body.blocks:
        real = [one.what for one in block.insns if one.what is not None and one.what.op is not ir.Operation.NOTHING]
        if len(real) != 1 or len(block.succ) != 1:
            continue
        (zero,) = real
        zeroed = (
            zero.op is ir.Operation.BINARY
            and zero.name == "xor"
            and zero.dests == (ir.Reg(Register.EAX, 4),)
            and len(zero.sources) == 2
            and zero.sources[0] == zero.sources[1]
        ) or (
            zero.op is ir.Operation.MOVE
            and zero.dests == (ir.Reg(Register.EAX, 4),)
            and len(zero.sources) == 1
            and isinstance(zero.sources[0], ir.Imm)
            and zero.sources[0].value == 0
        )
        predecessors = sum(block.at in other.succ for other in procedure.body.blocks)
        if zeroed and predecessors >= 3:
            shared_zero_tails.append(block.at)
    assert len(shared_zero_tails) == 1


def test_loop_test_is_placed_after_its_latch():
    """The raise lays a C loop out test first, so entered at its body the
    latch still jumped back to the test every pass: `cmp / je out / body / jmp test`."""
    body = lir.LirBody(
        "f",
        1,
        (
            lir.LirBlock(1, (_jump(1, 8),), (8,)),
            lir.LirBlock(4, (_compare(4), _branch(5, "je", 23)), (23, 8)),
            lir.LirBlock(8, (_move(8, CX), _jump(9, 4)), (4,)),
            lir.LirBlock(23, (_return(23),), ()),
        ),
        {},
        {},
    )
    procedure = masm.Procedure("_f", True, False, jumps.threaded(jumps.placed(body)), 0, {})
    assert [line.strip() for line in masm._procedure(procedure, {}, 0)][1:-1] == [
        "L0_1:",
        "L0_8:",
        "mov ax, cx",
        "L0_4:",
        "cmp ax, bx",
        "jne L0_8",
        "L0_23:",
        "ret",
    ]


def test_loop_not_known_to_run_is_entered_at_its_test_placed_last():
    """A loop whose first test could fail kept its test on top, `cmp / jge out /
    body / jmp test`. Entered by one jump to the test placed after the body, as
    bcc writes it, each pass takes one branch."""
    body = lir.LirBody(
        "f",
        1,
        (
            lir.LirBlock(1, (_move(1, CX),), (3,)),
            lir.LirBlock(3, (_compare(3), _branch(4, "jge", 17)), (17, 8)),
            lir.LirBlock(8, (_move(8, BX), _jump(9, 3)), (3,)),
            lir.LirBlock(17, (_return(17),), ()),
        ),
        {},
        {},
    )
    procedure = masm.Procedure("_f", True, False, jumps.threaded(jumps.placed(body)), 0, {})
    assert [line.strip() for line in masm._procedure(procedure, {}, 0)][1:-1] == [
        "L0_1:",
        "mov ax, cx",
        "jmp L0_3",
        "L0_8:",
        "mov ax, bx",
        "L0_3:",
        "cmp ax, bx",
        "jl L0_8",
        "L0_17:",
        "ret",
    ]


def test_loop_test_is_followed_by_the_block_it_leaves_for():
    """With the test placed after the latch, the walk went on to whatever block
    came next in the old order, so every pass took `jg out` and then `jmp body`:
    sieve ran 5% slower than with its test on top."""
    body = lir.LirBody(
        "f",
        1,
        (
            lir.LirBlock(1, (_move(1, CX),), (3,)),
            lir.LirBlock(3, (_compare(3), _branch(4, "jge", 20)), (20, 8)),
            lir.LirBlock(8, (_move(8, BX), _jump(9, 3)), (3,)),
            lir.LirBlock(12, (_return(12),), ()),
            lir.LirBlock(20, (_move(20, CX), _jump(21, 12)), (12,)),
        ),
        {},
        {},
    )
    procedure = masm.Procedure("_f", True, False, jumps.threaded(jumps.placed(body)), 0, {})
    assert [line.strip() for line in masm._procedure(procedure, {}, 0)][1:-1] == [
        "L0_1:",
        "mov ax, cx",
        "jmp L0_3",
        "L0_8:",
        "mov ax, bx",
        "L0_3:",
        "cmp ax, bx",
        "jl L0_8",
        "L0_20:",
        "mov ax, cx",
        "L0_12:",
        "ret",
    ]


def test_loop_trace_is_kept_before_its_exit():
    """pal_bestfit left its latch until after the rotated loop header, producing
    `jge exit / jmp body` on every pass instead of one branch.  Prefer the
    still-unplaced in-loop edge even when it is the conditional edge."""
    body = lir.LirBody(
        "f",
        1,
        (
            lir.LirBlock(1, (_move(1, CX), _jump(2, 12)), (12,)),
            lir.LirBlock(12, (_compare(12), _branch(13, "jge", 73)), (73, 16)),
            lir.LirBlock(16, (_compare(16), _branch(17, "jge", 68)), (68, 57)),
            lir.LirBlock(57, (_compare(57), _branch(58, "jne", 68)), (68, 73)),
            lir.LirBlock(68, (_move(68, BX), _jump(69, 12)), (12,)),
            lir.LirBlock(73, (_return(73),), ()),
        ),
        {},
        {},
    )
    procedure = masm.Procedure("_f", True, False, jumps.threaded(jumps.placed(body)), 0, {})
    printed = [line.strip() for line in masm._procedure(procedure, {}, 0)][1:-1]
    assert not any(
        printed[index].startswith("j")
        and not printed[index].startswith("jmp ")
        and printed[index + 1].startswith("jmp ")
        for index in range(len(printed) - 1)
    )
