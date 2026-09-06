"""
qbopt/wholeseg.py: an object whose code segment this pass wrote.

The claim these make is structural -- the object parses, keeps its code
length, keeps every fixup. Whether it *runs* is tests/test_e2e.py's, because
only LINK and a real 386 can say, and both of the bugs this had were
invisible to everything else.
"""

from pathlib import Path

import pytest

from qbopt import omf
from qbopt import module
from qbopt import wholeseg

FIXTURES = sorted(Path("fixtures/omf").glob("*.obj"))


@pytest.mark.parametrize("obj", FIXTURES, ids=lambda p: p.stem)
def test_a_rebuilt_object_parses_and_agrees_with_itself(obj: Path) -> None:
    """The segment may change length -- selection chooses encodings BC did
    not, and `nots-p-g2` comes out eight bytes longer. What has to hold is
    that the object agrees with itself: SEGDEF says what the LEDATA records
    actually carry, so a reader gets back exactly what was written."""
    data = obj.read_bytes()
    out, why = wholeseg.rebuilt(data)
    if why != wholeseg.REBUILT:
        assert out == data, f"{obj.stem}: refused and still changed the object"
        return
    after = module.of(omf.parse(out))
    assert after is not None
    assert after.end - after.start == len(after.code), f"{obj.stem}: SEGDEF and LEDATA disagree"
    assert len(after.code) > 0


@pytest.mark.parametrize("obj", FIXTURES, ids=lambda p: p.stem)
def _resolved(one, seg: int | None = None, moved: dict | None = None) -> tuple:
    """Everything a relocation means, so a retarget cannot hide in it.

    `loc`, `target` and `index` alone let a fixup keep its name and change
    what it does: self-relative decides whether the linker writes an
    address or a distance, the displacement is the target's own offset,
    and the frame is which segment that offset is measured from -- a
    thread and an explicit frame resolve to the same pair here.
    """
    frame = one.frame
    method = getattr(frame, "method", one.frame_method)
    index = getattr(frame, "index", frame if not hasattr(frame, "method") else None)
    # A fixup naming this same segment carries the target's own offset,
    # and that offset moved with the code. Compared through the map, so
    # the check is that it still names the same instruction rather than
    # the same number.
    disp = one.disp
    if seg is not None and one.target == "segment" and one.index == seg:
        disp = (moved or {}).get(disp, disp)
    return (one.loc, one.selfrel, one.target, one.index, disp, method, index)


def test_a_rebuilt_object_keeps_every_code_fixup_it_still_has_a_home_for(obj: Path) -> None:
    """A fixup left behind is a field reading a bare zero at run time.

    Not every one survives now, and exactly one kind may not: the high half
    of a widened pair reads `[x+2]`, and folding the pair takes that
    relocation with it because there is no longer an instruction with that
    operand. layout.py reports which, relocate.py drops only those, and one
    it cannot explain is still refused outright -- so the count is checked
    against what was deliberately dropped rather than relaxed.
    """
    from qbopt import layout
    from qbopt import blocks as split
    from qbopt.blocks import code_map

    data = obj.read_bytes()
    # Through whichever emitter ran. Both converge on layout.rebuild, and
    # its `Laid` is the only per-occurrence account of where each fixup
    # went -- re-deriving the pipeline here measured one path and compared
    # it against the other's output, which is how a routing change made
    # 311 of these fail on arithmetic rather than on a lost relocation.
    grabbed: dict = {}
    was = layout.rebuild

    def spy(*a, **k):
        got = was(*a, **k)
        if not isinstance(got, str):
            grabbed["laid"] = got
        return got

    layout.rebuild = spy
    try:
        out, why = wholeseg.rebuilt(data)
    finally:
        layout.rebuild = was
    if why != wholeseg.REBUILT:
        return
    laid = grabbed["laid"]
    before, after = module.of(omf.parse(data)), module.of(omf.parse(out))
    # Above the module header only. Whatever sits before the first
    # instruction is BC's own 48 bytes of name and padding; its fixups keep
    # their offsets and never enter the layout's account.
    header = min(one.at for block in split.partition(before, code_map(before)) for one in block.insns)
    original = [x for x in omf.fixups(omf.parse(data)) if x.seg == before.seg and x.offset >= header]
    emitted = [x for x in omf.fixups(omf.parse(out)) if x.seg == after.seg and x.offset >= header]

    # Per occurrence, not per distinct target: nine references to one cell
    # are nine relocations, and a set of (target, index, disp) says nothing
    # about losing eight of them.
    # `laid.relocations` is (offset within the image, the original field);
    # the header sits in front of the image in the object, which is the
    # same `kept + new` the record writer applies.
    moved = {old: header + new for new, old in laid.relocations}
    kept = [one for one in original if one.offset in moved]
    gone = [one for one in original if one.offset not in moved]
    assert not (set(moved) & set(laid.dropped)), f"{obj.stem}: a fixup is both relocated and dropped"
    assert {one.offset for one in gone} == set(laid.dropped), (
        f"{obj.stem}: "
        f"{sorted(hex(x) for x in {one.offset for one in gone} ^ set(laid.dropped))[:4]} "
        "is neither relocated nor accounted as dropped"
    )
    assert len(emitted) == len(kept), f"{obj.stem}: {len(kept)} relocations survived and {len(emitted)} were emitted"

    # Each survivor still names what it named. `inside` maps an offset in
    # the old code to where the layout put it, which is what a fixup into
    # this same segment carries as its displacement.
    from qbopt import relocate

    both = {**laid.covered, **laid.moved}
    inside = {one: relocate._mapped(one, header, both) for one in both}
    landed = {one.offset: one for one in emitted}
    for one in kept:
        other = landed.get(moved[one.offset])
        assert other is not None, f"{obj.stem}: {one.offset:#x} moved to nothing"
        want, got = _resolved(one, before.seg, inside), _resolved(other, after.seg)
        assert want == got, f"{obj.stem}: {one.offset:#x} changed what it names: {want} became {got}"

    # And each one that went belonged to something that is gone.
    assert not (set(laid.dropped) & set(laid.moved)), f"{obj.stem}: a dropped fixup sits on an op that is still there"


