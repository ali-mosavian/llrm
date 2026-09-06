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
def test_a_barrier_names_every_register_it_pins(obj: Path) -> None:
    """ir.py's contract says a barrier's operands are pinned. The SSA has to
    name each one, or a value would cross an instruction that owns it.

    Not "everything", which is what this asserted while every barrier in the
    corpus happened to have unknowable effects. suite/fpdeep.bas brought in
    `movsw`, whose effects iced states exactly: si and di and nothing else.
    Demanding all of TRACKED there would be demanding a wrong answer -- ax
    may live across a movsw. What must hold is that mir names whatever
    ir.pinned() pins, and that is what this checks.
    """
    for built, _ in raised(obj):
        for one in built.blocks:
            for op in one.ops:
                if not op.barrier:
                    continue
                assert op.node is not None
                held = ir.pinned(op.node)
                wanted = set(mir.TRACKED) if held is None else {r for r in held if r in mir.TRACKED}
                assert {built.origin[v] for v in op.defines} >= wanted
                assert {built.origin[v] for v in op.uses} >= wanted


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
    # `variable` and `version` are an index and a counter -- which variable
    # this is a version of, and which version -- so that one register's
    # values read as one variable written N times. Neither is a register,
    # and which register a variable was is still origin's alone.
    assert {field.name for field in fields(mir.Value)} == {
        "id",
        "at",
        "flags",
        "variable",
        "version",
    }
    made = mir.Value(1, 0, False, 3, 7)
    assert isinstance(made.variable, int) and isinstance(made.version, int)
    assert repr(made) == "v3_7", repr(made)


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


@pytest.mark.parametrize("obj", FIXTURES, ids=lambda p: p.stem)
def test_resolving_a_body_that_has_not_moved_changes_nothing(obj: Path) -> None:
    """The rebuild has to be a no-op on a body nothing has touched.

    Everything a pass does afterwards rests on it, and a rebuild that
    quietly disagrees with raise_body would be the worst kind of bug here --
    the values would be plausible and wrong. Three things checked: the same
    blocks with the same operations, the same number of phis in each, and
    the same nodes coming back out of lower().
    """
    from qbopt import omf
    from qbopt import module
    from qbopt import blocks as split
    from qbopt.blocks import code_map

    found = module.of(omf.parse(obj.read_bytes()))
    if found is None:
        pytest.skip(reason="no code segment")  # ty: ignore[unknown-argument]
    mapped = code_map(found)
    if isinstance(mapped, str):
        pytest.skip(reason=mapped)  # ty: ignore[unknown-argument]

    for name, body in mir.bodies(found, split.partition(found, mapped)):
        got = mir.resolved(body, found.calls)
        assert not isinstance(got, str), f"{obj.stem} {name}: {got}"

        shape = [(one.at, tuple((op.at, op.name) for op in one.ops), len(one.phis)) for one in body.blocks]
        after = [(one.at, tuple((op.at, op.name) for op in one.ops), len(one.phis)) for one in got.blocks]
        assert after == shape, f"{obj.stem} {name}: the rebuild changed the body's shape"
        assert mir.lower(got) == mir.lower(body), f"{obj.stem} {name}: the rebuild lowers differently"
        assert not mir.verify(got, split.partition(found, mapped)), f"{obj.stem} {name}: the rebuild is not SSA"


