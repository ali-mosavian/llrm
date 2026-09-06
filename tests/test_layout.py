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
from qbopt import target
from qbopt import omf
from qbopt import asm
from qbopt import layout
from qbopt import select
from qbopt.declen import decode
from qbopt import blocks as split
from qbopt.blocks import code_map

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
            yield body, got, found


def paired(ops: list, back: list, found) -> list[tuple]:
    """(op, the instructions it emitted). One each, except a folded call.

    An operation is not always an instruction: the raise turns an
    absorbable runtime call and its push run into one operation over the
    argument values, and lowering writes four instructions for it. So these
    are paired by how many bytes each op emitted rather than one for one.
    """
    from qbopt import layout as laying

    out, at = [], 0
    for op in ops:
        folded = found.absorbed.get(getattr(op, "id", None)) if found is not None else None
        if folded is None:
            out.append((op, back[at : at + 1]))
            at += 1
            continue
        made = asm._absorbed(*folded)
        assert made is not None, f"{op.at:#x}: the absorbed call emits nothing"
        taken, size = 0, 0
        while at + taken < len(back) and size < len(made.code):
            size += back[at + taken].len
            taken += 1
        assert size == len(made.code), f"{op.at:#x}: {size} bytes read for {len(made.code)}"
        out.append((op, back[at : at + taken]))
        at += taken
    assert at == len(back), f"{len(back) - at} instructions belong to no operation"
    return out


@pytest.mark.parametrize("obj", FIXTURES, ids=lambda p: p.stem)
def test_a_laid_out_body_is_the_same_instructions_in_the_same_order(obj: Path) -> None:
    for body, got, found in laid(obj):
        ops = layout._ordered(body)
        back = walked(got.code, body.entry)
        for op, made in paired(ops, back, found):
            if found.absorbed.get(getattr(op, "id", None)) is not None:
                continue  # a folded call is not the instruction it came from
            want = original(op)
            assert made[0].mnemonic == want.mnemonic, f"{obj.stem} {op.at:#x}: {made[0]} != {want}"


@pytest.mark.parametrize("obj", FIXTURES, ids=lambda p: p.stem)
def test_every_branch_points_where_its_target_went(obj: Path) -> None:
    """The whole reason layout exists.

    An instruction whose encoding differs in length from BC's moves
    everything after it, and a branch left with its old number then points
    at whatever now sits there -- which is a working program that does
    something else, the worst kind of wrong.
    """
    for body, got, found in laid(obj):
        ops = layout._ordered(body)
        back = walked(got.code, body.entry)
        for op, made in paired(ops, back, found):
            if found.absorbed.get(getattr(op, "id", None)) is not None:
                continue  # a folded call branches nowhere
            want = original(op)
            if want.op0_kind != OpKind.NEAR_BRANCH16:
                continue
            landed = got.moved.get(want.near_branch16)
            assert landed is not None, f"{obj.stem} {op.at:#x}: target left this body"
            assert made[0].near_branch16 == landed, (
                f"{obj.stem} {op.at:#x}: {made[0]} should reach {landed:#x}"
            )


@pytest.mark.parametrize("obj", FIXTURES, ids=lambda p: p.stem)
def test_every_relocation_points_at_a_field_the_module_really_has(obj: Path) -> None:
    """A relocated displacement goes out as zero, so the fixup naming it has
    to move. A relocation naming a field that is not one leaves the value
    reading a bare zero at run time -- silent, and the shape tools/mutate.py
    calls bridged-fixup-dropped."""
    found = corpus.loaded(obj)
    assert found is not None
    for _body, got, _found in laid(obj):
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
    assert (total, done) == (583, 515)


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
    for body, got, found in laid(obj):
        ops = layout._ordered(body)
        back = walked(got.code, body.entry)
        for op, group in paired(ops, back, found):
            if found.absorbed.get(getattr(op, "id", None)) is not None:
                continue  # a folded call branches nowhere
            made = group[0]
            want = original(op)
            if want.op0_kind != OpKind.NEAR_BRANCH16:
                continue
            landed = got.moved[want.near_branch16]
            assert made.near_branch16 == landed
            if made.len <= 2:  # the short form was taken
                assert landed - (made.ip + made.len) in asm.REACH