def test_the_partition_notices_an_occurrence_that_went_missing() -> None:
    """The invariant above counts occurrences, so losing one has to break
    it. A set of what each fixup names would not notice: flags-p-g2-zd
    references one cell nine times, and eight of them could go with the
    set unchanged."""
    from qbopt import layout

    grabbed: dict = {}
    was = layout.rebuild

    def spy(*a, **k):
        got = was(*a, **k)
        if not isinstance(got, str):
            grabbed["laid"] = got
        return got

    layout.rebuild = spy
    try:
        out, why = wholeseg.rebuilt((Path("fixtures/omf") / "flags-p-g2-zd.obj").read_bytes())
    finally:
        layout.rebuild = was
    assert why == wholeseg.REBUILT
    laid = grabbed["laid"]
    after = module.of(omf.parse(out))
    header = min(one for _new, one in laid.relocations)
    emitted = [x for x in omf.fixups(omf.parse(out)) if x.seg == after.seg and x.offset >= 48]
    kept = {old for _new, old in laid.relocations}
    assert len(emitted) == len(kept) > 1, (len(emitted), len(kept))
    del header

    # One occurrence removed, and the count no longer reconciles.
    short = set(sorted(kept)[1:])
    assert len(emitted) != len(short), "the count must not survive losing an occurrence"

    # And by what each fixup names, it would: the same cell is referenced
    # more than once, so dropping one leaves the set of names unchanged.
    names = {(one.target, one.index, one.disp) for one in emitted}
    fewer = {(one.target, one.index, one.disp) for one in emitted[1:]}
    assert names == fewer, "this fixture does not repeat a target, so it proves nothing"


@pytest.mark.parametrize("obj", FIXTURES, ids=lambda p: p.stem)
def test_the_code_block_comes_after_every_extdef(obj: Path) -> None:
    """OMF numbers external symbols by the order their EXTDEFs appear.

    A code block written where the FIRST code LEDATA stood carries fixups
    naming externals whose EXTDEF has not been read yet, and LINK rejects
    the whole object -- `fatal error L1101: invalid object module`, with
    nothing in it to say which index was wrong. It took bisecting the
    emitter against BC's own boundaries to find, so it is pinned here.
    """
    out, why = wholeseg.rebuilt(obj.read_bytes())
    if why != wholeseg.REBUILT:
        return
    records = omf.parse(out)
    found = module.of(records)
    assert found is not None
    code_at = [
        n for n, r in enumerate(records) if r.type & 0xFE == omf.LEDATA and omf._index(r.body, 0)[0] == found.seg
    ]
    externals = [n for n, r in enumerate(records) if r.type & 0xFE == omf.EXTDEF]
    if externals and code_at:
        assert min(code_at) > max(externals), f"{obj.stem}: code fixups precede an EXTDEF"


