"""
qbopt/analysis/avail.py's own gate.

The counts here are canaries, not specifications: the point of each is that
it moved for a reason someone can name. The important one is the last --
nbody, the benchmark this pass exists to speed up, has no forwardable load
at all, and knowing that is what stops a transform being built for it.
"""

from pathlib import Path

import pytest

import corpus
from qbopt.model import ir
from qbopt.model import mir
from qbopt.analysis import avail
from qbopt.analysis import liveness
from qbopt.model.mir import MirBody
from qbopt.objectfile import module
from qbopt.objectfile.module import Addr
from qbopt.objectfile.module import Space
from qbopt.frontend.blocks import code_map
from qbopt.frontend import blocks as blockmod

FIXTURES = sorted(Path("fixtures/omf").glob("*.obj"))


@pytest.mark.parametrize("real_call", [False, True])
@pytest.mark.parametrize("metadata", [False, True])
def test_current_mir_decides_whether_a_call_invalidates_memory(real_call, metadata):
    """Removed HARY sites killed frame facts; real unknown calls must still invalidate them."""
    ref = mir.MemRef(Addr(Space.FRAME, -20), 2)
    value = mir.Value(1, 0)
    op = mir.Op(
        10,
        ir.Operation.CALL if real_call else ir.Operation.NOTHING,
        "",
        (),
        (),
        kind=mir.Kind.CALL if real_call else mir.Kind.NOTHING,
    )
    held = {ref: value}
    calls = {10: "B$HARY"} if metadata else {}
    assert avail._after(op, held, frozenset(), calls) == ({} if real_call else held)


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
        alive = liveness.live(body)
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


def test_a_runtime_name_does_not_replace_missing_effect_proofs() -> None:
    found = corpus.loaded(Path("fixtures/omf/divmod-p-evt.obj"))
    assert found is not None
    for body in bodies_of(found, corpus.partitioned(Path("fixtures/omf/divmod-p-evt.obj"))):
        for block in body.blocks:
            for op in block.ops:
                if found.calls.get(op.at) == "B$MUI4":
                    ref = mir.MemRef(Addr(Space.FRAME, -20), 2)
                    held = {ref: mir.Value(1, 0)}
                    assert avail._after(op, held, found.dgroup, found.calls) == {}
                    return
    raise AssertionError("no B$MUI4 call in this fixture -- it is what the test is about")


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
    from qbopt.objectfile.module import Space

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
    from qbopt.objectfile.module import Space

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
    from qbopt.model import ir
    from qbopt.frontend import declen
    from qbopt.objectfile.module import Addr
    from qbopt.objectfile.module import Space

    def somewhere(*_args: object, **_kwargs: object) -> Addr:
        return Addr(Space.LITERAL, 0, 0)

    def semantics(hexs: str) -> ir.Semantics:
        insn = declen.decode(bytes.fromhex(hexs), 0)
        assert insn is not None
        return ir.instruction_semantics(insn, somewhere)

    made = semantics("2b060000")  # sub ax,[x]
    moved = semantics("a1000000")  # mov ax,[x]
    assert made.dests[0] in made.sources, "a subtract reads its own destination"
    assert moved.dests[0] not in moved.sources, "a move does not"
    assert made.op is not ir.Operation.MOVE
    assert moved.op is ir.Operation.MOVE


@pytest.mark.parametrize("obj", FIXTURES, ids=lambda p: p.stem)
def test_nothing_redundant_is_an_accumulate(obj: Path) -> None:
    """Corpus-wide: every deletion redundant() proposes is a move."""
    from qbopt.model import ir
    from qbopt.model import mir
    from qbopt.frontend.blocks import code_map

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
                from qbopt.backend import lower

                what = lower.current(op)
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

    from qbopt.model import ir

    old = mir.Value(1, 0x100)
    new = mir.Value(2, 0x100)
    cell = mir.MemRef(addr=Addr(Space.LITERAL, 0, 0), width=2)
    where = ir.Mem(addr=Addr(Space.LITERAL, 0, 0), width=2)
    into = ir.Reg(register=Register.AX, width=2)

    def op(what: ir.Semantics, args, results) -> mir.Op:
        return mir.Op(
            at=0x100,
            op=what.op,
            name=what.name or "",
            defines=(new,),
            uses=(old,),
            loads=(cell,),
            kind=mir._kind_of(what, args, results),
            args=args,
            results=results,
            raised=((), ()),
        )

    # In MIR's own operands: a load's only argument is the cell, so the use
    # of the old value is a preserved half; a subtract names it as an input
    # and it is not. This asked the instruction whether it was a MOVE and
    # looked the register up in `origin`.
    moved = op(
        ir.Semantics(ir.Operation.MOVE, "mov", dests=(into,), sources=(where,)),
        (mir.Cell(cell),),
        (mir.Held(new, 2),),
    )
    assert avail._preserved(moved) == {old}, "a move's read of its own destination is the high half"

    accumulated = op(
        ir.Semantics(ir.Operation.BINARY, "sub", dests=(into,), sources=(into, where)),
        (mir.Held(old, 2), mir.Cell(cell)),
        (mir.Held(new, 2),),
    )
    assert avail._preserved(accumulated) == set(), "a subtract reads its destination as data"
    assert avail.loaded_into(accumulated) is None, "and so is not a load"


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
    from qbopt.analysis import avail as under_test

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


