"""
qbopt/wholeseg.py: an object whose code segment this pass wrote.

The claim these make is structural -- the object parses, keeps its code
length, keeps every fixup. Whether it *runs* is tests/test_e2e.py's, because
only LINK and a real 386 can say, and both of the bugs this had were
invisible to everything else.
"""

from pathlib import Path

import pytest

from qbopt import wholeseg
from qbopt.objectfile import omf
from qbopt.objectfile import module

FIXTURES = sorted(Path("fixtures/omf").glob("*.obj"))


@pytest.mark.parametrize("name", ["qb45", "pds-g2"])
def test_absorbed_division_survives_removed_call_result_pins(name: str) -> None:
    """qb45 and pds-g2 refused emission when a fresh restore reused pinned value 13."""
    import corpus

    output, _ = corpus.rewritten(Path(f"fixtures/omf/{name}.obj"), dry_run=False)
    assert omf.finalised_at(omf.parse(output)) is not None


@pytest.mark.parametrize("name", ["hotlop", "press", "matrix", "jumps"])
def test_a_loop_label_survives_coalescing_its_first_copy(name: str) -> None:
    """hotlop refused its branch at 0x45 after the copy at loop entry 0x5e disappeared."""
    import corpus

    output, _ = corpus.rewritten(Path(f"fixtures/omf/{name}-p-g2.obj"), dry_run=False)
    assert omf.finalised_at(omf.parse(output)) is not None


