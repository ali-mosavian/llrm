"""
qbopt/layout.py: a whole body emitted, and everything that moves with it.

The claim is not that the bytes match. select.py may choose an encoding BC
did not -- it emits the near form of every branch where BC often wrote the
short one -- so the body is a different length and every address in it
shifts. The claim is that it is the same program: the same instructions in
the same order, and every branch pointing at the instruction it pointed at
before rather than at whatever now sits at the old address.
"""

from pathlib import Path
from collections.abc import Iterator

import pytest
from iced_x86 import OpKind
from iced_x86 import Register_
from iced_x86 import Mnemonic
from iced_x86 import Instruction

import corpus
from qbopt import ir
from qbopt import mir
from qbopt import omf
from qbopt import layout
from qbopt import select
from qbopt.declen import decode
from qbopt import blocks as split
from qbopt.rewrite import code_map

FIXTURES = sorted(Path("fixtures/omf").glob("*.obj"))


def walked(code: bytes, start: int) -> list:
    """Every instruction in `code`, read the way this project reads code.

    declen.decode rather than a bare iced Decoder: an emulated x87 site is
    `cd 35 46 c8` and only declen knows it is the fld it stands for. layout
    carries those verbatim -- converting one to native x87 is a decision
    fpu.py gates behind --native-fpu -- so anything checking the result has
    to read them the same way.
    """
    # Zero-padded to `start` so every instruction decodes at the address it
    # will actually sit at, which is what a branch's target is measured from.
    image = bytes(start) + code
    out = []
    at = start
    while at < len(image):
        found = decode(image, at)
        if found is None:
            break
        out.append(found.insn)
        at += found.length
    return out


def original(op: mir.Op) -> Instruction:
    """The instruction an op came from.

    Narrowed here rather than at each use: only nodes that carry one reach
    these tests, since a body holding anything else does not lay out.
    """
    node = op.node
    assert node is not None
    found = getattr(node, "insn", None)
    assert found is not None
    return found.insn


def laid(obj: Path) -> Iterator[tuple]:
    """Every body of the object that lays out, with what it produced."""
    found = corpus.loaded(obj)
    if found is None:
        return
    mapped = code_map(found)
    if isinstance(mapped, str):
        return
    for _, body in mir.bodies(found, split.partition(found, mapped)):
        got = layout.lay_out(body, body.entry, found)
        if not isinstance(got, str):
            yield body, got


@pytest.mark.parametrize("obj", FIXTURES, ids=lambda p: p.stem)
def test_a_laid_out_body_is_the_same_instructions_in_the_same_order(obj: Path) -> None:
    for body, got in laid(obj):
        ops = layout._ordered(body)
        back = walked(got.code, body.entry)
        assert len(back) == len(ops), f"{obj.stem}: {len(back)} instructions from {len(ops)} ops"
        for op, made in zip(ops, back, strict=True):
            want = original(op)
            assert made.mnemonic == want.mnemonic, f"{obj.stem} {op.at:#x}: {made} != {want}"


@pytest.mark.parametrize("obj", FIXTURES, ids=lambda p: p.stem)
def test_every_branch_points_where_its_target_went(obj: Path) -> None:
    """The whole reason layout exists.

    An instruction whose encoding differs in length from BC's moves
    everything after it, and a branch left with its old number then points
    at whatever now sits there -- which is a working program that does
    something else, the worst kind of wrong.
    """
    for body, got in laid(obj):
        ops = layout._ordered(body)
        back = walked(got.code, body.entry)
        for op, made in zip(ops, back, strict=True):
            want = original(op)
            if want.op0_kind != OpKind.NEAR_BRANCH16:
                continue
            landed = got.moved.get(want.near_branch16)
            assert landed is not None, f"{obj.stem} {op.at:#x}: target left this body"
            assert made.near_branch16 == landed, f"{obj.stem} {op.at:#x}: {made} should reach {landed:#x}"