def _defined(body) -> set[int]:
    """Every value some operation or phi in this body writes."""
    out = set()
    for block in body.blocks:
        out.update(phi.result.id for phi in block.phis)
        for op in block.ops:
            out.update(one.id for one in op.defines)
    return out


def _read(body) -> dict[int, str]:
    """Every value something reads, and where, by all four routes in.

    A value reaches a reader as a `use`, as a `Held` operand, as the base
    or segment of a memory operand, or along a phi's incoming edge. A
    deletion that forgets any of them leaves a use with no definition.
    """
    from qbopt.model import mir as form

    out: dict[int, str] = {}
    for block in body.blocks:
        for phi in block.phis:
            for at, one in phi.incoming.items():
                out.setdefault(one.id, f"the phi at {block.at:#06x}, along the edge from {at:#06x}")
        for op in block.ops:
            for one in op.uses:
                out.setdefault(one.id, f"{op.at:#06x}")
            for one in op.args:
                if isinstance(one, form.Held):
                    out.setdefault(one.value.id, f"{op.at:#06x}, as an operand")
            for ref in (*op.loads, *op.stores):
                for one in (ref.base, ref.segment):
                    if one is not None:
                        out.setdefault(one.id, f"{op.at:#06x}, reaching memory")
    return out


def _orphaned(before, after) -> list[str]:
    """Values that had a definition before the pass and have none after.

    Asked as a difference on purpose. "Used and defined nowhere" is also
    the description of a value the caller supplied -- `liveness.entry_values`
    is exactly that set -- so the absolute question cannot tell a genuine
    entry value from one whose definition a pass deleted. The change can.
    """
    had, has = _defined(before), _defined(after)
    reads = _read(after)
    return [
        f"v{one} is read by {reads[one]} and its definition was deleted" for one in sorted(had - has) if one in reads
    ]


@pytest.mark.parametrize("obj", ["arridx-p-g2", "arridx-q-O", "arridx-v-g2"])
def test_dropping_a_redundant_load_leaves_no_use_without_a_definition(obj: str) -> None:
    """arridx: `mov [x],ax` then `mov ax,[x]`, and the reload deleted.

    The load's own result was the value every later instruction read. The
    deletion removed the only definition of it and substituted nothing, so
    the add after it still named a value nobody wrote -- and allocation,
    having no constraint to honour, put it in bx and emitted `mov ax,bx`
    where the product was already in ax.

    Measured through tools/matrix.py: arridx miscompiles in all nine
    ordinary configurations. VBDOS answers -26096, PDS 0, QuickBASIC 4.5
    11008, where the program prints 1260.
    """
    from pathlib import Path

    from qbopt.model import mir
    from qbopt.objectfile import omf
    from qbopt.objectfile import module
    from qbopt.optimize import transform
    from qbopt.frontend import blocks as split
    from qbopt.frontend.blocks import code_map

    path = Path(f"fixtures/omf/{obj}.obj")
    if not path.exists():
        pytest.skip(f"no {obj} fixture")
    found = module.of(omf.parse(path.read_bytes()))
    mapped = code_map(found)
    assert not isinstance(mapped, str)
    blocks = split.partition(found, mapped)

    for name, body in mir.bodies(found, blocks):
        after = transform.applied(body, found.dgroup, found.calls, blocks=blocks, found=found, only="drop_loads")
        broken = _orphaned(body, after)
        assert not broken, f"{obj} {name}: " + "; ".join(broken[:3])