@pytest.mark.parametrize("tag", ["p-g2", "v-g3"])
def test_split_edges_do_not_create_phantom_padding(tag) -> None:
    """cmpof hung because synthetic phi blocks created 72 nonexistent padding bytes.

    bools has the same critical-edge shape in the checked-in BC corpus.
    The emitted branches must land on instructions, not shifted addresses.
    """
    from qbopt.frontend.blocks import code_map

    result = wholeseg.emitted(Path(f"fixtures/omf/bools-{tag}.obj").read_bytes())
    assert result.outcome is wholeseg.Emission.LIR, result
    found = module.of(omf.parse(result.data))
    mapped = code_map(found)
    assert not isinstance(mapped, str), mapped


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
    operand. layout.py reports which, omfwrite.py drops only those, and one
    it cannot explain is still refused outright -- so the count is checked
    against what was deliberately dropped rather than relaxed.
    """
    from qbopt.backend import layout
    from qbopt.frontend import blocks as split
    from qbopt.frontend.blocks import code_map

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
    # One field may land more than once: an op copied into two places takes
    # its relocation to both.
    moved: dict[int, list[int]] = {}
    for new, old in laid.relocations:
        moved.setdefault(old, []).append(header + new)
    kept = [one for one in original if one.offset in moved]
    gone = [one for one in original if one.offset not in moved]
    assert not (set(moved) & set(laid.dropped)), f"{obj.stem}: a fixup is both relocated and dropped"
    assert {one.offset for one in gone} == set(laid.dropped), (
        f"{obj.stem}: "
        f"{sorted(hex(x) for x in {one.offset for one in gone} ^ set(laid.dropped))[:4]} "
        "is neither relocated nor accounted as dropped"
    )
    # A reference the backend made itself -- a constant now held in a cell,
    # say -- is in `laid.symbols`, and is the only other fixup there may be.
    made = {(header + offset, address.index, address.disp) for offset, address in laid.symbols}
    placed = sum(len(moved[one.offset]) for one in kept)
    assert len(emitted) == placed + len(laid.symbols), (
        f"{obj.stem}: {placed} relocations were placed, {len(laid.symbols)} were made and {len(emitted)} were emitted"
    )
    assert made <= {(one.offset, one.index, one.disp) for one in emitted}, (
        f"{obj.stem}: a made reference was not emitted"
    )

    # Each survivor still names what it named. `inside` maps an offset in
    # the old code to where the layout put it, which is what a fixup into
    # this same segment carries as its displacement.
    from qbopt.backend import omfwrite

    both = {**laid.covered, **laid.moved}
    inside = {one: omfwrite._mapped(one, header, both) for one in both}
    landed = {one.offset: one for one in emitted}
    for one in kept:
        for at in moved[one.offset]:
            other = landed.get(at)
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
    from qbopt.backend import layout

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
    names = [(one.target, one.index, one.disp) for one in emitted]
    repeated = next((at for at, name in enumerate(names) if names.count(name) > 1), None)
    assert repeated is not None, "this fixture does not repeat a target, so it proves nothing"
    assert set(names) == set(names[:repeated] + names[repeated + 1 :])


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
    from qbopt.frontend import blocks as split
    from qbopt.frontend.blocks import code_map

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


def test_an_allocation_refusal_preserves_the_input_and_original_reason() -> None:
    """hotlop: an injected spill failure was hidden by the MIR emitter's
    unrelated 'mov is not one select.py can emit' error at 0x003c."""
    from qbopt.backend import allocate

    was = allocate.RegAlloc.transform

    def refuses(self, body):
        raise allocate.Spilled("nothing can hold it")

    allocate.RegAlloc.transform = refuses
    try:
        got = wholeseg.emitted((Path("fixtures/omf") / "hotlop-p-g2.obj").read_bytes())
    finally:
        allocate.RegAlloc.transform = was
    assert got.outcome is wholeseg.Emission.REFUSED
    assert got.data == Path("fixtures/omf/hotlop-p-g2.obj").read_bytes()
    assert "nothing can hold it" in got.reason


def test_a_tangled_copy_refuses_without_trying_another_emitter() -> None:
    """A phi's copies that all read each other's destinations need a
    temporary this does not have. Named, not emitted in the wrong order."""
    from qbopt.backend import parcopy

    was = parcopy.ParallelCopy.transform

    def tangles(self, body):
        raise parcopy.Tangled("injected: they all read each other")

    parcopy.ParallelCopy.transform = tangles
    try:
        got = wholeseg.emitted((Path("fixtures/omf") / "hotlop-p-g2.obj").read_bytes())
    finally:
        parcopy.ParallelCopy.transform = was
    assert got.outcome is wholeseg.Emission.REFUSED
    assert got.data == Path("fixtures/omf/hotlop-p-g2.obj").read_bytes()
    assert "Tangled" in got.reason


def test_a_frame_refusal_preserves_the_input(monkeypatch: pytest.MonkeyPatch) -> None:
    # Qrender MAIN crashed when its spill frame had no recognized return.
    from qbopt.model import lir
    from qbopt.backend import prologue

    def refuses(self: prologue.Prologue, body: lir.LirBody) -> lir.LirBody:
        raise prologue.Refused("2 bytes of frame have no recognized exit")

    monkeypatch.setattr(prologue.Prologue, "transform", refuses)
    data = Path("fixtures/omf/hotlop-p-g2.obj").read_bytes()
    result = wholeseg.emitted(data)
    assert result.outcome is wholeseg.Emission.REFUSED
    assert result.data == data
    assert "2 bytes of frame have no recognized exit" in result.reason


def test_a_malformed_copy_group_is_a_bug_and_escapes() -> None:
    """Something that is not a move in a copy group is this pass being
    wrong about its own data, not a body it cannot place."""
    from qbopt.backend import parcopy

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
    from qbopt.objectfile import module

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


def test_a_spilled_copy_stays_grouped_and_backend_refusals_are_reported(
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    """`mov [bp-2],[bp-4]` is not an instruction, and a phi's copies happen
    at once, so breaking it into a load and a store puts an ungrouped one
    inside the group. Keep both slots in one grouped move for scheduling.
    Historically the refusal escaped `wholeseg` as an exception and five
    objects crashed; retain coverage of its public exception handling too.
    """

    from qbopt.model import ir
    from qbopt.model import lir
    from qbopt.backend import spiller
    from qbopt.backend import frame as frames

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
    copied, _ = spiller.spilled(body, frozenset({3, 4}), frames.Frame(floor=0))
    grouped = [one for block in copied.blocks for one in block.insns if one.group == 1]
    assert len(grouped) == 1
    assert isinstance(grouped[0].what.dests[0], ir.Mem)
    assert isinstance(grouped[0].what.sources[0], ir.Mem)

    # And the emitter catches it rather than letting it escape: five objects
    # crashed before it was caught by name. Inject through the machine-phase
    # list rather than assuming a particular fixture will always spill.
    from qbopt import flow

    class Refusing:
        name = "refusing"

        def transform(self, _body):
            raise spiller.Simultaneous("both ends in slots")

    monkeypatch.setattr(flow, "machine", lambda *_args, **_kwargs: [Refusing()])
    got = wholeseg.emitted((Path("fixtures/omf") / "matrix-v-g3.obj").read_bytes())
    assert got.outcome is wholeseg.Emission.REFUSED
    assert got.data == Path("fixtures/omf/matrix-v-g3.obj").read_bytes()
    assert "Simultaneous" in got.reason


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
        else:
            assert got.outcome is wholeseg.Emission.REFUSED
            assert got.data == one.read_bytes()
            assert got.reason != wholeseg.REBUILT
    assert seen, "nothing emitted through LIR; the gate proves nothing"


def test_the_long_divide_bodys_entry_reads_nothing_it_has_not_written() -> None:
    """lngmix printed 3419650 where BC prints 142900.

    Checked here as the emitted invariant behind that, not as the sum:
    running the program is a manual corroboration and is recorded in the
    commit rather than automated in this slice.

    The raise's machine-state phis reached LIR unread, `phielim` gave each
    an edge copy, and one of those copied an ax nothing had written over
    the accumulator. Checked on the emitted bytes rather than by running
    it: the entry block may not read a register it has not written.
    """
    from pathlib import Path

    from iced_x86 import Decoder
    from iced_x86 import OpAccess
    from iced_x86 import Register
    from iced_x86 import FlowControl
    from iced_x86 import RegisterExt
    from iced_x86 import InstructionInfoFactory

    from qbopt import wholeseg
    from qbopt.objectfile import omf
    from qbopt.objectfile import module
    from qbopt.frontend import blocks as split
    from qbopt.frontend.blocks import code_map

    got = wholeseg.emitted(Path("fixtures/omf/lngmix-p-g2.obj").read_bytes())
    assert got.outcome is wholeseg.Emission.LIR, f"it fell back: {got.fallback_reason}"
    after = module.of(omf.parse(got.data))
    entry = min(one.at for block in split.partition(after, code_map(after)) for one in block.insns)
    written = {
        RegisterExt.full_register(one)
        for one in (Register.BP, Register.SP, Register.DS, Register.ES, Register.SS, Register.CS)
    }
    factory = InstructionInfoFactory()
    for one in Decoder(16, after.code[entry:], ip=entry):
        if one.is_invalid or one.flow_control != FlowControl.NEXT:
            break
        used = factory.info(one).used_registers()
        reads = {
            RegisterExt.full_register(register.register)
            for register in used
            if register.access in (OpAccess.READ, OpAccess.COND_READ, OpAccess.READ_WRITE, OpAccess.READ_COND_WRITE)
        }
        assert reads <= written, f"{one.ip:#06x} reads {sorted(reads - written)}, which nothing wrote"
        # cdq and idiv define edx without an explicit destination operand.
        # Operand position is not a read/write model (cmp writes neither).
        written |= {
            RegisterExt.full_register(register.register)
            for register in used
            if register.access in (OpAccess.WRITE, OpAccess.READ_WRITE)
        }


def test_the_half_a_divide_hands_back_does_not_fall_out_of_the_lir_route() -> None:
    """Owning no original bytes is not the same as having no identity.

    The projection stands for none of BC's bytes -- the site's range
    belongs to the divide, once -- and omfwrite read that as "inserted"
    and stripped its node, which is what says which idiom it is. It
    reached the general encoder as an operation named `restore` with no
    operands, so every body holding an absorbed divide left the route
    with the allocator that can spill and took the one that cannot.
    """
    from pathlib import Path

    from qbopt import wholeseg

    seen = []
    was = wholeseg._through_lir
    try:
        wholeseg._through_lir = lambda *a, **k: seen.append(was(*a, **k)) or seen[-1]
        wholeseg.rebuilt(Path("fixtures/omf/lngmix-p-g2.obj").read_bytes())
    finally:
        wholeseg._through_lir = was
    assert seen, "the LIR route was never tried"
    why = seen[0]
    assert not (isinstance(why, str) and "restore" in why), why


def test_production_never_copies_raise_provenance_back_onto_module(monkeypatch: pytest.MonkeyPatch) -> None:
    """The first external SourceMap still reached emission through
    ``SourceMap.applied()``, recreating the fused Module interface the split
    was meant to remove. Production must thread the side table directly.
    """
    from qbopt.objectfile import module

    def fused(*_args: object, **_kwargs: object) -> None:
        raise AssertionError("raise provenance was copied back onto Module")

    monkeypatch.setattr(module.SourceMap, "applied", fused)
    data = Path("fixtures/omf/cmpord-p-evt.obj").read_bytes()
    result = wholeseg.emitted(data, optimise=True)
    assert result.outcome is wholeseg.Emission.LIR, result.reason