def test_mir_operands_say_exactly_what_the_node_said() -> None:
    """The check that decides whether `made` can go.

    Rebuild ir.Semantics from Op.args and Op.results -- a value through
    origin, a constant as an Imm, a cell as its MemRef -- and it has to be
    the node's own, operand for operand. Anything MIR cannot express is an
    Opaque and reproduces itself; if the model were lossy this is where it
    would show, and it is 2,343 operations across the suite.
    """
    from qbopt import ir
    from qbopt import mir
    from qbopt import omf
    from qbopt import module
    from qbopt import select
    from qbopt import blocks as split
    from qbopt.blocks import code_map

    def back(arg, origin):
        if isinstance(arg, mir.Held):
            was = origin[arg.value]
            return ir.Reg(register=select.AT_WIDTH.get(was, {}).get(arg.width, was), width=arg.width)
        if isinstance(arg, mir.Const):
            return ir.Imm(value=arg.n, width=arg.width)
        if isinstance(arg, mir.Cell):
            return arg.ref
        return arg.what

    seen = 0
    for one in sorted(Path("fixtures/omf").glob("*-p-g2.obj")):
        found = module.of(omf.parse(one.read_bytes()))
        if found is None:
            continue
        mapped = code_map(found)
        if isinstance(mapped, str):
            continue
        for _, body in mir.bodies(found, split.partition(found, mapped)):
            for block in body.blocks:
                for op in block.ops:
                    what = getattr(op.node, "semantics", None)
                    if what is None:
                        continue
                    # Not one the raise folded: an absorbed runtime call is
                    # one operation over its argument values and the node it
                    # came from is the `call`, which names none of them.
                    if op.id is not None and op.id in found.absorbed:
                        continue
                    seen += 1
                    if op.kind is mir.Kind.CALL:
                        # A call still says exactly what its node said --
                        # `call` names no operand, and the node names none.
                        # What MIR adds is the contract's implicit inputs,
                        # which are values and not encodings, so they are
                        # checked against the read set rather than against
                        # the node.
                        assert not what.sources and not what.dests, f"{op.name}: a call names operands"
                        assert op.results == (), f"{op.name}: results {op.results}"
                        read = {one.id for one in op.uses}
                        for arg in op.args:
                            assert arg.value.id in read, f"{op.name}: {arg} is not among the values it reads"
                        assert op.args_known or op.args == (), f"{op.name}: unknown interface with arguments"
                        continue
                    for kind, mine, theirs in (
                        ("source", op.args, what.sources),
                        ("dest", op.results, what.dests),
                    ):
                        # `inc` and `dec` are their own operations, and
                        # each takes the one value it steps: MIR does not
                        # write out the 1 the opcode carries, because the
                        # two differ from `add`/`sub` in what they leave in
                        # the carry.
                        if op.kind in (mir.Kind.INCREMENT, mir.Kind.DECREMENT) and kind == "source":
                            assert len(mine) == len(theirs), f"{op.name}: source count"
                        assert len(mine) == len(theirs), f"{op.name}: {kind} count"
                        for arg, was in zip(mine, theirs):
                            if isinstance(was, ir.Mem):
                                continue  # a Cell is the MemRef, not the encoding
                            assert back(arg, body.origin) == was, f"{op.name}: {kind} {arg} != {was}"
    assert seen > 2000, f"only {seen} operations checked"


def test_a_variable_keeps_one_name_across_every_version_of_it() -> None:
    """In MIR a register is a variable and nothing more.

    Numbering values `v56`, `v394`, `v400` said three different things where
    BC wrote one variable three times -- and at a loop header, where the
    raise puts a phi on every register live around the loop, six registers
    read as six unrelated variables. 250 of the suite's 454 phis are read by
    nothing but other phis: that is the register file describing itself, and
    it is unreadable while every version has its own name.
    """
    from collections import Counter

    from qbopt import mir
    from qbopt import omf
    from qbopt import module
    from qbopt import blocks as split
    from qbopt.blocks import code_map

    found = module.of(omf.parse(Path("fixtures/omf/hotlop-p-g2.obj").read_bytes()))
    assert found is not None
    mapped = code_map(found)
    assert not isinstance(mapped, str)

    seen = 0
    for _name, body in mir.bodies(found, split.partition(found, mapped)):
        # One variable per place BC kept something, and every value of it
        # says which variable it is.
        by_variable: dict[int, set] = {}
        for value, register in body.origin.items():
            by_variable.setdefault(value.variable, set()).add(register)
            assert value.version, f"{value!r} has no version"
        assert all(len(one) == 1 for one in by_variable.values()), "one variable stands for two registers"

        # And versions of one variable are consecutive from 1, so `v3_7` is
        # the seventh time that variable was written.
        counted = Counter(value.variable for value in body.origin)
        for which, count in counted.items():
            versions = {value.version for value in body.origin if value.variable is which}
            assert versions == set(range(1, count + 1)), f"variable {which}: {sorted(versions)}"
        seen += len(counted)
    assert seen, "no body named a variable, so this proves nothing"


