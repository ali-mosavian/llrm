"""
qbopt/avail.py's own gate.

The counts here are canaries, not specifications: the point of each is that
it moved for a reason someone can name. The important one is the last --
nbody, the benchmark this pass exists to speed up, has no forwardable load
at all, and knowing that is what stops a transform being built for it.
"""

from pathlib import Path

import pytest

import corpus
from qbopt import ir
from qbopt import mir
from qbopt import avail
from qbopt import memory
from qbopt import module
from qbopt import regalloc
from qbopt import blocks as blockmod
from qbopt.blocks import code_map
from qbopt.mir import MirBody
from qbopt.module import Addr
from qbopt.module import Space

FIXTURES = sorted(Path("fixtures/omf").glob("*.obj"))


def bodies_of(found: module.Module, found_blocks: list) -> list[mir.MirBody]:
    result = ir.decode_module(found)
    if isinstance(result, str):
        return []
    nodes = {ir.span(n)[0]: n for body in result for n in body.nodes}
    out = []
    for body in result:
        mine = [b for b in found_blocks if any(lo <= b.at < hi for lo, hi in body.body.ranges)]
        if not mine:
            continue
        built = mir.raise_body(mine, nodes, body.body.seed, found.calls)
        if not isinstance(built, str):
            out.append(built)
    return out


def split(obj: Path) -> dict[str, int]:
    """Every redundant read, by whether a live value holds its bytes."""
    found = corpus.loaded(obj)
    assert found is not None
    found_blocks = corpus.partitioned(obj)
    reported = memory.redundant_loads(found_blocks, found.resolve, found.calls, found.dgroup)
    want = {at for ats in reported.values() for at in ats}

    tally = {"live": 0, "dead": 0, "none": 0}
    for body in bodies_of(found, found_blocks):
        held = avail.holders(body, found.dgroup, found.calls)
        alive = regalloc.live(body)
        for block in body.blocks:
            current = dict(held.into[block.at])
            after = set(alive.live_out[block.at])
            at_point: dict[int, frozenset] = {}
            for op in reversed(block.ops):
                at_point[op.at] = frozenset(after)
                after = (after - set(op.defines)) | set(op.uses)
            for op in block.ops:
                if op.at in want and op.loads:
                    who = next(
                        (w for c, w in current.items() if mir.same_bytes(c, op.loads[0])),
                        None,
                    )
                    if who is None:
                        tally["none"] += 1
                    elif who in at_point.get(op.at, ()):
                        tally["live"] += 1
                    else:
                        tally["dead"] += 1
                current = avail._after(op, current, found.dgroup, found.calls)
    return tally


@pytest.mark.parametrize("obj", FIXTURES, ids=lambda p: p.stem)
def test_the_map_reaches_a_fixed_point(obj: Path) -> None:
    """Intersection at joins is monotone, so it terminates -- and running it
    twice has to give the same answer."""
    found = corpus.loaded(obj)
    assert found is not None
    for body in bodies_of(found, corpus.partitioned(obj)):
        once = avail.holders(body, found.dgroup, found.calls)
        twice = avail.holders(body, found.dgroup, found.calls)
        assert once.into == twice.into
        assert once.outof == twice.outof


@pytest.mark.parametrize("obj", FIXTURES, ids=lambda p: p.stem)
def test_no_entry_survives_a_store_that_could_reach_it(obj: Path) -> None:
    """The kill is the only thing keeping this sound.

    A call is the one exception, and the reason it is one is runtime.py: a
    routine established to write no caller memory keeps the map, even though
    mir.py gives every call a store of MemRef(addr=None) that aliases
    everything. That is not this rule being bent -- the store is the default
    for a callee nothing is known about, and knowing something is what
    replaces it. B$MUI4 has always been such a routine; the fpemu fixtures
    added six more, and are what first put a surviving entry across one.
    """
    found = corpus.loaded(obj)
    assert found is not None
    for body in bodies_of(found, corpus.partitioned(obj)):
        held = avail.holders(body, found.dgroup, found.calls)
        for block in body.blocks:
            current = dict(held.into[block.at])
            for op in block.ops:
                clean = op.at in found.calls and avail._clean(op, found.calls)
                current = avail._after(op, current, found.dgroup, found.calls)
                if clean:
                    continue
                for ref in op.stores:
                    kept = avail.stored_from(op)
                    for cell in current:
                        if kept is not None and cell == kept[0]:
                            continue
                        assert not mir.overlapping(cell, ref, found.dgroup), (
                            f"{obj.stem} {op.at:#x}: {cell} survived a store to {ref}"
                        )