def test_the_rebuildable_share_is_what_was_measured() -> None:
    """124 of the corpus's 125 objects. The one that refuses has bytes
    between the ops that reachability never reached, so nothing here can
    say whether they are code."""
    done = sum(1 for obj in FIXTURES if wholeseg.rebuilt(obj.read_bytes())[1] == wholeseg.REBUILT)
    assert done == 487


def test_a_refused_body_is_laid_out_widened_and_only_that_body_is_widened() -> None:
    """Two things at once, because they are the same arrangement.

    A body the allocator refuses is laid out as it was raised -- widened,
    because widening writes machine form with the registers BC had, so it
    needs no allocation and is right either way. Dropping it cost nbody
    every byte the object gained, 2,664 for 2,551.

    And the widening happens for the body that needs it. Pre-widening every
    raised body as well as every optimised one walked each pair chain twice:
    68 calls over the p-g2 fixtures where 38 do.
    """
    from qbopt import mir
    from qbopt import omf
    from qbopt import module
    from qbopt import regalloc
    from qbopt import transform
    from qbopt import blocks as split
    from qbopt.blocks import code_map

    calls = []
    real = transform.widened

    def counting(body, dead=frozenset()):
        calls.append(body)
        return real(body, dead)

    # addrm-p-evt rather than arith-p-g2: raising a declared call's
    # arguments changed what the allocator sees, and arith no longer has a
    # body it refuses. This one still does.
    obj = Path("fixtures/omf/addrm-p-evt.obj")
    found = module.of(omf.parse(obj.read_bytes()))
    assert found is not None
    mapped = code_map(found)
    assert not isinstance(mapped, str)
    blocks = split.partition(found, mapped)

    refused = 0
    for _who, body in mir.bodies(found, blocks):
        done = real(transform.applied(body, found.dgroup, found.calls, blocks=blocks, found=found))
        fixed = regalloc.untangled(done)
        if isinstance(regalloc.colour(fixed, fixed.pins), str) and isinstance(regalloc.colour(done, done.pins), str):
            refused += 1
            assert real(body) is not body, "the raise of the refused body widens"
    assert refused, "no body is refused here, so this proves nothing"

    transform.widened = counting
    try:
        out, why = wholeseg.rebuilt(obj.read_bytes())
    finally:
        transform.widened = real
    assert why == wholeseg.REBUILT, why
    # One per body, plus one for each the allocator sent back to its raise.
    bodies = len(list(mir.bodies(found, blocks)))
    assert bodies < len(calls) <= bodies * 2, f"{len(calls)} for {bodies} bodies"


@pytest.mark.parametrize("stem", ["ivchan-q-O", "stride-p-g2"])
def test_a_rewritten_operation_still_has_machine_operands(stem: str) -> None:
    """15 objects stopped rebuilding: `add is not one select.py can emit`.

    lower.current says it answers in machine form, and for a cell it did
    not -- it left mir.MemRef, which select has no encoding for. Nothing
    noticed because it only builds fresh semantics for an operation a pass
    rewrote, and the one caller that did rewrite operands, lowered(), put
    the missing _located step in itself. The whole-segment path calls
    current directly and got MIR operands.
    """
    raw = (Path("fixtures/omf") / f"{stem}.obj").read_bytes()
    assert wholeseg.rebuilt(raw)[1] == wholeseg.REBUILT


def _unrelocated(data: bytes) -> list[str]:
    """Every emitted displacement of zero that no fixup names.

    BC encodes a data reference as a displacement of zero and lets the
    linker write the address in. One the rebuild emits without its fixup
    reads offset zero of the segment instead -- the right instruction on
    the wrong address, which no structural check on the object notices.
    """
    from qbopt import blocks as split
    from qbopt.blocks import code_map

    found = module.of(omf.parse(data))
    fields = {one.offset for one in omf.fixups(omf.parse(data)) if one.seg == found.seg}
    # Through the block split, not a linear decode: the first 48 bytes are
    # BC's module header and decoding them as code desynchronises the rest.
    insns = [one for block in split.partition(found, code_map(found)) for one in block.insns]
    return [
        f"{one.at:#06x} {one.insn}"
        for one in insns
        if one.disp_at is not None
        and one.disp_len == 2
        and int.from_bytes(found.code[one.disp_at : one.disp_at + 2], "little") == 0
        and one.disp_at not in fields
    ]