def test_a_push_does_not_alias_a_global() -> None:
    """LLVM's PseudoSourceValue: an unnamed address still names its object.

    The raise names the slot a push lands in while it knows the stack
    depth, and gives up on the address when it does not -- but the push is
    still a push, and a push cannot land on a global whatever the depth.
    Two thirds of the corpus's pushes had no nameable slot and so aliased
    every named cell in their own body.
    """
    from qbopt.module import Space

    stack = mir.MemRef(addr=None, width=2, space=Space.STACK)
    glob = mir.MemRef(addr=Addr(Space.SEGMENT, 8), width=2)
    frame = mir.MemRef(addr=Addr(Space.FRAME, -4), width=2)
    blind = mir.MemRef(addr=None, width=2)

    assert not mir.overlapping(glob, stack, frozenset())
    assert not mir.overlapping(stack, glob, frozenset())
    # The stack and the frame are one region reached two ways.
    assert mir.overlapping(frame, stack, frozenset())
    # And an address in no known object still aliases everything.
    assert mir.overlapping(glob, blind, frozenset())
    assert mir.overlapping(stack, blind, frozenset())


def test_the_raise_says_which_object_a_push_reaches() -> None:
    """Measured: 413 of the corpus's references name an object and no byte."""
    from pathlib import Path

    from qbopt import omf
    from qbopt import module
    from qbopt.module import Space
    from qbopt import blocks as split
    from qbopt.blocks import code_map

    found = module.of(omf.parse(Path("fixtures/omf/lngmix-p-g2.obj").read_bytes()))
    blocks = split.partition(found, code_map(found))
    spaced = 0
    for _name, body in mir.bodies(found, blocks):
        for block in body.blocks:
            for op in block.ops:
                for one in (*op.loads, *op.stores):
                    if one.addr is None and one.where is Space.STACK:
                        spaced += 1
    assert spaced, "no push says it is on the stack"


def test_induction_finds_a_counter_and_what_it_derives() -> None:
    """`docs/targets.md` names what closes each program's gap, and
    induction variables come up in six of the thirteen -- more than
    anything else. matrix recomputes a row address from the counter every
    iteration with `imul word [w]`.

    Three things had to be right and each was wrong first: a cell can be
    loop-invariant (23 of 28 sites multiply by one), proving it needs the
    object bounds (an indexed store otherwise reads as writing every scalar
    in DGROUP), and the multiply itself loads (excluding anything that
    touches memory excluded every site this exists for).
    """
    from pathlib import Path

    from qbopt import omf
    from qbopt import module
    from qbopt import induction
    from qbopt import blocks as split
    from qbopt.blocks import code_map

    found = module.of(omf.parse(Path("fixtures/omf/matrix-p-g2.obj").read_bytes()))
    blocks = split.partition(found, code_map(found))
    bounds = module.landmarks(found)

    counters = reduced = 0
    for _name, body in mir.bodies(found, blocks):
        for _loop, basics, derived in induction.of(body, found.dgroup, bounds):
            counters += len(basics)
            reduced += len(derived)
            for one in basics.values():
                assert isinstance(one.step, (mir.Const, mir.Held)), "a step that is neither"
            for one in derived:
                assert one.op.kind in (mir.Kind.MUL, mir.Kind.SHL)
                assert not one.op.stores, "a multiply that stores is not a candidate"
    assert counters, "matrix counts; the analysis says otherwise"
    # Two, not one: matrix multiplies in both the inner loop and the outer.
    # A weaker analysis finds neither -- each of the three fixes above takes
    # this to zero on its own.
    assert reduced >= 2, f"matrix reduces {reduced}; it multiplies its counter by a width it never changes"


