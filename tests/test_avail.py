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
    """The kill is the only thing keeping this sound."""
    found = corpus.loaded(obj)
    assert found is not None
    for body in bodies_of(found, corpus.partitioned(obj)):
        held = avail.holders(body, found.dgroup, found.calls)
        for block in body.blocks:
            current = dict(held.into[block.at])
            for op in block.ops:
                current = avail._after(op, current, found.dgroup, found.calls)
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

    A canary. If it moves, something changed the join and the reason
    should be nameable before this number is edited. It last moved when the
    fpemu fixtures arrived: 42 dead became 48 and 366 with no provider
    became 408, all of it float code, and the live count did not move at all
    -- a reload whose provider is on the x87 stack is not one a register can
    serve.
    """
    total = {"live": 0, "dead": 0, "none": 0}
    for obj in FIXTURES:
        for key, count in split(obj).items():
            total[key] += count
    assert total == {"live": 73, "dead": 48, "none": 408}


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