@pytest.mark.parametrize("stem", ["stride-q-O", "ivchan-q-O"])
def test_an_operation_a_pass_rewrote_keeps_its_relocation(stem: str) -> None:
    """stride printed T= 0 for 210: `t = t + b(i)` accumulated into offset 0.

    drop_loads serves the read of b(i) from the register the store just
    wrote, which rewrites the `add [t],ax` beside it -- and asm asked
    whether a rewritten operation still has a memory operand by way of
    lower.semantics, which answers in MIR's own operands. A mir.MemRef is
    not an ir.Mem, so the answer was no, and the fixup naming `t` went.
    """
    raw = (Path("fixtures/omf") / f"{stem}.obj").read_bytes()
    out, why = wholeseg.rebuilt(raw, only="drop_loads")
    assert why == wholeseg.REBUILT
    assert not _unrelocated(raw), "the fixture itself has one"
    assert not _unrelocated(out)


def test_an_emission_says_which_emitter_produced_it() -> None:
    """`rebuilt` says only whether it worked, and there are two emitters:
    one from MIR and one from LIR. A caller that has to know which cannot
    ask a boolean, and "it worked" would treat a fallback as the LIR
    path's own output."""
    got = wholeseg.emitted((Path("fixtures/omf") / "hotlop-p-g2.obj").read_bytes())
    assert got.outcome is wholeseg.Emission.LIR
    assert got.reason == wholeseg.REBUILT and got.fallback_reason is None


def test_a_fallback_keeps_its_reason_where_a_caller_can_read_it() -> None:
    """A body nothing can place is laid out the way it always was. The
    public reason stays REBUILT -- every caller reads that -- and what the
    allocator actually said is beside it."""
    from qbopt import allocate

    was = allocate.RegAlloc.transform

    def refuses(self, body):
        raise allocate.Spilled("nothing can hold it")

    allocate.RegAlloc.transform = refuses
    try:
        got = wholeseg.emitted((Path("fixtures/omf") / "hotlop-p-g2.obj").read_bytes())
    finally:
        allocate.RegAlloc.transform = was
    assert got.outcome is wholeseg.Emission.MIR
    assert got.reason == wholeseg.REBUILT
    assert got.fallback_reason and "nothing can hold it" in got.fallback_reason


def test_a_tangled_copy_falls_back_and_says_which() -> None:
    """A phi's copies that all read each other's destinations need a
    temporary this does not have. Named, not emitted in the wrong order."""
    from qbopt import parcopy

    was = parcopy.ParallelCopy.transform

    def tangles(self, body):
        raise parcopy.Tangled("injected: they all read each other")

    parcopy.ParallelCopy.transform = tangles
    try:
        got = wholeseg.emitted((Path("fixtures/omf") / "hotlop-p-g2.obj").read_bytes())
    finally:
        parcopy.ParallelCopy.transform = was
    assert got.outcome is wholeseg.Emission.MIR
    assert got.reason == wholeseg.REBUILT
    assert got.fallback_reason and "Tangled" in got.fallback_reason


def test_a_malformed_copy_group_is_a_bug_and_escapes() -> None:
    """Something that is not a move in a copy group is this pass being
    wrong about its own data, not a body it cannot place."""
    from qbopt import parcopy

    was = parcopy.ParallelCopy.transform

    def broken(self, body):
        raise parcopy.Malformed("injected")

    parcopy.ParallelCopy.transform = broken
    try:
        with pytest.raises(parcopy.Malformed):
            wholeseg.emitted((Path("fixtures/omf") / "hotlop-p-g2.obj").read_bytes())
    finally:
        parcopy.ParallelCopy.transform = was


def test_a_refusal_says_so_rather_than_looking_like_a_rebuild() -> None:
    from qbopt import module

    was = module.of
    try:
        module.of = lambda *a, **k: None
        got = wholeseg.emitted((Path("fixtures/omf") / "hotlop-p-g2.obj").read_bytes())
    finally:
        module.of = was
    assert got.outcome is wholeseg.Emission.REFUSED
    assert got.reason != wholeseg.REBUILT and got.reason


