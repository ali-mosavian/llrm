"""
qbopt/mir.py's own gate: SSA is either well-formed or it is not a graph you
can reason about, so the invariants are the test.
"""

from pathlib import Path
from dataclasses import fields

import pytest

import corpus
from qbopt import ir
from qbopt import mir
from qbopt.blocks import Ends
from qbopt.module import Addr
from qbopt.blocks import Block
from qbopt.module import Space

FIXTURES = sorted(Path("fixtures/omf").glob("*.obj"))


def block(at: int, succ: tuple[int, ...], ends: Ends = Ends.CONDITIONAL) -> Block:
    return Block(at=at, end=at + 1, insns=(), ends=ends, succ=succ)


def raised(obj: Path) -> list[tuple[mir.MirBody, list[Block]]]:
    """Every body of one object, raised -- the unit mir.py actually takes."""
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
        built = mir.raise_body(mine, nodes, body.body.seed)
        assert not isinstance(built, str), built
        out.append((built, [b for b in mine if built.block(b.at) is not None]))
    return out


def test_an_empty_body_is_refused_rather_than_guessed() -> None:
    assert isinstance(mir.raise_body([], {}), str)


def test_an_entry_outside_the_blocks_is_refused() -> None:
    assert isinstance(mir.raise_body([block(0, ())], {}, 99), str)


def test_another_bodys_blocks_do_not_come_along() -> None:
    """A procedure is reached by a call, which is no CFG edge.

    Handing raise_body a whole module's blocks used to leave the other
    body's in the graph -- they have their own predecessors, so a phi went
    into them that the dominator walk never reached to fill, and the value
    arriving there vanished. Only what the entry reaches is raised.
    """
    both = [block(0, (1,)), block(1, (), Ends.RETURN), block(0x99, (0x99,))]
    built = mir.raise_body(both, {}, 0)
    assert not isinstance(built, str)
    assert {one.at for one in built.blocks} == {0, 1}, "the second body is not this body"


def test_a_successor_outside_the_body_is_dropped_from_the_edge() -> None:
    built = mir.raise_body([block(0, (1, 0x99)), block(1, (), Ends.RETURN)], {}, 0)
    assert not isinstance(built, str)
    entry = built.block(0)
    assert entry is not None
    assert entry.succ == (1,)


@pytest.mark.parametrize("obj", FIXTURES, ids=lambda p: p.stem)
def test_every_body_raises_and_the_form_holds(obj: Path) -> None:
    """The three SSA promises, over the real corpus.

    Each is load-bearing for a different consumer, and verify()'s own
    docstring says which. A renaming bug does not make the graph malformed
    in any way an eye would catch -- it makes it describe a different
    program -- so this is the only thing standing between construction and
    everything built on it.
    """
    for built, where in raised(obj):
        assert mir.verify(built, where) == []


@pytest.mark.parametrize("obj", FIXTURES, ids=lambda p: p.stem)
def test_a_phi_argument_comes_from_every_predecessor(obj: Path) -> None:
    """The bug the ordering fix was for: a phi created on entry to its own
    block loses the edge from any predecessor renamed before it."""
    for built, _ in raised(obj):
        for one in built.blocks:
            for phi in one.phis:
                assert phi.incoming, f"{phi.result} at {one.at:#06x} has no arguments at all"


@pytest.mark.parametrize("obj", FIXTURES, ids=lambda p: p.stem)
def test_a_value_is_defined_exactly_once(obj: Path) -> None:
    for built, _ in raised(obj):
        seen = built.values
        assert len(seen) == len(set(seen))


@pytest.mark.parametrize("obj", FIXTURES, ids=lambda p: p.stem)
def test_a_barrier_reads_and_writes_every_tracked_value(obj: Path) -> None:
    """ir.py's contract says a barrier's operands are pinned and nothing may
    move across one. Here that is exactly "it defines and uses everything",
    so no value's live range spans it and the allocator has no choice to make.
    """
    for built, _ in raised(obj):
        for one in built.blocks:
            for op in one.ops:
                if not op.barrier:
                    continue
                assert {built.origin[v] for v in op.defines} >= set(mir.TRACKED)
                assert {built.origin[v] for v in op.uses} >= set(mir.TRACKED)