def test_a_multiply_by_something_the_loop_writes_is_not_reducible() -> None:
    """There is no recurrence to reduce if the multiplier moves."""
    from pathlib import Path

    from qbopt import omf
    from qbopt import module
    from qbopt import induction
    from qbopt import blocks as split
    from qbopt.blocks import code_map

    for name in ("matrix-p-g2", "hotlop-p-g2", "stride-p-g2"):
        found = module.of(omf.parse(Path(f"fixtures/omf/{name}.obj").read_bytes()))
        blocks = split.partition(found, code_map(found))
        for _who, body in mir.bodies(found, blocks):
            at_of = {block.at: block for block in body.blocks}
            for loop, _basics, derived in induction.of(body, found.dgroup, module.landmarks(found)):
                inside = {at for at in loop.body if at in at_of}
                wrote = [one for at in inside for op in at_of[at].ops for one in op.stores]
                for one in derived:
                    if isinstance(one.by, mir.Cell):
                        assert not any(
                            mir.overlapping(one.by.ref, other, found.dgroup, module.landmarks(found)) for other in wrote
                        ), f"{one.op.at:#06x} multiplies by a cell the loop writes"


def test_every_program_hands_the_runtime_an_address() -> None:
    """Which is why a whole-body escape test grants nothing.

    BC puts a program's variables in BC_DATA and the runtime keeps its
    buffers in BR_DATA -- different segments of one group -- so a runtime
    call writes the second and not the first *unless the program handed it
    a pointer*. That reads like a guarantee worth 47 cells.

    It is not: all 32 of the -p-g2 fixtures push a relocated immediate,
    which is what handing over an address looks like here. `lea` is not,
    and testing for `Operation.ADDRESS` found none of them.

    Asked per cell instead of per body, 59 cells have an address nothing
    takes. That analysis is sound and is not written; this test is here so
    that the whole-body version is not written again.
    """
    from pathlib import Path

    from qbopt import omf
    from qbopt import module
    from qbopt import blocks as split
    from qbopt.blocks import code_map

    clean = []
    for path in sorted(Path("fixtures/omf").glob("*-p-g2.obj"))[:8]:
        records = omf.parse(path.read_bytes())
        found = module.of(records)
        mapped = code_map(found)
        if isinstance(mapped, str):
            continue
        blocks = split.partition(found, mapped)
        fields = {one.offset for one in omf.fixups(records) if one.seg == found.seg}
        if not any(
            str(insn.insn).lower().startswith(("push", "lea")) and any(insn.at <= one < insn.end for one in fields)
            for block in blocks
            for insn in block.insns
        ):
            clean.append(path.stem)
    assert not clean, f"these hand out no address, so the whole-body test would grant something: {clean}"


def test_the_object_says_which_segment_holds_the_program_s_variables() -> None:
    """BC_DATA is the program's; BR_DATA and BR_SKYS are the runtime's."""
    from pathlib import Path

    from qbopt import omf
    from qbopt import module

    found = module.of(omf.parse(Path("fixtures/omf/matrix-p-g2.obj").read_bytes()))
    assert found.program_data is not None, "no segment is named BC_DATA"
    assert found.program_data in found.dgroup, "the program's data is not in DGROUP"


def _op_at(stem: str, at: int):
    from pathlib import Path

    from qbopt import omf
    from qbopt import module
    from qbopt import mir as raised
    from qbopt import blocks as split
    from qbopt.blocks import code_map

    found = module.of(omf.parse((Path("fixtures/omf") / f"{stem}.obj").read_bytes()))
    blocks = split.partition(found, code_map(found))
    for _name, body in raised.bodies(found, blocks):
        for block in body.blocks:
            for op in block.ops:
                if op.at == at:
                    return op, body
    return None, None


def test_an_add_that_carries_is_not_an_ordinary_add() -> None:
    """negnot printed D=-33752584 for -33818120, short by exactly 0x10000.

    `adc dx,0` propagates the borrow `neg ax` left into the high word.
    The raise folded `add` and `adc` to one kind, so nothing below could
    tell them apart -- and a lowering that re-encodes from MIR wrote a
    plain `add`, dropping the carry.
    """
    from qbopt import mir as raised

    op, _body = _op_at("negnot-q-O", 0xCE)
    assert op is not None, "the adc is not where this test thinks"
    assert op.kind is raised.Kind.ADD_CARRY, f"raised as {op.kind}"
    assert op.kind is not raised.Kind.ADD