def test_a_laid_out_body_is_no_bigger_than_bc_s_own() -> None:
    """What relaxation buys, and the reason it is worth the loop.

    Emitting every branch long cost 6.3% -- 1,856 bytes across the corpus's
    fifty layable bodies. With the short forms, the accumulator's own moffs
    load and store, the one-byte inc and dec and the byte-sized immediates,
    the same fifty come out four bytes smaller than BC wrote them.
    """
    was = now = 0
    for obj in FIXTURES:
        for body, got, _found in laid(obj):
            was += sum(asm._length_of(op) or 0 for op in layout._ordered(body))
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
        if one.seg != found.seg or one.offset < lowest:
            continue
        # Or deliberately dropped, which layout reports rather than does
        # silently: a folded runtime call reads its whole four-byte operand
        # from one address, so the second push's own fixup has no field to
        # go in and the fold says so by covering its bytes.
        assert one.offset in carried or one.offset in got.dropped, (
            f"{obj.stem}: the fixup at {one.offset:#x} was left behind"
        )


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
    if len(got.code) != sum(asm._length_of(op) or 0 for op in ops):
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
    assert done == 487


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
        held: dict | None = None,
    ) -> select.Emitted | None:
        # every call, not any: layout asks the selector three times -- to
        # measure, to relax, and to emit -- and a flag missing from one of
        # them is a wrong encoding at exactly that stage
        seen[id(what)] = seen.get(id(what), True) and relocated
        return real(what, at=at, where=where, short=short, relocated=relocated, held=held)

    monkeypatch.setattr(select, "emit", watch)
    found, bodies, laid = rebuilt(obj)
    if laid is None:
        return
    fields = frozenset(one.offset for one in omf.fixups(omf.parse(obj.read_bytes())) if one.seg == found.seg)
    asked = 0
    for _name, body in bodies:
        for block in body.blocks:
            for op in block.ops:
                what = asm._semantics(op)
                if what is None or asm._field_in(found, op, fields) is None:
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
    # Whichever value and register the allocator will take. Naming one
    # outright stopped working when two-address operands began tying values
    # into classes: the classes are larger, the pressure is real, and
    # "interferes with every register at once" is a refusal rather than a
    # failure. What this test is about is whether an allocation that *is*
    # made reaches the bytes.
    got = None
    for one in body.origin:
        if one.flags or one in crossing or body.origin[one] is not Register.EAX:
            continue
        for want in target.AVAILABLE:
            if want is Register.EAX:
                continue
            tried = regalloc.colour(body, {one: want})
            if isinstance(tried, str) or not regalloc.moved(body, tried):
                continue
            # And one that some instruction actually names. A value can be
            # moved and change no byte: `mov ax,1` *uses* the eax before it,
            # because writing ax preserves the high half, and that use
            # appears in no operand. Remapping it correctly touches nothing,
            # which is the whole point of the map being per side -- but it
            # would leave this test proving that.
            if any(
                asm._where(op, tried, body.origin) is not None
                and (what := asm._semantics(op)) is not None
                and (first := select.emit(what, at=0)) is not None
                and (second := select.emit(what, at=0, where=asm._where(op, tried, body.origin)))
                is not None
                and first.code != second.code
                for block in body.blocks
                for op in block.ops
            ):
                got = tried
                break
        if got is not None:
            break
    assert got is not None, "no pin was accepted, so this proves nothing"

    changed = 0
    for block in body.blocks:
        for op in block.ops:
            where = asm._where(op, got, body.origin)
            what = asm._semantics(op)
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
                # What the pass made of it, in MIR's own operands. `made`
                # was the marker; a pass that says what it computes as
                # values and cells sets none.
                if op.raised is None or (op.args, op.results) == op.raised:
                    continue
                if any(isinstance(one, mir.Cell) for one in (*op.args, *op.results)):
                    continue
                served += 1
                assert asm._field_in(found, op) is None, (
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


def test_allocation_is_a_phase_and_the_assembler_emits_what_it_is_handed(obj: Path) -> None:
    """`rebuild` used to colour when handed no assignment, and does not.

    An assembler that allocates is one nothing downstream can be told has
    already allocated: objwrite.py runs after a real allocator and was
    allocated over a second time. `layout.allocated()` is that work, and
    the two together give what rebuild alone used to.
    """
    from qbopt import mir
    from qbopt import regalloc
    from qbopt import transform
    from qbopt import blocks as split
    from qbopt.blocks import code_map

    found = corpus.loaded(obj)
    mapped = code_map(found)
    if isinstance(mapped, str):
        return
    blocks = split.partition(found, mapped)
    bodies = [
        # The same pipeline wholeseg runs: widening is not a pass and
        # goes after every one of them, just before lowering.
        (name, transform.widened(transform.applied(body, found.dgroup, found.calls)))
        for name, body in mir.bodies(found, blocks)
    ]
    if not any(layout._names_a_value(body) for _name, body in bodies):
        return

    # What rebuild does for itself when handed none: untangle, then colour.
    # A body nothing can colour is laid out as it was raised, which is a
    # different question and test_wholeseg's own; this one is about what
    # "no assignment" means when there is an assignment to be had.
    mine: dict = {}
    fixed = []
    for name, body in bodies:
        one = regalloc.untangled(body)
        got = regalloc.colour(one, one.pins)
        if isinstance(got, str):
            return
        mine.update(got)
        fixed.append((name, one))

    coloured, assignment = layout.allocated(bodies)
    theirs = layout.rebuild(found, coloured, mapped.tables, assignment=assignment)
    ours = layout.rebuild(found, fixed, mapped.tables, assignment=mine or None)
    assert isinstance(theirs, str) == isinstance(ours, str), f"{obj.stem}: one refused and one did not"
    if not isinstance(theirs, str):
        assert theirs.code == ours.code, f"{obj.stem}: two answers for one body"

    # And handed nothing, it remaps nothing rather than deciding for itself.
    import inspect

    assert "regalloc" not in inspect.getsource(layout.rebuild), "the assembler allocates again"


def test_a_relocation_belongs_to_the_operand_and_not_to_a_place(obj: Path) -> None:
    """Which fixup an operation carries is settled at the raise.

    It used to be found by searching BC's own byte span for one, every time
    layout asked -- so an operation could only be relocated correctly while
    it still stood where BC wrote it, and `covers` had to keep saying which
    bytes it stood for. A relocation belongs to an operand.

    Checked as: the answer from the operation matches the answer the search
    gave, on every operation in the corpus that has one.
    """
    from qbopt import ir
    from qbopt import mir
    from qbopt import blocks as split
    from qbopt.blocks import code_map

    found = corpus.loaded(obj)
    mapped = code_map(found)
    if isinstance(mapped, str):
        return
    fields = frozenset(found.fixup_at)
    seen = 0
    for _name, body in mir.bodies(found, split.partition(found, mapped)):
        for op in layout._ordered(body):
            if op.node is None or isinstance(op.node, ir.Restore):
                continue
            if found.code[op.at : op.at + 1] in (b"\x9a", b"\xea") and op.at + 1 in fields:
                want = op.at + 1
            else:
                lo, hi = ir.span(op.node)
                inside = [one for one in fields if lo <= one < hi]
                want = inside[0] if len(inside) == 1 else None
            # One fixup per relocated instruction. An operation the raise
            # made from one instruction has one, and the bytes it came from
            # are where it is; an operation the raise folded a whole call
            # into has one per operand, and those are the pushes' own --
            # which the bytes at the call say nothing about.
            said = found.refs.get(op.id)
            if op.id is not None and op.id in found.absorbed:
                site, _read = found.absorbed[op.id]
                assert said, f"{obj.stem} {op.at:#06x}: a folded call with no fixup"
                assert all(site.start <= one < site.end for one in said), (
                    f"{obj.stem} {op.at:#06x}: {said} is outside {site.start:#x}..{site.end:#x}"
                )
                seen += 1
                continue
            ref = said[0] if said else None
            assert said is None or len(said) == 1, f"{obj.stem} {op.at:#06x}: {said}"
            assert ref == want, f"{obj.stem} {op.at:#06x}: says {ref}, the bytes say {want}"
            seen += ref is not None
    assert seen or not fields, f"{obj.stem}: nothing carries a fixup, so this proves nothing"


def test_a_body_the_allocator_refused_is_untangled_before_it_is_laid_out() -> None:
    """Wired in, not merely available.

    addrm and arridx each hold a class two of whose members are live at
    once, and colour() refuses to move one. rebuild() puts a copy in first
    -- on the phi edge for addrm, before a two-address operation for arridx
    -- and the body colours.

    One of the two shapes now, not both: the second came from the copies
    the hoist emitted while it was doing its own allocation, and those are
    gone. What is checked is the wiring, and one body still checks it.
    """
    from qbopt import mir
    from qbopt import regalloc
    from qbopt import transform
    from qbopt import blocks as split
    from qbopt.blocks import code_map

    seen = 0
    for name in ("addrm-p-g2", "arridx-p-g2"):
        found = corpus.loaded(Path("fixtures/omf") / f"{name}.obj")
        mapped = code_map(found)
        assert not isinstance(mapped, str)
        blocks = split.partition(found, mapped)
        for _who, body in mir.bodies(found, blocks):
            done = transform.applied(body, found.dgroup, found.calls, blocks=blocks, found=found)
            if not regalloc._tangled(done):
                continue
            seen += 1
            assert isinstance(regalloc.colour(done, done.pins), str), f"{name}: refused while tangled"
            fixed = regalloc.untangled(done)
            assert not isinstance(regalloc.colour(fixed, fixed.pins), str), f"{name}: colours after"
    assert seen >= 1, "neither body was tangled, so this proves nothing"


def test_a_moved_operation_keeps_its_fixup() -> None:
    """Why the side table is keyed by the operation and not by its address.

    `ref` used to sit on the op, which is what made it survive the hoist
    re-seating something. Off the op it has to be keyed by an identity that
    moves with the operation; keyed by `at` the relocation stays behind and
    the address comes out a bare zero.
    """
    from dataclasses import replace
    from qbopt import mir
    from qbopt import module
    from qbopt import omf
    from qbopt import blocks as split
    from qbopt.blocks import code_map

    found = module.of(omf.parse(Path("fixtures/omf/hotlop-p-g2.obj").read_bytes()))
    assert found is not None
    mapped = code_map(found)
    assert not isinstance(mapped, str)
    bodies = mir.bodies(found, split.partition(found, mapped))
    assert found.refs, "the raise recorded no relocations at all"

    carried = [
        op
        for _, body in bodies
        for block in body.blocks
        for op in block.ops
        if op.id in found.refs and asm._field_in(found, op) is not None
    ]
    assert carried, "no operation carries a fixup; the test measures nothing"

    one = carried[0]
    was = asm._field_in(found, one)
    moved = replace(one, at=one.at + 0x100)
    assert asm._field_in(found, moved) == was, "the relocation stayed behind"


def test_a_fold_does_not_keep_the_fixup_of_the_read_it_replaced() -> None:
    """`cmp word [k],1` folded to `cmp ax,1` has nowhere to put an address.

    Relocating that immediate writes an address over it and over the branch
    behind it; suite/jumps.bas took the CASE ELSE arm for k = 1 that way.
    The check asked `op.made is not None` to mean "a pass rewrote this",
    and a pass that says what it computes in MIR's own operands sets no
    `made` at all -- so every fold was answered "nothing touched it".
    """
    from qbopt import ir
    from qbopt import mir
    from qbopt import module
    from qbopt import omf
    from qbopt import transform
    from qbopt import blocks as split
    from qbopt.blocks import code_map

    found = module.of(omf.parse(Path("fixtures/omf/bools-p-g2.obj").read_bytes()))
    assert found is not None
    mapped = code_map(found)
    assert not isinstance(mapped, str)

    checked = 0
    for _, body in mir.bodies(found, split.partition(found, mapped)):
        for block in transform.applied(body, found.dgroup, found.calls, only="fold").blocks:
            for op in block.ops:
                if op.raised is None or (op.args, op.results) == op.raised:
                    continue
                was = getattr(op.node, "semantics", None)
                if was is None or not any(
                    isinstance(one, (ir.Mem, ir.Address)) for one in (*was.dests, *was.sources)
                ):
                    continue
                checked += 1
                assert not asm._still_has_an_operand_for_it(op), (
                    f"{op.at:#x}: the memory operand is gone and the fixup was kept"
                )
    assert checked, "no fold replaced a memory read; the test measures nothing"


def test_an_operation_may_carry_a_fixup_for_each_instruction_it_stands_for() -> None:
    """One operation is not always one instruction.

    Absorbing a long divide is `mov eax,[a] / mov ecx,[b] / cdq / idiv ecx`
    and two of those four carry a fixup. select.Emitted reported one field
    because until now an operation was an instruction and an instruction
    relocates at most one -- select.restore emits three and gets away with
    it only by having no fixup at all.

    The plumbing carries as many as an operation has: Emitted.places says
    where each landed, the module's refs say which fixup each operand
    carries, and layout pairs them in the order the instructions came out.
    """
    from qbopt import select

    # However it was built: _assemble reads the two offsets iced gives back,
    # an idiom says where its own fields are, and both answer `places`.
    plain = select.Emitted(b"\x8b\x06\x00\x00", displacement_at=2)
    assert plain.places == (2,) and plain.relocated_at == 2

    idiom = select.Emitted(b"\x66\xa1\x00\x00\x66\xb9\x00\x00", fields=(2, 6))
    assert idiom.places == (2, 6), idiom.places
    assert idiom.relocated_at == 2, "the first is what a caller with one wants"

    silent = select.Emitted(b"\x99")
    assert silent.places == () and silent.relocated_at is None



def test_a_restore_emits_the_idiom_and_not_the_bytes_it_stands_on() -> None:
    """negnot printed A=-7317895 for 305419897: the pair was never handed back.

    The length pass asks whether the operation carries an ir.Restore before
    it asks whether anything rewrote it; the emit pass asked the other way
    round. A restore has no semantics -- it is three instructions, not one
    -- so emit took the verbatim branch and wrote the `neg dx` sitting at
    its address, two bytes where four were measured. PRINT then consumed a
    dx the widened `neg eax` had already made stale.
    """
    from qbopt import blocks as split
    from qbopt import module
    from qbopt import omf
    from qbopt import select
    from qbopt import transform
    from qbopt import wholeseg
    from qbopt.blocks import code_map

    raw = Path("fixtures/omf/negnot-q-O.obj").read_bytes()
    found = module.of(omf.parse(raw))
    blocks = split.partition(found, code_map(found))
    wanted = [
        op.node.pair
        for _name, body in mir.bodies(found, blocks)
        for block in transform.widened(body).blocks
        for op in block.ops
        if isinstance(op.node, ir.Restore)
    ]
    assert wanted, "nothing to restore, so this proves nothing"

    was = transform.applied
    transform.applied = lambda body, *a, **k: body  # widening on its own
    try:
        out, why = wholeseg.rebuilt(raw)
    finally:
        transform.applied = was
    assert why == wholeseg.REBUILT
    code = bytes(module.of(omf.parse(out)).code)
    for pair in wanted:
        assert code.count(select.restore(pair).code) >= 1, f"pair {pair} was never handed back"