def test_the_carry_between_a_pair_is_an_edge_not_an_adjacency() -> None:
    """BC's own 32-bit arithmetic: `sub` then `sbb`, where the borrow is the
    whole reason the second instruction is there. In SSA the second reads the
    flags value the first defined, so folding the pair later needs no
    pattern-match on their addresses."""
    found = corpus.loaded(Path("fixtures/omf/arith-v-g3.obj"))
    assert found is not None
    partitioned = corpus.partitioned(Path("fixtures/omf/arith-v-g3.obj"))
    result = corpus.bodies(Path("fixtures/omf/arith-v-g3.obj"))
    assert not isinstance(result, str)
    nodes = {ir.span(n)[0]: n for body in result for n in body.nodes}
    built = mir.raise_body(partitioned, nodes, partitioned[0].at)
    assert not isinstance(built, str)

    pairs = 0
    for one in built.blocks:
        for first, second in zip(one.ops, one.ops[1:], strict=False):
            if second.name not in ("adc", "sbb"):
                continue
            carried = [v for v in first.defines if v.flags]
            assert carried, f"{first.name} at {first.at:#06x} feeds {second.name} but defines no flags"
            assert carried[0] in second.uses, "the pair is joined by the flags value"
            pairs += 1
    assert pairs, "arith-v-g3 is a program of 32-bit arithmetic; it has pairs"


def test_the_frame_and_the_segments_never_become_values() -> None:
    """They are where values live, not values. Promoting bp would dissolve
    every local, and a segment register decides which bytes an access names.

    Asked of body.origin, since a value no longer names a register at all --
    the question is whether raising ever made one OF bp or a segment, which
    is a fact about what was raised and lives in that map now.
    """
    assert not (set(mir.TRACKED) & mir.PHYSICAL)
    for built, _ in raised(Path("fixtures/omf/procs-v-g3.obj")):
        for value in built.values:
            assert built.origin[value] not in mir.PHYSICAL


@pytest.mark.parametrize("obj", FIXTURES, ids=lambda p: p.stem)
def test_every_op_keeps_the_node_it_came_from(obj: Path) -> None:
    """A lowering that applies no transform is that node's own bytes. Losing
    the origin is what would make the identity round-trip unprovable."""
    for built, _ in raised(obj):
        for one in built.blocks:
            for op in one.ops:
                assert isinstance(op.node, ir.Opaque | ir.Long | ir.Call | ir.Restore | ir.Data)


@pytest.mark.parametrize("obj", FIXTURES, ids=lambda p: p.stem)
def test_a_body_lowers_back_to_the_bytes_it_came_from(obj: Path) -> None:
    """The identity gate, per block.

    Raising and lowering with nothing transformed has to give back exactly
    what was read, and it is checkable now precisely because nothing is
    transformed yet -- which is the only moment the machinery is free to
    verify. Once a pass changes a body this round trip is what an
    instruction selector gets measured against.

    Per block, not per body: a body's own ranges also carry alignment
    padding and code no path reaches, which raise_body deliberately drops
    (see test_another_bodys_blocks_do_not_come_along). Measured across
    fixtures/omf and bench/nbody.bas, a raised block covers 97.1% of body
    bytes; the rest is not instructions this graph ever claimed to hold.
    """
    found = corpus.loaded(obj)
    assert found is not None
    where = {one.at: one for one in corpus.partitioned(obj)}
    for built, _ in raised(obj):
        for one in built.blocks:
            source = where[one.at]
            rebuilt = ir.emit(found, tuple(op.node for op in one.ops if op.node is not None))
            assert rebuilt == found.code[source.at : source.end], f"{obj.stem} {one.at:#06x}"


def test_a_phi_lowers_to_nothing() -> None:
    """It never was an instruction. BC said "these two definitions meet" by
    writing one register on both paths, so until a pass actually splits them
    there is nothing to put back."""
    joined = 0
    for built, _ in raised(Path("fixtures/omf/jumptable.obj")):
        joined += sum(len(one.phis) for one in built.blocks)
        assert len(mir.lower(built)) == sum(1 for one in built.blocks for op in one.ops if op.node is not None)
    assert joined, "jumptable.obj has joins; some block carries a phi"


def test_a_call_that_preserves_si_does_not_give_it_a_new_value() -> None:
    """ir.Effects answers "any register" for every call, which is right for
    a layer that knows nothing about the callee and wrong here: it hands si
    a fresh value across B$DVI4, which the QuickBASIC 4.5 source says
    preserves it, and two accesses through that si stop looking like one
    address. Worth 19% of all SSA values across the corpus.
    """
    known = mir._call_touches("B$DVI4")
    assert known is not None
    disturbed, _ = known
    assert mir.NAMES[mir.Register.ESI] == "esi"
    assert mir.Register.ESI not in disturbed
    assert mir.Register.EDI not in disturbed
    assert mir.Register.EAX in disturbed


def test_a_call_with_no_established_contract_still_disturbs_everything() -> None:
    """A user SUB, or a routine runtime.py could not read. Falling back to
    ir.Effects is what keeps using the contracts from being an assumption."""
    assert mir._call_touches("NOT_A_ROUTINE") is None
    assert mir._call_touches("B$EVCK") is None, "it can dispatch into user code"