def test_an_accumulate_is_not_a_provider() -> None:
    """`and cx,[x]` leaves cx holding cx-and-the-cell, not the cell.

    Refused by loaded_into() reading op.uses: the and reads cx as data.
    forward.py shipped without this check and tools/matrix.py caught it on
    nine of twelve real-compiler configurations.
    """
    found = corpus.loaded(Path("fixtures/omf/arith-v-plain.obj"))
    assert found is not None
    seen = 0
    for body in bodies_of(found, corpus.partitioned(Path("fixtures/omf/arith-v-plain.obj"))):
        for block in body.blocks:
            for op in block.ops:
                if op.at in (0x123, 0x127):
                    seen += 1
                    assert avail.loaded_into(op) is None, f"{op.name} at {op.at:#x} read as a load"
    assert seen == 2, "the two accumulate sites are gone from the fixture"


def test_a_memory_clean_call_does_not_wipe_the_map() -> None:
    """B$MUI4 multiplies two longs in registers and touches no caller memory.

    mir.py gives every call a store of MemRef(addr=None), which aliases
    everything -- the right default, and wrong for the routines runtime.py
    has actually read. Without this, two of nbody's reloads report "no
    provider" when the truth is "the provider is dead", which are different
    findings: one is a modelling gap and the other is a spill.
    """
    found = corpus.loaded(Path("fixtures/omf/divmod-p-evt.obj"))
    assert found is not None
    for body in bodies_of(found, corpus.partitioned(Path("fixtures/omf/divmod-p-evt.obj"))):
        for block in body.blocks:
            for op in block.ops:
                if found.calls.get(op.at) == "B$MUI4":
                    assert avail._clean(op, found.calls)
                    return
    raise AssertionError("no B$MUI4 call in this fixture -- it is what the test is about")


def test_the_corpus_split_is_what_was_measured() -> None:
    """73 reads have a live value holding their bytes, 48 a dead one.

    A canary. If it moves, something changed the join and the reason should
    be nameable before this number is edited. It has moved twice, both times
    for float code and neither time in the live count -- a reload whose
    provider is on the x87 stack is not one a register can serve.

    The fpemu fixtures took 42 dead to 48 and 366 with no provider to 408.
    Then 87bhelp.asm's contracts took the total from 529 to 553: knowing
    that B$FILD and the rest write no caller memory means a cell established
    before one is still that value after, so memory.py finds 24 reads
    redundant that it used to give up on at the call.

    And a third time, in the live count as well: a frame or stack slot no
    longer aliases a named variable, so a cell established before a `push`
    survives it. 1,805 cells to 1,952, live 96 to 108.
    """
    total = {"live": 0, "dead": 0, "none": 0}
    for obj in FIXTURES:
        for key, count in split(obj).items():
            total[key] += count
    assert total == {"live": 108, "dead": 402, "none": 1442}


@pytest.mark.parametrize("obj", FIXTURES, ids=lambda p: p.stem)
def test_a_live_provider_is_never_the_value_the_read_defines(obj: Path) -> None:
    """A read cannot be its own provider -- that would forward it to itself."""
    found = corpus.loaded(obj)
    assert found is not None
    for body in bodies_of(found, corpus.partitioned(obj)):
        held = avail.holders(body, found.dgroup, found.calls)
        for block in body.blocks:
            current = dict(held.into[block.at])
            for op in block.ops:
                if op.loads:
                    who = next((w for c, w in current.items() if mir.same_bytes(c, op.loads[0])), None)
                    assert who is None or who not in op.defines
                current = avail._after(op, current, found.dgroup, found.calls)


