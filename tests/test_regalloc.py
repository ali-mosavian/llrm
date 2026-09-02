"""
qbopt/regalloc.py's own gate: liveness is where an allocator goes wrong
quietly, so the invariants that bound it are the test.
"""

from pathlib import Path

import pytest

import corpus
from iced_x86 import Register_
from qbopt import ir
from qbopt import mir
from qbopt import regalloc

FIXTURES = sorted(Path("fixtures/omf").glob("*.obj"))


def raised(obj: Path) -> list[mir.MirBody]:
    found = corpus.loaded(obj)
    assert found is not None
    partitioned = corpus.partitioned(obj)
    result = corpus.bodies(obj)
    if isinstance(result, str) or not partitioned:
        return []
    nodes = {ir.span(n)[0]: n for body in result for n in body.nodes}
    out = []
    for body in result:
        mine = [b for b in partitioned if any(lo <= b.at < hi for lo, hi in body.body.ranges)]
        if not mine:
            continue
        built = mir.raise_body(mine, nodes, body.body.seed, found.calls)
        assert not isinstance(built, str), built
        out.append(built)
    return out


@pytest.mark.parametrize("obj", FIXTURES, ids=lambda p: p.stem)
def test_no_body_needs_more_registers_than_bc_used(obj: Path) -> None:
    """The program came out of six registers, so at no point can seven
    values have been live. A number above that is a liveness bug and not a
    body that needs spilling -- which is what it was twice, first from
    attributing phi arguments to the join instead of the edge, then from
    never killing a value the caller supplied.
    """
    for body in raised(obj):
        assert regalloc.pressure(body) <= len(regalloc.AVAILABLE)


@pytest.mark.parametrize("obj", FIXTURES, ids=lambda p: p.stem)
def test_a_value_never_interferes_with_another_version_of_itself(obj: Path) -> None:
    """Two versions of eax live at once would mean BC had them in one
    register at one moment, which it cannot have. Measured over 27,680
    values: none. This is what makes the identity assignment always valid,
    and so what the whole allocator rests on.

    Asks body.origin rather than the value: a value no longer carries where
    it lived, which is the point of docs/variables.md -- and this is one of
    the few questions genuinely about BC's registers, so it asks for them.
    """
    for body in raised(obj):
        origin = body.origin
        for one, others in regalloc.interference(body).items():
            assert not [other for other in others if origin.get(other) is origin.get(one)]


@pytest.mark.parametrize("obj", FIXTURES, ids=lambda p: p.stem)
def test_an_unconstrained_body_moves_nothing(obj: Path) -> None:
    """Every value back where BC had it. An allocator that moves what it was
    not asked to move emits copies for nothing -- greedy by degree did,
    1,274 of them, all valid and all pointless."""
    for body in raised(obj):
        got = regalloc.colour(body)
        assert not isinstance(got, str), got
        assert regalloc.moved(body, got) == 0


@pytest.mark.parametrize("obj", FIXTURES, ids=lambda p: p.stem)
def test_a_colouring_gives_interfering_values_different_registers(obj: Path) -> None:
    for body in raised(obj):
        got = regalloc.colour(body)
        assert not isinstance(got, str), got
        for one, others in regalloc.interference(body).items():
            for other in others:
                if other in got and one in got:
                    assert got[one] is not got[other], f"{one} and {other} share {got[one]}"


def test_an_entry_value_is_defined_where_the_body_starts() -> None:
    """A body reads a register before writing it whenever BC passes
    something in. Nothing defines it, so a backward liveness that does not
    treat the entry as its definition keeps it live at every point that can
    reach its use -- which around a loop is everywhere."""
    seen = 0
    for body in raised(Path("fixtures/omf/divmod-p-g2-zd.obj")):
        arriving = regalloc.entry_values(body)
        if not arriving:
            continue
        seen += 1
        found = regalloc.live(body)
        assert not (arriving & found.live_in[body.entry]), "defined on entry, so not live into it"
    assert seen, "divmod's own main body reads registers BC passed in"


@pytest.mark.parametrize("obj", FIXTURES[:20], ids=lambda p: p.stem)
def test_a_pin_that_cannot_be_honoured_is_refused_not_guessed(obj: Path) -> None:
    """Pre-colouring breaks the chordal guarantee, so colouring can fail --
    and failing has to be a refusal. Pinning every value in a body to one
    register is the extreme case of that."""
    for body in raised(obj):
        graph = regalloc.interference(body)
        clashing = [one for one, others in graph.items() if others]
        if len(clashing) < 2:
            continue
        pinned = {one: regalloc.AVAILABLE[0] for one in clashing}
        assert isinstance(regalloc.colour(body, pinned), str)
        return