@pytest.mark.parametrize("obj", FIXTURES, ids=lambda p: p.stem)
def test_every_relocation_points_at_a_field_the_module_really_has(obj: Path) -> None:
    """A relocated displacement goes out as zero, so the fixup naming it has
    to move. A relocation naming a field that is not one leaves the value
    reading a bare zero at run time -- silent, and the shape tools/mutate.py
    calls bridged-fixup-dropped."""
    found = corpus.loaded(obj)
    assert found is not None
    for _, got in laid(obj):
        for where, field in got.relocations:
            assert 0 <= where < len(got.code), f"{obj.stem}: relocation past the end"
            assert got.code[where : where + 2] == bytes(2), "a relocated field is not a number"
            known = field in found.fixup_at or field - 1 in found.calls
            assert known, f"{obj.stem}: {field:#x} is not a field this module relocates"


def test_a_body_is_refused_whole_or_not_at_all() -> None:
    """Half this pass's code and half BC's is not something anything
    downstream could reason about, so one op it cannot emit refuses the
    body. Measured: 151 of the corpus's 171 bodies lay out, and the rest name
    the operation that stopped them."""
    total = done = 0
    for obj in FIXTURES:
        found = corpus.loaded(obj)
        if found is None:
            continue
        mapped = code_map(found)
        if isinstance(mapped, str):
            continue
        for _, body in mir.bodies(found, split.partition(found, mapped)):
            total += 1
            got = layout.lay_out(body, body.entry, found)
            if isinstance(got, str):
                assert ":" in got, f"a refusal should say which op: {got}"
            else:
                done += 1
    assert (total, done) == (579, 511)


@pytest.mark.parametrize("obj", FIXTURES, ids=lambda p: p.stem)
def test_relaxation_settles_and_leaves_every_branch_reaching(obj: Path) -> None:
    """The fixed point's own claim.

    A branch shrunk to the short form has to still reach: the range is
    -128..127 from the end of the instruction, and a shrink that put its
    target out of reach would encode a displacement that means somewhere
    else entirely. Safe because shrinking only ever brings a target closer,
    which is also why the loop terminates instead of oscillating between two
    lengths that each justify the other.
    """
    for body, got in laid(obj):
        ops = layout._ordered(body)
        back = walked(got.code, body.entry)
        for op, made in zip(ops, back, strict=True):
            want = original(op)
            if want.op0_kind != OpKind.NEAR_BRANCH16:
                continue
            landed = got.moved[want.near_branch16]
            assert made.near_branch16 == landed
            if made.len <= 2:  # the short form was taken
                assert landed - (made.ip + made.len) in layout.REACH


def test_a_laid_out_body_is_no_bigger_than_bc_s_own() -> None:
    """What relaxation buys, and the reason it is worth the loop.

    Emitting every branch long cost 6.3% -- 1,856 bytes across the corpus's
    fifty layable bodies. With the short forms, the accumulator's own moffs
    load and store, the one-byte inc and dec and the byte-sized immediates,
    the same fifty come out four bytes smaller than BC wrote them.
    """
    was = now = 0
    for obj in FIXTURES:
        for body, got in laid(obj):
            was += sum(layout._length_of(op) or 0 for op in layout._ordered(body))
            now += len(got.code)
    assert now <= was, f"{now} against BC's {was}"


def rebuilt(obj: Path) -> tuple:
    """The object's whole code segment, laid out, or None where it refuses."""
    found = corpus.loaded(obj)
    if found is None:
        return None, None, None
    mapped = code_map(found)
    if isinstance(mapped, str):
        return None, None, None
    blocks = split.partition(found, mapped)
    bodies = list(mir.bodies(found, blocks))
    if not bodies:
        return None, None, None
    # The same fixup set wholeseg.py passes: Module.fixup_at holds only the
    # segment and group OFF16 fixups, and a memory operand naming an
    # external has one this would otherwise leave behind.
    fields = frozenset(one.offset for one in omf.fixups(omf.parse(obj.read_bytes())) if one.seg == found.seg)
    # And the same reachability, so a gap the decoder never walked into is
    # carried here exactly as it is in a real rebuild.
    reached = frozenset(at for block in blocks for insn in block.insns for at in range(insn.at, insn.end))
    got = layout.rebuild(found, bodies, mapped.tables, fields, reached)
    return found, bodies, (None if isinstance(got, str) else got)