def test_a_carrying_add_reads_the_flags_that_carry() -> None:
    """The carry is a value, not an adjacency: nothing may schedule
    between the operation that set it and the one that reads it without
    the dataflow saying so."""
    op, body = _op_at("negnot-q-O", 0xCE)
    assert op is not None
    flags = [one for one in op.uses if one.flags]
    assert flags, f"the carrying add reads no flags: {op.uses}"
    made = [other.at for block in body.blocks for other in block.ops for one in other.defines if one in flags]
    assert made, "nothing defines the flags it reads"


def test_what_an_operation_steps_by_is_asked_in_one_place() -> None:
    """`mir.stepping` is what a pass asks instead of listing kinds: the
    machine spells an affine step four ways and MIR should say one thing.
    """
    from qbopt import mir as raised

    def made(kind, args):
        return raised.Op(0, None, kind.value, (), (), (), (), None, kind=kind, args=args, results=())

    one = raised.Held(raised.Value(1, 0, 0, 1, 1), 2)
    other = raised.Held(raised.Value(2, 0, 0, 2, 1), 2)
    assert raised.stepping(made(raised.Kind.INCREMENT, (one,)))[1] == raised.Const(1, 2)
    assert raised.stepping(made(raised.Kind.DECREMENT, (one,)))[1] == raised.Const(-1, 2)
    assert raised.stepping(made(raised.Kind.SUB, (one, raised.Const(4, 2))))[1] == raised.Const(-4, 2)
    assert raised.stepping(made(raised.Kind.ADD, (one, other)))[1] == other
    # `x - y` for an invariant y is not `x + y`: a counter told otherwise
    # would run the wrong way.
    assert raised.stepping(made(raised.Kind.SUB, (one, other))) is None


def test_a_declared_call_raises_its_arguments_as_operands() -> None:
    """B$FILD takes a long in dx:ax, and said so nowhere an operand could
    be read from.

    Its `Contract.inputs` names ax and dx; the raise put them only in the
    conservative use set, alongside every register a call is assumed to
    read. An argument that is not an operand cannot carry a width, and the
    lowering had nothing to pair with the register the routine reads it in.
    """
    from pathlib import Path

    from qbopt import omf
    from qbopt import module
    from qbopt import runtime
    from qbopt import blocks as split
    from qbopt.blocks import code_map

    found = module.of(omf.parse(Path("fixtures/omf/fpemu-p-evt.obj").read_bytes()))
    blocks = split.partition(found, code_map(found))
    seen = []
    for _name, body in mir.bodies(found, blocks):
        for block in body.blocks:
            for op in block.ops:
                if op.kind is not mir.Kind.CALL:
                    continue
                if found.calls.get(op.at) == "B$FILD":
                    seen.append(op)
    assert seen, "no B$FILD call; the fixture cannot show this"
    assert runtime.slots(runtime.contract("B$FILD")) == (runtime.Reg.AX, runtime.Reg.DX)
    one = seen[0]
    assert len(one.args) == 2, f"the call raises {one.args} for two declared inputs"
    assert [x.width for x in one.args] == [2, 2], f"widths are {[x.width for x in one.args]}"


def _raised_calls(name: str):
    """Every CALL a checked-in object raises, by the routine it names.

    Through `mir.bodies`, not the helper: the question is whether the raise
    encodes the state, and a helper called directly cannot answer it.
    """
    from pathlib import Path

    from qbopt import omf
    from qbopt import module
    from qbopt import blocks as split
    from qbopt.blocks import code_map

    at = Path("fixtures/omf") / name
    assert at.exists(), f"{name} is checked in and this test needs it"
    found = module.of(omf.parse(at.read_bytes()))
    blocks = split.partition(found, code_map(found))
    out: dict[str, list] = {}
    for _who, body in mir.bodies(found, blocks):
        for block in body.blocks:
            for op in block.ops:
                if op.kind is mir.Kind.CALL:
                    out.setdefault(found.calls.get(op.at) or "(program)", []).append(op)
    return out