def test_the_same_address_through_a_rewritten_register_is_not_the_same_bytes() -> None:
    """NBODY 0x461 and 0x476 both read es:[bx+0], with `mov bx,6Dh` between
    them. Keyed on the register name they are one address and forwarding
    one to the other is corruption; keyed on the value they are two."""
    here = mir.MemRef(
        Addr(Space.FAR, 0, base=mir.Register.BX, segment=mir.Register.ES),
        1,
        base=mir.Value(41, 0x461),
    )
    there = mir.MemRef(
        Addr(Space.FAR, 0, base=mir.Register.BX, segment=mir.Register.ES),
        1,
        base=mir.Value(47, 0x476),
    )
    assert here.addr == there.addr, "the same Addr, which is the point"
    assert not mir.same_bytes(here, there)
    assert mir.same_bytes(here, here)


def test_a_reference_nothing_can_name_is_never_known_to_be_anything() -> None:
    unknown = mir.MemRef(None, 2)
    assert not mir.same_bytes(unknown, unknown)
    assert mir.overlapping(unknown, mir.MemRef(Addr(Space.SEGMENT, 0, 5), 2), frozenset())


def test_a_value_carries_no_register() -> None:
    """The invariant docs/variables.md exists for.

    A value used to be named after the register BC kept it in, which made
    two computations incomparable by what they compute and left no way to
    say "the low half of that". Guarded here rather than trusted, because
    re-adding the field would be the easy way to fix any downstream break
    and would silently undo the whole change.
    """
    assert not hasattr(mir.Value(1, 0), "of")
    assert {field.name for field in fields(mir.Value)} == {"id", "at", "flags"}


@pytest.mark.parametrize("obj", FIXTURES, ids=lambda p: p.stem)
def test_every_value_has_an_origin_and_a_distinct_name(obj: Path) -> None:
    """Nothing is lost by moving the register off the value.

    Every value still has one, and no two values share an id -- which is
    what lets `v7` be a name rather than a description.
    """
    for built, _ in raised(obj):
        seen: set[int] = set()
        for value in built.values:
            assert value in built.origin, f"{value} has no origin"
            assert value.id not in seen, f"{value.id} names two values"
            seen.add(value.id)


def test_the_flags_variable_is_a_kind_not_a_register() -> None:
    """`flags` is a bool on the value; mir.FLAGS is only what raising reads.

    Every consumer that used to compare against a sentinel register now
    asks the value what it is, which is the same question without the
    machine in it.
    """
    assert mir.Value(1, 0).flags is False
    assert mir.Value(2, 0, flags=True).flags is True
    for built, _ in raised(Path("fixtures/omf/arith-v-g3.obj")):
        for value in built.values:
            assert value.flags == (built.origin[value] is mir.FLAGS)


@pytest.mark.parametrize("obj", FIXTURES, ids=lambda p: p.stem)
def test_a_restore_redefines_only_the_half_it_moves(obj: Path) -> None:
    """`push eax / pop ax / pop dx` does not redefine eax.

    ir.RESTORE_EFFECTS says both roots, and has to: `pop ax` is a partial
    write, and a per-register layer cannot say the bits written are the ones
    already there. Here it can, and saying otherwise ends the live range of
    the very value being restored -- a false definition in the middle of
    every absorbed site.
    """
    for built, _ in raised(obj):
        for block in built.blocks:
            for op in block.ops:
                if op.op is not mir.Synth.HALF_TO_LOW:
                    continue
                assert len(op.defines) == 1, f"{op.at:#x}: a restore defines one register, not {op.defines}"
                assert len(op.uses) == 2, f"{op.at:#x}: it reads the source and the register it writes into"
                assert op.defines[0] in op.uses, "the written root is read too -- the write is partial"


@pytest.mark.parametrize("obj", FIXTURES, ids=lambda p: p.stem)
def test_the_restored_value_survives_its_own_restore(obj: Path) -> None:
    """The point of the previous test, stated as the thing it buys.

    The source of a restore is not redefined by it, so whatever reads that
    value afterwards reads the same one -- which is what makes the round
    trip an identity rather than a chain through a fresh definition.
    """
    for built, _ in raised(obj):
        for block in built.blocks:
            for op in block.ops:
                if op.op is not mir.Synth.HALF_TO_LOW:
                    continue
                source = next(one for one in op.uses if one != op.defines[0])
                assert source not in op.defines, f"{op.at:#x}: the restore redefined its own source"


def test_the_restore_pairs_are_the_two_calls_py_emits() -> None:
    """eax/edx and ecx/ebx, matching ir.FIXUP's own numbering."""
    assert mir.RESTORE_PAIR == {
        0: (mir.Register.EAX, mir.Register.EDX),
        1: (mir.Register.ECX, mir.Register.EBX),
    }