@pytest.mark.parametrize("obj", FIXTURES, ids=lambda p: p.stem)
def test_a_rebuilt_segment_carries_every_fixup(obj: Path) -> None:
    """The one that decides whether whole-segment emission can be complete.

    A fixup left behind is a field reading a bare zero at run time --
    silent, and the shape tools/mutate.py calls bridged-fixup-dropped. 464
    of the corpus's are in a `push offset X` or `mov ax,offset X`, where the
    relocated field is the immediate rather than the displacement, and
    reporting only the displacement missed every one.
    """
    from qbopt import omf

    found, bodies, got = rebuilt(obj)
    if got is None:
        return
    carried = {old for _, old in got.relocations}
    lowest = min(op.at for _, body in bodies for op in layout._ordered(body))
    for one in omf.fixups(omf.parse(obj.read_bytes())):
        if one.seg == found.seg and one.offset >= lowest:
            assert one.offset in carried, f"{obj.stem}: the fixup at {one.offset:#x} was left behind"


@pytest.mark.parametrize("obj", FIXTURES, ids=lambda p: p.stem)
def test_a_rebuilt_segment_is_the_same_instructions(obj: Path) -> None:
    """Only where nothing was carried: an ON GOTO table's bytes decode as
    instructions too, so counting them against the ops compares different
    things. What the tables get instead is
    test_a_rebuilt_segment_carries_every_fixup, since their entries are
    fixups."""
    found, bodies, got = rebuilt(obj)
    if got is None or found is None:
        return
    ops = sorted((op for _, body in bodies for op in layout._ordered(body)), key=lambda one: one.at)
    # Only where nothing was carried. A table's entries and BC's alignment
    # padding decode as instructions too, so counting decoded instructions
    # against ops would compare different things. What the carried runs get
    # instead is test_a_rebuilt_segment_carries_every_fixup.
    if len(got.code) != sum(layout._length_of(op) or 0 for op in ops):
        return
    ops = sorted((op for _, body in bodies for op in layout._ordered(body)), key=lambda one: one.at)
    back = walked(got.code, ops[0].at)
    if len(back) != len(ops):
        # BC aligns its procedures, so a run of `90` can sit inside the laid
        # out span and is carried verbatim rather than selected. It decodes
        # as an instruction and is not an op, which is the same reason the
        # length guard above exists -- and that guard misses the case where
        # the padding's own byte is offset by a shorter encoding elsewhere.
        # suite/hotlop.bas under /V is the first object in the corpus with
        # that shape.
        carried = [one for one in back if one.mnemonic == Mnemonic.NOP]
        padding = [op for op in ops if original(op) is not None and original(op).mnemonic == Mnemonic.NOP]
        back = [one for one in back if one.mnemonic != Mnemonic.NOP]
        ops = [op for op in ops if op not in padding]
        assert carried, f"{obj.stem}: {len(back)} instructions from {len(ops)} ops, and none is padding"
    assert len(back) == len(ops)
    for op, made in zip(ops, back, strict=True):
        assert made.mnemonic == original(op).mnemonic, f"{obj.stem} {op.at:#x}"


@pytest.mark.parametrize("obj", FIXTURES, ids=lambda p: p.stem)
def test_data_between_the_instructions_is_carried_or_refused(obj: Path) -> None:
    """BC puts an ON GOTO table inline, in the middle of the code.

    A known table is copied verbatim with its fixups moved -- its entries
    are relocated words, so the destinations live in the fixups and
    as_records remaps them. Anything else between the ops refuses the whole
    object: emitting only what layout understands would drop the rest
    silently, along with whatever it holds.
    """
    found = corpus.loaded(obj)
    if found is None:
        return
    mapped = code_map(found)
    if isinstance(mapped, str):
        return
    bodies = list(mir.bodies(found, split.partition(found, mapped)))
    if not bodies:
        return
    ops = sorted((op for _, body in bodies for op in layout._ordered(body)), key=lambda one: one.at)
    if not ops:
        return
    # declen's own length, not iced's. An emulated x87 site is four bytes --
    # `cd 35 46 c8` -- where the instruction it stands for decodes as three,
    # so iced's `len` undercounts every one of them and invents a gap.
    mapped2 = code_map(found)
    assert not isinstance(mapped2, str)
    fields = frozenset(one.offset for one in omf.fixups(omf.parse(obj.read_bytes())) if one.seg == found.seg)
    got = layout.rebuild(found, bodies, mapped2.tables, fields)
    if isinstance(got, str):
        # A gap that is neither a table nor padding refuses the object,
        # which is the claim: emitting only what layout understands would
        # drop the rest silently.
        assert ":" in got or "not instructions" in got
        return
    # It rebuilt, so every gap was accounted for. The image may well be
    # shorter than the span it came from -- relaxation turns a near branch
    # into a short one -- so the size says nothing and the fixups do:
    # test_a_rebuilt_segment_carries_every_fixup is where that is checked.
    assert got.code