@pytest.mark.parametrize("obj", FIXTURES, ids=lambda p: p.stem)
def test_no_stack_slot_crosses_a_block_boundary(obj: Path) -> None:
    """A Space.STACK address is a depth, not an address.

    `[sp-8]` measured from the top of one block and `[sp-8]` measured from
    the top of another are different bytes that compare equal, so carrying
    one across an edge is the single way this analysis could be unsound.
    """
    from qbopt.module import Space

    found = corpus.loaded(obj)
    assert found is not None
    for body in bodies_of(found, corpus.partitioned(obj)):
        held = avail.holders(body, found.dgroup, found.calls)
        for block in body.blocks:
            if block.at == body.entry:
                continue
            for cell in held.into[block.at]:
                assert cell.addr is None or cell.addr.space is not Space.STACK, (
                    f"{obj.stem} {block.at:#x}: {cell} arrived from another block"
                )


@pytest.mark.parametrize("obj", FIXTURES, ids=lambda p: p.stem)
def test_a_stack_slot_never_survives_a_call(obj: Path) -> None:
    """Even one runtime.py proves memory-clean.

    "Writes no caller memory" is a claim about the caller's variables. A
    call is entered by pushing a return address and the callee pops its own
    arguments, so the scratch below sp is gone either way.
    """
    from qbopt.module import Space

    found = corpus.loaded(obj)
    assert found is not None
    for body in bodies_of(found, corpus.partitioned(obj)):
        held = avail.holders(body, found.dgroup, found.calls)
        for block in body.blocks:
            current = dict(held.into[block.at])
            for op in block.ops:
                current = avail._after(op, current, found.dgroup, found.calls)
                if op.at not in found.calls:
                    continue
                for cell in current:
                    assert cell.addr is None or cell.addr.space is not Space.STACK, (
                        f"{obj.stem} {op.at:#x}: {cell} survived a call"
                    )


def test_an_accumulate_is_not_a_load_however_its_values_look() -> None:
    """`sub ax,[x]` reads the old ax as data; `mov ax,[x]` does not.

    Both are one use whose origin is the destination's own register, so
    values alone cannot tell them apart -- which is how allowing the
    preserved high half of a narrow write let an accumulate through.
    redundant() then deleted `sub ax,ds:[0]` and `adc dx,[si+2]` from a
    generated program.

    The semantics can tell them apart: a binary operation names its
    destination among its sources and a move does not. This is the same
    thing forward._loads_only exists to stop, arrived at from the other
    side.
    """
    from qbopt import declen
    from qbopt import ir
    from qbopt.module import Addr
    from qbopt.module import Space

    def somewhere(*_args: object, **_kwargs: object) -> Addr:
        return Addr(Space.LITERAL, 0, 0)

    def semantics(hexs: str) -> ir.Semantics:
        insn = declen.decode(bytes.fromhex(hexs), 0)
        assert insn is not None
        return ir.instruction_semantics(insn, somewhere)

    made = semantics("2b060000")   # sub ax,[x]
    moved = semantics("a1000000")  # mov ax,[x]
    assert made.dests[0] in made.sources, "a subtract reads its own destination"
    assert moved.dests[0] not in moved.sources, "a move does not"
    assert made.op is not ir.Operation.MOVE
    assert moved.op is ir.Operation.MOVE


@pytest.mark.parametrize("obj", FIXTURES, ids=lambda p: p.stem)
def test_nothing_redundant_is_an_accumulate(obj: Path) -> None:
    """Corpus-wide: every deletion redundant() proposes is a move."""
    from qbopt import ir
    from qbopt import mir
    from qbopt import blocks as split
    from qbopt.blocks import code_map

    found = corpus.loaded(obj)
    assert found is not None
    mapped = code_map(found)
    if isinstance(mapped, str):
        return
    for _name, body in mir.bodies(found, blockmod.partition(found, mapped)):
        gone = set(avail.redundant(body, found.dgroup, found.calls))
        for block in body.blocks:
            for op in block.ops:
                if op.at not in gone:
                    continue
                what = op.made if op.made is not None else getattr(op.node, "semantics", None)
                assert what is not None and what.op is ir.Operation.MOVE, (
                    f"{obj.stem} {op.at:#x}: {op.name} is not a move and was proposed for deletion"
                )