def test_a_flags_phi_does_not_enter_the_interference_graph() -> None:
    """meet() keeps flags out of the graph. One line put them back.

    `graph.setdefault(phi.result, set())` ran for every phi, flags included,
    so a latch that merges a flags value put it in the graph. The greedy
    pass then asked for a register, found NONE was not one it could use, and
    handed out the first free general register -- taking eax from values
    that wanted it and pushing three of them into the register a pin had
    asked for. segld printed 0 for 1050.

    Driven twice over: the graph is only consulted when something pins, and
    the shape needs a flags value merged at a header.
    """
    from iced_x86 import Register

    start = mir.Value(1, 0x10)
    again = mir.Value(2, 0x20)
    merged = mir.Value(3, 0x20)
    flag_in = mir.Value(4, 0x10, flags=True)
    flag_back = mir.Value(5, 0x20, flags=True)
    flag_merged = mir.Value(6, 0x20, flags=True)

    ax = ir.Reg(register=Register.AX, width=2)
    one = ir.Imm(value=1, width=2)

    def op(at: int, what: ir.Semantics, defines: tuple, uses: tuple = ()) -> mir.Op:
        return mir.Op(at, what.op, what.name or "", defines, uses, made=what)

    moving = ir.Semantics(ir.Operation.MOVE, "mov", dests=(ax,), sources=(one,))
    comparing = ir.Semantics(ir.Operation.COMPARE, "cmp", dests=(), sources=(ax, one))

    head = mir.MirBlock(0x10, (), (op(0x10, moving, (start,)), op(0x14, comparing, (flag_in,), (start,))), (0x20,))
    latch = mir.MirBlock(
        0x20,
        (mir.Phi(merged, {0x10: start, 0x20: again}), mir.Phi(flag_merged, {0x10: flag_in, 0x20: flag_back})),
        (op(0x20, moving, (again,), (merged,)), op(0x24, comparing, (flag_back,), (again, flag_merged))),
        (0x20,),
    )
    body = mir.MirBody(
        0x10,
        (head, latch),
        {
            start: Register.AX,
            again: Register.AX,
            merged: Register.AX,
            flag_in: Register.NONE,
            flag_back: Register.NONE,
            flag_merged: Register.NONE,
        },
        {},
    )

    graph = regalloc.interference(body)
    assert flag_merged not in graph, "a flags phi is not something a register is placed in"
    assert not any(value.flags for value in graph), "and neither is any other flags value"


def test_an_allocation_that_moves_a_value_across_a_phi_is_refused() -> None:
    """A phi is not an instruction: nothing runs on the edge to move a value.

    So a phi's result and every value arriving at it have to already be in
    one register. colour() assigns them independently, and segld's inner
    counter was defined into dx and read back out of ax through the phi
    between them -- it never reached its bound and the program ran forever.

    Driven, because the corpus reaches this only through a pass that asks
    for a pin, and every such pass refuses first for other reasons.
    """
    from iced_x86 import Register

    from qbopt import ir
    from qbopt import mir

    start = mir.Value(1, 0x10)
    round_again = mir.Value(2, 0x20)
    merged = mir.Value(3, 0x20)

    def move(at: int, into: Register_, defines: tuple, uses: tuple = ()) -> mir.Op:
        what = ir.Semantics(
            ir.Operation.MOVE, "mov", dests=(ir.Reg(register=into, width=2),), sources=(ir.Imm(value=1, width=2),)
        )
        return mir.Op(at, ir.Operation.MOVE, "mov", defines, uses, made=what)

    head = mir.MirBlock(0x10, (), (move(0x10, Register.AX, (start,)),), (0x20,))
    latch = mir.MirBlock(
        0x20,
        (mir.Phi(merged, {0x10: start, 0x20: round_again}),),
        (move(0x20, Register.AX, (round_again,), (merged,)),),
        (0x20,),
    )
    body = mir.MirBody(
        0x10, (head, latch), {start: Register.AX, round_again: Register.AX, merged: Register.AX}, {}
    )

    assert not isinstance(regalloc.colour(body, {}), str), "leaving everything where it was is always valid"
    moved = regalloc.colour(body, {start: Register.DX})
    assert isinstance(moved, str), "moving one side of a phi and not the other must be refused"
    assert "nothing moves it" in moved, moved