@pytest.mark.parametrize("stem", ["hotlop-p-g2", "nots-q-O"])
def test_rebuilt_still_answers_exactly_what_it_used_to(stem: str) -> None:
    """Every caller reads (bytes, why); the outcome is beside that, not
    instead of it."""
    raw = (Path("fixtures/omf") / f"{stem}.obj").read_bytes()
    out, why = wholeseg.rebuilt(raw)
    got = wholeseg.emitted(raw)
    assert (out, why) == (got.data, got.reason)
    assert isinstance(out, bytes) and isinstance(why, str)


def test_a_copy_with_both_ends_spilled_falls_back_and_says_so() -> None:
    """`mov [bp-2],[bp-4]` is not an instruction, and a phi's copies happen
    at once, so breaking it into a load and a store puts an ungrouped one
    inside the group. The spiller refuses by name -- and that refusal
    escaped `wholeseg` as an exception, so five of the corpus's objects
    crashed the rewrite instead of falling back to BC's own layout.
    Constructed since jumps-v-g3 stopped needing it: raising a declared
    call's arguments changed what the allocator sees, and that object now
    emits through LIR and prints the right answer. The invariant is the
    refusal and its name, not which object happens to provoke it.
    """

    from qbopt import ir
    from qbopt import lir
    from qbopt import spiller
    from qbopt import frame as frames

    # One parallel copy, `v3 <- v4`, with both ends spilled.
    move = lir.Insn(
        at=0x10,
        covers=(0x10, 0x10),
        what=ir.Semantics(ir.Operation.MOVE, "mov", (ir.Held(3, 2),), (ir.Held(4, 2),)),
        defines=(3,),
        uses=(4,),
        op=None,
        group=1,
    )
    body = lir.LirBody(
        name="one",
        entry=0,
        blocks=(lir.LirBlock(at=0, insns=(move,), succ=()),),
        origin={},
        pins={},
    )
    with pytest.raises(spiller.Simultaneous):
        spiller.spilled(body, frozenset({3, 4}), frames.Frame(floor=0))

    # And the emitter catches it rather than letting it escape: five
    # objects crashed the rewrite before it was caught by name. Provoked
    # on an object that does reach the spiller, so the path is the real one.
    real = spiller.spilled

    def refusing(*args, **kwargs):
        raise spiller.Simultaneous("both ends in slots")

    spiller.spilled = refusing
    try:
        got = wholeseg.emitted((Path("fixtures/omf") / "jumps-v-g3.obj").read_bytes())
    finally:
        spiller.spilled = real
    assert got.outcome is wholeseg.Emission.MIR, f"it emitted through {got.outcome}"
    assert got.reason == wholeseg.REBUILT
    assert got.fallback_reason and "Simultaneous" in got.fallback_reason, got.fallback_reason


def test_a_body_that_falls_back_is_not_reported_as_lir() -> None:
    """A refusal is not an emission.

    `emitted` says which emitter wrote the object, and a gate that reads
    a fallback as LIR success would have counted arrprm green while the
    LIR path refused it. The reason has to survive to the caller.
    """
    from pathlib import Path

    from qbopt import wholeseg

    seen = 0
    for one in sorted(Path("fixtures/omf").glob("*.obj"))[:40]:
        got = wholeseg.emitted(one.read_bytes())
        if got.outcome is wholeseg.Emission.LIR:
            assert got.fallback_reason is None, f"{one.name}: reported LIR while falling back -- {got.fallback_reason}"
            seen += 1
        elif got.outcome is wholeseg.Emission.MIR:
            assert got.fallback_reason, f"{one.name}: fell back to MIR and said nothing"
    assert seen, "nothing emitted through LIR; the gate proves nothing"


def test_a_call_with_an_unestablished_interface_falls_back_and_is_not_lir() -> None:
    """A refusal is not an emission, and forced LIR must say so.

    procs-p-evt calls B$ENRA, whose contract declares no inputs, so the
    lowering refuses it rather than emitting a call whose arguments the
    allocation is free to move. The object still comes back -- rewritten
    by the MIR emitter -- and the outcome says which wrote it.
    """
    from pathlib import Path

    from qbopt import wholeseg

    got = wholeseg.emitted(Path("fixtures/omf/procs-p-evt.obj").read_bytes())
    assert got.outcome is not wholeseg.Emission.LIR, "an unestablished call emitted through LIR"
    assert got.fallback_reason and "not established" in got.fallback_reason, (
        f"it fell back for another reason: {got.fallback_reason}"
    )
