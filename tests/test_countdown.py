"""Guarded countdown loops and their profile-free measurement."""

import re

import pytest

from tools import quality
from qbopt.model import ir
from qbopt.model import lir
from qbopt.model import mir
from qbopt.analysis import loops
from qbopt.optimize import rotate
from qbopt.objectfile.module import Space


def counted_loop(*, observed: bool = False) -> mir.MirBody:
    seed = mir.Value(1, 0, variable=1, version=1)
    bound = mir.Value(2, 0, variable=2, version=1)
    counter = mir.Value(3, 1, variable=1, version=2)
    following = mir.Value(4, 2, variable=1, version=3)
    flags = mir.Value(5, 1, flags=True, variable=3, version=1)
    source = mir.MemRef(None, 2, space=Space.FRAME)
    sink = mir.MemRef(None, 2, space=Space.SEGMENT)
    initialize = mir.Op(
        0,
        ir.Operation.MOVE,
        "",
        (seed,),
        (),
        kind=mir.Kind.COPY,
        args=(mir.Const(0, 2),),
        results=(mir.Held(seed, 2),),
    )
    load = mir.Op(
        0,
        ir.Operation.MOVE,
        "",
        (bound,),
        (),
        loads=(source,),
        kind=mir.Kind.LOAD,
        args=(mir.Cell(source),),
        results=(mir.Held(bound, 2),),
    )
    compare = mir.Op(
        1,
        ir.Operation.COMPARE,
        "cmp",
        (flags,),
        (counter, bound),
        kind=mir.Kind.SUB,
        args=(mir.Held(counter, 2), mir.Held(bound, 2)),
    )
    branch = mir.Op(
        1,
        ir.Operation.BRANCH,
        "",
        (),
        (flags,),
        kind=mir.Kind.BRANCH,
        test=mir.Kind.ABOVE_EQ,
        target=3,
    )
    store = mir.Op(
        2,
        ir.Operation.MOVE,
        "",
        (),
        (counter,),
        stores=(sink,),
        kind=mir.Kind.STORE,
        args=(mir.Held(counter, 2),),
        results=(mir.Cell(sink),),
    )
    increment = mir.Op(
        2,
        ir.Operation.UNARY,
        "",
        (following,),
        (counter,),
        kind=mir.Kind.INCREMENT,
        args=(mir.Held(counter, 2),),
        results=(mir.Held(following, 2),),
    )
    jump = mir.Op(2, ir.Operation.JUMP, "", (), (), kind=mir.Kind.JUMP, target=1)
    returned = mir.Op(3, ir.Operation.RETURN, "", (), (), kind=mir.Kind.RETURN)
    return mir.MirBody(
        0,
        (
            mir.MirBlock(0, (), (initialize, load), (1,)),
            mir.MirBlock(1, (mir.Phi(counter, {0: seed, 2: following}),), (compare, branch), (2, 3)),
            mir.MirBlock(2, (), ((store,) if observed else ()) + (increment, jump), (1,)),
            mir.MirBlock(3, (), (returned,), ()),
        ),
        sealed=True,
    )


def test_dead_dynamic_counter_counts_down_on_the_step_flags_after_a_zero_trip_guard() -> None:
    """C floats retained ``add/cmp/jb`` in its ten-trip hot path.

    Clang's canonical form tests the dynamic iteration count once before the
    loop, then ends every executed trip with ``dec/jne``.  The entry test is
    essential: ``bench_floats(0)`` must still execute the body zero times.
    Require the semantic shape rather than only looking for a ``dec`` in the
    final listing, so dropping the zero-trip guard cannot satisfy the test.
    """
    body = rotate.entered(counted_loop())
    (loop,) = loops.loops(body.blocks, body.entry)
    inside = set(loop.body)
    decrements = [
        op for block in body.blocks if block.at in inside for op in block.ops if op.kind is mir.Kind.DECREMENT
    ]
    assert len(decrements) == 1
    flags = {value for value in decrements[0].defines if value.flags}
    backedges = [
        op
        for block in body.blocks
        if block.at in inside
        for op in block.ops
        if op.kind is mir.Kind.BRANCH and op.target in inside
    ]
    assert len(backedges) == 1 and backedges[0].test is mir.Kind.NE
    assert flags.intersection(backedges[0].uses)

    predecessors = loops.predecessors(body.blocks)
    entries = [at for at in predecessors[loop.header] if at not in inside]
    assert len(entries) == 1
    guard = body.block(entries[0])
    assert len(guard.succ) == 2 and any(at not in inside for at in guard.succ)
    assert guard.ops[-1].kind is mir.Kind.BRANCH and guard.ops[-1].test is mir.Kind.EQ


def test_countdown_refuses_an_observed_source_counter() -> None:
    """Replacing an index stored by the body with trips-remaining changes the program."""
    body = counted_loop(observed=True)

    assert rotate.entered(body) == body


def test_dynamic_frequencies_do_not_charge_a_rotated_entry_guard_as_fifty_fifty() -> None:
    """Floats appeared to improve from 96 to 57 dynamic operations.

    Its new zero-trip guard sits immediately outside the natural loop.  The
    generic successor split called that guard 50/50 even though the old
    pretest used the estimator's documented 90/10 loop convention.  A pure
    rotation must not manufacture a performance win in the instrument.
    """
    body = lir.LirBody(
        "guarded",
        0,
        (
            lir.LirBlock(0, (), (1, 3)),
            lir.LirBlock(1, (), (2,)),
            lir.LirBlock(2, (), (1, 3)),
            lir.LirBlock(3, (), ()),
        ),
        {},
        {},
    )

    assert quality._frequencies(body) == pytest.approx({0: 1, 1: 9, 2: 9, 3: 1})


def test_an_element_read_every_iteration_bounds_the_trip_count() -> None:
    """C's sum_three kept `dec ax` beside its byte offset: `index < first->length` has no range.

    Its word reads through 16-bit offsets stay inside one 64K object, so
    the loop runs at most 32768 times, and the offset alone can end it.
    """
    from pathlib import Path

    from qbopt.cfront import compile as cfront

    source = Path(__file__).resolve().parents[1] / "bench" / "parity" / "sum_three.c"
    text = cfront.compiled(cfront.recorded(source, []), "sum_three", optimise=True)
    procedure = text[text.index("_sum_three proc") : text.index("_sum_three endp")]

    assert re.search(r"add \w\w, 2\n", procedure)
    assert not re.search(r"    (?:dec|inc) \w\w\n", procedure)