def test_a_call_whose_interface_is_unestablished_says_so_rather_than_empty() -> None:
    """`()` alone would say the routine reads nothing.

    B$ENRA's code is not in the runtime tree, so its contract declares no
    inputs -- which is not the same fact as an established empty set, and
    the difference is why constraining nothing let arrprm's `mov cx,0`
    move to another register.
    """
    from qbopt import runtime

    assert runtime.contract("B$ENRA").inputs is None
    # VBDOS's, where the `bx` path reaches a call no disassembly follows.
    made = _raised_calls("procs-v-evt.obj")
    assert "B$ENRA" in made, f"no B$ENRA raised; found {sorted(made)}"
    for op in made["B$ENRA"]:
        assert op.args == () and op.args_known is False, f"args={op.args} known={op.args_known}"


def test_a_call_established_to_read_nothing_says_that_instead() -> None:
    """B$PEI2 takes its argument on the stack, and that is established.

    The distinction this holds: an empty argument list with the flag set
    means "reads no register", which is a fact, and the unknown case above
    means nothing is known. Both are `args == ()`.
    """
    from qbopt import runtime

    assert runtime.contract("B$PEI2").inputs == frozenset()
    made = _raised_calls("addrm-p-evt.obj")
    assert "B$PEI2" in made, f"no B$PEI2 raised; found {sorted(made)}"
    for op in made["B$PEI2"]:
        assert op.args == () and op.args_known is True, f"args={op.args} known={op.args_known}"


def test_a_call_established_to_read_its_arguments_raises_them() -> None:
    """B$FILD takes a long in dx:ax, and its contract says so."""
    from qbopt import runtime

    assert runtime.slots(runtime.contract("B$FILD")) == (runtime.Reg.AX, runtime.Reg.DX)
    made = _raised_calls("fpemu-p-evt.obj")
    assert "B$FILD" in made, f"no B$FILD raised; found {sorted(made)}"
    for op in made["B$FILD"]:
        assert op.args_known is True, f"{op.at:#06x}: known={op.args_known}"
        assert [one.width for one in op.args] == [2, 2], f"{op.at:#06x}: {op.args}"
        read = {one.id for one in op.uses}
        assert all(one.value.id in read for one in op.args), f"{op.at:#06x}: {op.args} vs {op.uses}"


def test_an_unknown_calls_read_set_and_its_flag_survive_a_pass() -> None:
    """The conservative uses keep a live value from being discarded, and
    `args_known` has to reach the lowering."""
    from qbopt import transform

    made = _raised_calls("procs-v-evt.obj")
    op = made["B$ENRA"][0]
    assert op.uses, "the call reads nothing, so preservation proves nothing"
    body = mir.MirBody(op.at, (mir.MirBlock(op.at, (), (op,), ()),), {}, {})
    after = transform.widened(body)
    out = next(one for block in after.blocks for one in block.ops)
    assert out.args_known is False, "the flag did not survive the pass"
    assert {one.id for one in out.uses} == {one.id for one in op.uses}, f"the read set became {out.uses}"


def test_the_entry_routine_declares_its_frame_size_where_that_is_established() -> None:
    """B$ENRA takes the frame size in cx, and its own code says so.

    Disassembled from the linked image for each toolchain: PDS 7.1 at
    0x1d35 and QuickBASIC 4.5 at 0x211d read cx before writing it, through
    `push cx` and `sub sp,cx`, with no unresolved edge on any path. VBDOS
    also reads bx -- `or bx,bx` gates a further far call -- but that call
    reaches `call far [di+24h]`, which no disassembly can follow, so
    nothing is established for it.
    """
    from qbopt import module
    from qbopt import runtime

    made = _raised_calls("procs-p-g2.obj")
    assert "B$ENRA" in made, f"no B$ENRA raised; found {sorted(made)}"
    for op in made["B$ENRA"]:
        assert op.args_known is True, f"{op.at:#06x}: known={op.args_known}"
        assert [one.width for one in op.args] == [2], f"{op.at:#06x}: {op.args}"

    # QuickBASIC 4.5 is established the same way, on the selector rather
    # than on a fixture: no checked-in q object calls B$ENRA.
    assert runtime.per_call({0: "B$ENRA"}, module.Family.QUICKBASIC)[0].inputs == frozenset({runtime.Reg.CX})
    assert runtime.per_call({0: "B$ENRA"}, module.Family.PDS)[0].inputs == frozenset({runtime.Reg.CX})
    assert runtime.per_call({0: "B$ENRA"}, module.Family.VBDOS)[0].inputs is None
    assert runtime.per_call({0: "B$ENRA"}, module.Family.UNKNOWN)[0].inputs is None

    # And where nothing is established, nothing is claimed.
    from pathlib import Path

    from qbopt import omf

    assert module.family(omf.parse(Path("fixtures/omf/procs-v-evt.obj").read_bytes())) is module.Family.VBDOS
    theirs = _raised_calls("procs-v-evt.obj")
    assert "B$ENRA" in theirs
    for op in theirs["B$ENRA"]:
        assert op.args_known is False, f"{op.at:#06x}: VBDOS claimed an interface"
    assert runtime.contract("B$ENRA").inputs is None, "the nameless contract still declares nothing"