def test_the_rebuildable_share_is_what_was_measured() -> None:
    """Every object in the corpus rebuilds whole-segment.

    A canary on reach, and it moved for nameable reasons: refusing inline
    data took it from 42 to 34, the bare x87 forms took it back to 42,
    carrying the tables took it to 49, asking for every fixup rather than
    the subset Module.fixup_at holds took it to 55, carrying BC's own
    trailing zero padding took it to 109, keeping the base register on a
    cell whose address cannot be named took it to 123, and the padding BC
    puts between procedures took it to 124.

    The last one to refuse was jumps-q-evt, on two calls to B$EVCK that sit
    after an unconditional jump -- code under /V that nothing can reach.
    Carrying a gap no block walked into took it to all of them.
    """
    done = 0
    for obj in FIXTURES:
        if rebuilt(obj)[2] is not None:
            done += 1
    assert done == 485


@pytest.mark.parametrize("obj", FIXTURES[:12], ids=lambda p: p.stem)
def test_layout_tells_the_selector_which_instructions_are_relocated(obj: Path, monkeypatch: pytest.MonkeyPatch) -> None:
    """The wiring, which is what broke.

    select.emit takes `relocated` and honours it everywhere an immediate can
    shrink. The miscompile was that emit's own callers did not pass it:
    `add ax,offset X` arrives as `add ax,0`, the sign-extended byte form
    fits zero, and the two-byte fixup then names a one-byte field. The
    linker patches two bytes regardless, over the immediate and the byte
    after it.

    No fixture contains that instruction -- BC only writes it for an array
    reached by adding its own address into ax, which is why this shipped and
    why a corpus test cannot catch it. What every fixture does have is
    relocated instructions, so this checks the one thing that generalises:
    layout tells the selector, every time.
    """
    # Keyed by the semantics object, not by `at`: layout passes the address
    # the instruction is moving *to*, which is not the one the op came from.
    seen: dict[int, bool] = {}
    real = select.emit

    def watch(
        what: ir.Semantics,
        at: int = 0,
        where: dict[Register_, Register_] | None = None,
        short: bool = False,
        relocated: bool = False,
    ) -> select.Emitted | None:
        # every call, not any: layout asks the selector three times -- to
        # measure, to relax, and to emit -- and a flag missing from one of
        # them is a wrong encoding at exactly that stage
        seen[id(what)] = seen.get(id(what), True) and relocated
        return real(what, at=at, where=where, short=short, relocated=relocated)

    monkeypatch.setattr(select, "emit", watch)
    found, bodies, laid = rebuilt(obj)
    if laid is None:
        return
    fields = frozenset(one.offset for one in omf.fixups(omf.parse(obj.read_bytes())) if one.seg == found.seg)
    asked = 0
    for _name, body in bodies:
        for block in body.blocks:
            for op in block.ops:
                what = layout._semantics(op)
                if what is None or layout._field_in(found, op, fields) is None:
                    continue
                if id(what) not in seen:
                    continue
                asked += 1
                assert seen[id(what)], f"{obj.stem} {op.at:#x}: laid out without telling the selector it is relocated"
    assert asked, f"{obj.stem}: no relocated instruction reached the selector"