def test_preserved_allows_a_move_and_refuses_a_binary() -> None:
    """The guard itself, on the two shapes it has to separate.

    Built here rather than found in a fixture, because no fixture has the
    shape: `sub ax,[x]` reaching redundant() needs the memory map to hold
    that cell, which only happens in longer code than the suite writes. The
    corpus test above is the invariant; this is what discriminates.
    """
    from iced_x86 import Register

    from qbopt import ir

    old = mir.Value(1, 0x100)
    new = mir.Value(2, 0x100)
    origin = {old: Register.EAX, new: Register.EAX}
    cell = mir.MemRef(addr=Addr(Space.LITERAL, 0, 0), width=2)
    where = ir.Mem(addr=Addr(Space.LITERAL, 0, 0), width=2)
    into = ir.Reg(register=Register.AX, width=2)

    def op(what: ir.Semantics) -> mir.Op:
        return mir.Op(
            at=0x100,
            op=what.op,
            name=what.name or "",
            defines=(new,),
            uses=(old,),
            loads=(cell,),
            made=what,
        )

    moved = op(ir.Semantics(ir.Operation.MOVE, "mov", dests=(into,), sources=(where,)))
    assert avail._preserved(moved, new, origin) == {old}, "a move's read of its own destination is the high half"

    accumulated = op(ir.Semantics(ir.Operation.BINARY, "sub", dests=(into,), sources=(into, where)))
    assert avail._preserved(accumulated, new, origin) == set(), "a subtract reads its destination as data"
    assert avail.loaded_into(accumulated, origin) is None, "and so is not a load"


@pytest.mark.parametrize("obj", FIXTURES, ids=lambda p: p.stem)
def test_a_partial_write_is_recorded_only_where_it_is_asked_for(obj: Path) -> None:
    """`holders(partial=True)` sees more cells, and only redundant() may.

    `mov ax,[x]` writes sixteen bits of a thirty-two bit variable, so the
    value holding that cell holds it in its *low half*. That is exactly
    right for deciding the load is a no-op -- the instruction changes
    nothing, so the high half is preserved either way -- and wrong for
    serving some other read from that register, which is what
    rewrite._substituted does with the same map.

    Letting it leak made a generated program read the wrong cell. The
    switch is the whole fix, so the two answers have to stay different
    wherever a partial write exists at all.
    """
    found = corpus.loaded(obj)
    assert found is not None
    mapped = code_map(found)
    if isinstance(mapped, str):
        return
    for _name, body in mir.bodies(found, blockmod.partition(found, mapped)):
        strict = avail.holders(body, found.dgroup, found.calls)
        loose = avail.holders(body, found.dgroup, found.calls, partial=True)
        for block in body.blocks:
            a, b = strict.outof[block.at], loose.outof[block.at]
            assert set(a) <= set(b), "asking for partial writes may only add cells, never remove one"


def test_only_redundant_asks_the_map_for_partial_writes(monkeypatch: pytest.MonkeyPatch) -> None:
    """Which caller asks for what, recorded rather than grepped for.

    A cell established by `mov ax,[x]` is held in its value's *low half*.
    Deleting that load is sound -- the instruction changes nothing, so the
    high half is preserved either way -- and serving some other read from
    the whole register is not. rewrite._substituted uses forwardable(), so
    a leak there reads the wrong bytes, and a generated program did.

    The switch is the entire fix, so this pins who turns it on.
    """
    from qbopt import avail as under_test

    obj = FIXTURES[0]
    found = corpus.loaded(obj)
    assert found is not None
    mapped = code_map(found)
    assert not isinstance(mapped, str)
    body = next(b for _n, b in mir.bodies(found, blockmod.partition(found, mapped)))

    asked: list[bool] = []
    real = under_test.holders

    def watch(
        body: MirBody, dgroup: frozenset[int], calls: dict[int, str] | None = None, partial: bool = False
    ) -> avail.Held:
        asked.append(partial)
        return real(body, dgroup, calls, partial)

    monkeypatch.setattr(under_test, "holders", watch)
    asked.clear()
    under_test.redundant(body, found.dgroup, found.calls)
    assert asked == [True], f"redundant() asked {asked}"

    asked.clear()
    under_test.forwardable(body, found.dgroup, found.calls, frozenset())
    assert asked == [False], f"forwardable() asked {asked} -- it serves reads from the register"

    asked.clear()
    read = next((one for block in body.blocks for one in block.ops if one.loads), None)
    assert read is not None, "this body reads no memory, so provider() would not be asked"
    under_test.provider(body, found.dgroup, 0, read.loads[0], found.calls)
    assert asked == [False], f"provider() asked {asked}"