def test_a_procedure_this_module_defines_is_not_a_runtime_routine() -> None:
    """`REPORT` and `TWICE` reach `Module.calls` like B$ENRA does.

    BC compiles a SUB as a PUBDEF of the same object and calls it through
    an EXTDEF fixup, so the call target alone cannot say which it is. The
    PUBDEF set can, and nothing else may: a name is not evidence.
    """
    from pathlib import Path

    from qbopt import omf
    from qbopt import module
    from qbopt import runtime

    records = omf.parse(Path("fixtures/omf/procs-p-evt.obj").read_bytes())
    found = module.of(records)
    mine = module.defines(records, found.seg)
    assert {"REPORT", "TWICE"} <= mine, f"this module defines {sorted(mine)[:6]}"

    where = {at: name for at, name in found.calls.items() if name in ("REPORT", "B$ENRA")}
    assert len(set(where.values())) == 2, f"the fixture needs both kinds: {where}"
    got = runtime.per_call(found.calls, module.family(records), mine)
    for at, name in where.items():
        one = got[at]
        if name == "REPORT":
            assert one.inputs == frozenset(), f"{name} claims register inputs {one.inputs}"
            assert one.clobbers == runtime.EVERY, f"{name} narrowed its clobbers"
            assert one.established is False, f"{name} claims to be established"
        else:
            # The runtime's own, and under PDS its frame size is established.
            assert one.inputs == frozenset({runtime.Reg.CX}), f"{name} reads {one.inputs}"
            assert one.name == "B$ENRA", f"it was selected as {one.name}"
            assert "PUBDEF of this same module" not in one.evidence, one.evidence

    # And the same name without a PUBDEF behind it is not selected.
    assert runtime.per_call({0: "REPORT"}, module.Family.PDS, frozenset())[0].inputs is None


def test_the_exit_routine_declares_what_it_reads_where_that_is_established() -> None:
    """B$EXSA's returning path reads no register at all.

    Its other path is error dispatch, and what is declared there is the
    survivor set rather than a read set: ax and bx are written before
    every terminal, cx, dx, si and di are not. A superset costs a copy
    where it is wrong and cannot be unsound. QuickBASIC 4.5's reads
    nothing on any path.
    """
    from qbopt import module
    from qbopt import runtime

    every = frozenset({runtime.Reg.CX, runtime.Reg.DX, runtime.Reg.SI, runtime.Reg.DI})
    assert runtime.per_call({0: "B$EXSA"}, module.Family.PDS)[0].inputs == every
    assert runtime.per_call({0: "B$EXSA"}, module.Family.QUICKBASIC)[0].inputs == frozenset()
    assert runtime.per_call({0: "B$EXSA"}, module.Family.VBDOS)[0].inputs is None

    made = _raised_calls("procs-p-g2.obj")
    assert "B$EXSA" in made, f"no B$EXSA raised; found {sorted(made)}"
    for op in made["B$EXSA"]:
        assert op.args_known is True, f"{op.at:#06x}: known={op.args_known}"
        assert [one.width for one in op.args] == [2, 2, 2, 2], f"{op.at:#06x}: {op.args}"