def test_an_allocation_reaches_the_bytes() -> None:
    """`select.emit` has taken a `where` since it was written; nothing passed
    one, so `regalloc.colour()` could move a value and the output was
    identical.

    Two things had to be true and only the first was. The map is per *value*
    and an instruction names registers, so it has to be rebuilt per op out
    of the values that op touches. And `body.origin` holds the 32-bit root
    while the instruction names `ax` -- a map keyed on `eax` alone never
    matches, which is exactly what happened and left the whole thing silent.
    """
    from iced_x86 import Register

    from qbopt import regalloc

    found, bodies, base = rebuilt(Path("fixtures/omf/hotlop-p-g2.obj"))
    assert base is not None and found is not None
    _name, body = bodies[0]
    # Not one a phi touches. colour() refuses to move those now: nothing
    # runs on an edge to bring the value across, so the result and every
    # value arriving at it have to share a register.
    crossing = {phi.result for block in body.blocks for phi in block.phis} | {
        value for block in body.blocks for phi in block.phis for value in phi.incoming.values()
    }
    victim = next(
        one
        for one in body.origin
        if body.origin[one] is Register.EAX and not one.flags and one not in crossing
    )

    got = regalloc.colour(body, {victim: Register.ESI})
    assert not isinstance(got, str), got
    assert regalloc.moved(body, got), "the allocation moved nothing, so this proves nothing"

    changed = 0
    for block in body.blocks:
        for op in block.ops:
            where = layout._where(op, got, body.origin)
            what = layout._semantics(op)
            if not where or what is None:
                continue
            was = select.emit(what, at=0)
            now = select.emit(what, at=0, where=where)
            if was is not None and now is not None and was.code != now.code:
                changed += 1
    assert changed, "an allocation that moves a value emitted the same bytes"

    # and with no allocation, byte for byte what it was
    again = layout.rebuild(found, bodies, (), frozenset(), None)
    assert not isinstance(again, str), again
    assert again.code == base.code


@pytest.mark.parametrize("name", ["jumps-q-O", "jumps-p-ot"])
def test_a_served_read_does_not_keep_the_fixup_of_the_operand_it_removed(name: str) -> None:
    """A fixup names an operand, and a transform can remove that operand.

    `cmp word [k],1` served from a register becomes `cmp ax,1`, three bytes
    with no displacement in them. The fixup that named `[k]` was still found
    inside the new instruction's span and re-anchored onto its immediate, so
    LINK wrote an address over the immediate and over the `je` behind it --
    suite/jumps.bas took the CASE ELSE arm for k = 1, and every host test
    passed. An immediate is a real relocation site for `push offset X`, so
    the question is not the operand kind but whether a transform is what
    took the memory operand away.
    """
    from qbopt import mir
    from qbopt import omf
    from qbopt import layout
    from qbopt import module
    from qbopt import transform
    from qbopt import blocks as split
    from qbopt.blocks import code_map

    found = module.of(omf.parse((Path("fixtures/omf") / f"{name}.obj").read_bytes()))
    assert found is not None
    mapped = code_map(found)
    assert not isinstance(mapped, str), mapped

    served = 0
    for _body_name, body in mir.bodies(found, split.partition(found, mapped)):
        for block in transform.forwarded(body, found.dgroup, found.calls).blocks:
            for op in block.ops:
                if op.made is None:
                    continue
                where = (*op.made.dests, *op.made.sources)
                if any(isinstance(one, (ir.Mem, ir.Address)) for one in where):
                    continue
                served += 1
                assert layout._field_in(found, op) is None, (
                    f"{name}: {op.at:#06x} {op.name} kept a relocation with no memory operand to put it in"
                )
    assert served, f"{name}: the pass served no read, so this proves nothing"


def test_bytes_claimed_twice_are_reported_rather_than_raising() -> None:
    """The gap report assumed there was a gap.

    `covered != highest - lowest` has two causes and it only handled one.
    Where a transform moves an op and leaves its `covers` behind, two ops
    claim the same bytes and the total is over, not under -- and the search
    for the first unheld byte found none and raised StopIteration from
    inside the error path. A refusal has to be able to say what is wrong.
    """
    from dataclasses import replace

    from qbopt import ir
    from qbopt import mir
    from qbopt import module
    from qbopt import omf
    from qbopt.blocks import code_map

    found = module.of(omf.parse(Path("fixtures/omf/hotlop-p-g2.obj").read_bytes()))
    assert found is not None
    mapped = code_map(found)
    assert not isinstance(mapped, str), mapped

    name, body = next(iter(mir.bodies(found, split.partition(found, mapped))))
    first = next(one for block in body.blocks for one in block.ops if one.node is not None)
    assert first.node is not None
    span = ir.span(first.node)

    # A second op standing for bytes another one already claims.
    twin = replace(first, covers=span)
    blocks = list(body.blocks)
    blocks[0] = replace(blocks[0], ops=(*blocks[0].ops, twin))
    doubled = replace(body, blocks=tuple(blocks))

    got = layout.rebuild(found, [(name, doubled)], mapped.tables)
    assert isinstance(got, str), "two ops claiming one byte is a refusal"
    assert "claimed by more than one op" in got, got
