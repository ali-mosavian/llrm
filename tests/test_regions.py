"""What a reference reaches, as a set -- and the facts that were lost without it.

Each test here names a shape the old five-field model got wrong or could not
state. The counts are from tools/aliasdiff.py over the 487-object corpus.
"""

from dataclasses import replace

from qbopt.model import mir
from qbopt.analysis import regions
from qbopt.objectfile.module import Addr
from qbopt.objectfile.module import Space
from qbopt.objectfile.module import Register


def _ref(addr, width=2, **rest) -> mir.MemRef:
    return mir.MemRef(addr=addr, width=width, **rest)


def test_an_exclusion_bounds_a_reference_that_names_its_address_too() -> None:
    """32 loss shapes: excludes were read only off a reference naming no byte.

    The indexed store proved it cannot touch the descriptor's own fields, and
    that proof was dropped because the same reference also had an address.
    """
    array = _ref(Addr(Space.SEGMENT, 0x6, 5, Register.SI), excludes=((Addr(Space.SEGMENT, 0x266, 5), 2),))
    field = _ref(Addr(Space.SEGMENT, 0x266, 5), width=1)
    neighbour = _ref(Addr(Space.SEGMENT, 0x300, 5), width=1)

    assert not regions.may_alias(array, field)
    assert regions.may_alias(array, neighbour), "only the excluded bytes, not the segment"


def test_a_call_clear_of_the_frame_still_reaches_the_stack() -> None:
    """489 loss shapes: the frame modelled as part of the stack, not beside it.

    Both are the same region counted from different registers. Stating it as
    containment makes the exclusion unstatable -- a bounded call writes no
    local and still pushes.
    """
    call = _ref(None, width=0, space=Space.STACK, excludes=(mir.WHOLE_FRAME,))
    local = _ref(Addr(Space.FRAME, -0x16), width=1)
    pushed = _ref(Addr(Space.STACK, 0))

    assert not regions.may_alias(call, local)
    assert regions.may_alias(call, pushed)
    assert regions.may_alias(local, pushed), "different origins never compare"


def test_an_external_cell_is_a_named_static_and_neither_the_frame_nor_the_stack() -> None:
    """The link decides which segment holds it, so it is DGROUP and no byte of it."""
    cell = _ref(Addr(Space.EXTERNAL, 0, 3))
    other = _ref(Addr(Space.EXTERNAL, 0, 4))

    assert regions.may_alias(cell, other), "distinct EXTDEFs may be one cell"
    assert regions.may_alias(cell, _ref(Addr(Space.SEGMENT, 0x10, 5)))
    assert not regions.may_alias(cell, _ref(Addr(Space.FRAME, -4)))
    assert not regions.may_alias(cell, _ref(Addr(Space.STACK, 0)))


def test_an_unbounded_index_reaches_its_own_segment_and_no_other() -> None:
    array = _ref(Addr(Space.SEGMENT, 0x6, 5, Register.SI))

    assert regions.may_alias(array, _ref(Addr(Space.SEGMENT, 0x999, 5)))
    assert not regions.may_alias(array, _ref(Addr(Space.SEGMENT, 0x6, 7)))


def test_a_displacement_no_fixup_claims_reaches_dgroup_but_not_the_stack() -> None:
    literal = _ref(Addr(Space.LITERAL, 0x10))

    assert regions.may_alias(literal, _ref(Addr(Space.SEGMENT, 0x10, 5)))
    assert regions.may_alias(literal, _ref(Addr(Space.FRAME, -4)))
    assert not regions.may_alias(literal, _ref(Addr(Space.STACK, 0)))


def test_a_resume_entry_still_lowers_with_the_selector_a_value() -> None:
    """A body entered with ES already loaded gave the allocator a phantom.

    Construction resolves its placeholder to a reaching definition, and where
    none reaches it numbers one anyway -- version 1, indistinguishable from a
    real first definition. The reference then named a value no instruction
    writes, which the allocator tried to place and the spiller to slot:
    `2 bytes of frame are wanted and this body has no return to give them
    back at`, on a resume entry that cannot grow a frame.
    """
    from pathlib import Path

    from qbopt.rewrite import rewrite

    assert rewrite(Path("fixtures/omf/divmod-p-evt.obj").read_bytes(), dry_run=False)


def test_an_absolute_selector_is_not_dgroup() -> None:
    """Axiom 3. `DEF SEG = &HA000` then `POKE` is video memory, not a variable.

    Refused, every POKE aliased every static and the `DEF SEG` cell could not
    be forwarded to the access that needed it. The selector's value carries
    it: a singleton interval is a constant.
    """
    from qbopt.analysis.ranges import Interval

    selector = mir.Value(99, 0x100, variable=7, version=1)
    poke = _ref(Addr(Space.FAR, 0, 0, Register.BX, Register.ES), width=1, segment=selector)
    static = _ref(Addr(Space.SEGMENT, 0x10, 5))
    known = {selector: Interval(0xA000, 0xA000, 2)}

    assert regions.may_alias(poke, static), "with no value for the selector it could be anywhere"
    assert not regions.may_alias(poke, static, None, known), "0xa000 is not where the linker put b$seg"
    assert regions.may_alias(poke, poke, None, known, known), "the same segment meets itself"

    other = replace(poke, segment=mir.Value(98, 0x90, variable=7, version=2))
    assert not regions.may_alias(poke, other, None, known, {other.segment: Interval(0xB800, 0xB800, 2)})


def test_a_call_stores_its_return_address_and_its_callees_writes() -> None:
    """Two stores, because one reference cannot say both.

    Named nowhere, the return address reached every cell and every far call
    dropped every memory fact. Named STACK, the callee's writes went with it:
    a user SUB, B$INKY and B$DDIM all wrote only the stack, and a static was
    forwarded across the SUB that assigns it.
    """
    from pathlib import Path

    from tests import corpus
    from qbopt.abi import runtime
    from qbopt.frontend import blocks as split
    from qbopt.frontend.blocks import code_map

    found = corpus.loaded(Path("fixtures/regressions/qbdemo-fil2.obj"))
    assert found is not None
    mapped = code_map(found)
    assert not isinstance(mapped, str)
    static = _ref(Addr(Space.SEGMENT, 0x10, 5))
    sub = 0
    for _name, body in mir.bodies(found, split.partition(found, mapped), runtime.for_module(found)):
        for block in body.blocks:
            for op in block.ops:
                if op.kind is not mir.Kind.CALL:
                    continue
                assert any(ref.space is Space.STACK for ref in op.stores), f"{op.at:#06x} has no return address"
                if found.calls.get(op.at) == "UPDPALPLASMA":
                    sub += 1
                    assert any(mir.overlapping(ref, static, frozenset()) for ref in op.stores), (
                        f"{op.at:#06x}: a SUB call writes nothing"
                    )
    assert sub


def test_a_forwarded_load_takes_its_fixup_with_it() -> None:
    """`add ax,[d]` served from a register is `add ax,bx`, and the
    displacement the fixup named is gone with the operand.

    The rule was written down for the several-fixup case and not the one
    with a single fixup, so thirteen objects said
    `add has 1 fixups and 0 fields to put them in` as soon as anything
    forwarded more.
    """
    from pathlib import Path

    from qbopt.rewrite import rewrite

    out, _regions = rewrite(Path("fixtures/omf/hotlop-p-evt.obj").read_bytes(), dry_run=False)
    assert len(out) > 2


def test_the_sign_a_word_to_float_helper_leaves_in_dx_is_named_not_kept() -> None:
    """`B$FIL2` is a `cwd` falling into `B$FILD`.

    Its contract calls `dx` a clobber, so wherever BC read `dx` afterwards
    the call could not be raised to a load: 18 of qbdemo's 31 stayed calls,
    each a barrier to everything around it. `dx` is the sign of the word the
    call was handed, and saying so serves the read. No corpus object has
    both the emulator symbol and the read; this one is qbdemo's.
    """
    from pathlib import Path

    from iced_x86 import Register as Reg

    from tests import corpus
    from qbopt.abi import runtime
    from qbopt.frontend import blocks as split
    from qbopt.frontend.blocks import code_map

    found = corpus.loaded(Path("fixtures/regressions/qbdemo-fil2.obj"))
    assert found is not None
    mapped = code_map(found)
    assert not isinstance(mapped, str)
    named = 0
    raised = mir.bodies(found, split.partition(found, mapped), runtime.for_module(found))
    for _name, body in raised:
        hints = raised.hints[body.entry]
        read = {one for block in body.blocks for op in block.ops for one in op.uses}
        read.update(one for block in body.blocks for phi in block.phis for one in phi.incoming.values())
        for block in body.blocks:
            for index, op in enumerate(block.ops):
                if op.kind is mir.Kind.CALL and found.calls.get(op.at) == "B$FIL2":
                    kept = [one for one in op.defines if one in read]
                    assert not (kept and all(not one.flags and hints.origin_of(one) == Reg.EDX for one in kept)), (
                        f"{op.at:#06x}: kept as a call for a dx it could have named"
                    )
                if (
                    op.kind is mir.Kind.EXTRACT
                    and index + 1 < len(block.ops)
                    and block.ops[index + 1].kind is mir.Kind.FLOAD
                    and block.ops[index + 1].at == op.at
                ):
                    named += 1
    assert named, "the sign is named at the load it came from"


def test_a_rewritten_object_keeps_every_far_call_relocation() -> None:
    """A call's fixup is in its target, which is not an operand.

    Dropping fixups that had no operand to sit in took the far calls with
    them: the code bytes were identical, every `call` went out as `call 0:0`,
    qbdemo never finished, and the corpus read 25,560 bytes smaller.
    """
    from pathlib import Path

    from iced_x86 import Decoder
    from iced_x86 import Mnemonic

    from qbopt.objectfile import omf
    from qbopt.rewrite import rewrite

    records = omf.parse(rewrite(Path("fixtures/omf/jumptable.obj").read_bytes(), dry_run=False)[0])
    seg, _name, size = omf.code_segment(records)
    code = omf.segment_image(records, seg, size)
    relocated = {one.offset for one in omf.fixups(records) if one.seg == seg and one.loc == omf.LOC_PTR32}
    far = [insn.ip for insn in Decoder(16, code) if insn.mnemonic == Mnemonic.CALL and code[insn.ip] == 0x9A]
    assert far, "the fixture calls the runtime"
    assert [at for at in far if at + 1 not in relocated] == []


def test_a_bounded_calls_return_address_does_not_end_a_local() -> None:
    """The return address is stored below sp, where the callee's own pushes go.

    Given no bound of its own, it met every frame slot, so FRACLINE's
    `L24 := 160` stopped folding across the B$FCMP in its loop and the add
    at 0x059d read the slot again.
    """
    from pathlib import Path

    from tests import corpus
    from qbopt.abi import runtime
    from qbopt.analysis import consts

    path = Path("fixtures/regressions/qbdemo-fil2.obj")
    found = corpus.loaded(path)
    body = next(
        body
        for name, body in mir.bodies(found, corpus.partitioned(path), runtime.for_module(found))
        if name == "procedure FRACLINE"
    )
    known = consts.known(body, found.dgroup, found.calls)
    held = consts.cells(body, found.dgroup, found.calls, known)
    block, index, op = next(
        (block, index, op) for block in body.blocks for index, op in enumerate(block.ops) if op.at == 0x059D
    )
    fact = consts._cell(held[(block.at, index)], op.loads[0])
    assert fact is not None and fact.n == 160


def test_an_extern_is_in_no_segment_this_object_owns() -> None:
    """PLASMA's `totalframecount = totalframecount + 1` ended b$seg's value.

    An EXTDEF names bytes another object contributes. PUBLIC-combined
    BC_DATA is this object's alone; only a COMMON-combined segment is laid
    over another object's bytes, so only there can the two meet.
    """
    from qbopt.objectfile import module

    layout = module.Group((4, 5), shared=(4,))
    bseg = _ref(Addr(Space.EXTERNAL, 0, 3))
    other = _ref(Addr(Space.EXTERNAL, 0, 4))
    static = _ref(Addr(Space.SEGMENT, 0x12, 5))
    common = _ref(Addr(Space.SEGMENT, 0x12, 4))

    assert not regions.may_alias(bseg, static, layout=layout)
    assert regions.may_alias(bseg, common, layout=layout)
    assert regions.may_alias(bseg, static), "without the layout any segment may be overlaid"
    assert regions.may_alias(bseg, other, layout=layout), "two symbols cannot be compared"

    spared = _ref(None, width=4, excludes=((Addr(Space.EXTERNAL, -(1 << 15), 3), 1 << 16),))
    assert not regions.may_alias(bseg, spared)
    assert regions.may_alias(other, spared), "the exclusion names one symbol"


def test_a_named_far_objects_selector_reaches_neither_stack_nor_dgroup() -> None:
    """qcport's `reached[i] = 0` over a `static short far` array: the store's
    selector is the array's own segment, yet it aliased every local and static,
    so the bound and the counter were reloaded around it and the loop never
    became a fill -- bcc's `rep stosw`."""
    offset, selector = mir.Value(1, 1), mir.Value(2, 1)
    named = _ref(Addr(Space.FAR, 0, 5), base=offset, segment=selector, space=Space.FAR, base_width=2)
    loaded = _ref(Addr(Space.FAR, 0), base=offset, segment=selector, space=Space.FAR, base_width=2)
    other = _ref(Addr(Space.FAR, 4, 6), base=selector, segment=offset, space=Space.FAR, base_width=2)
    local = _ref(Addr(Space.FRAME, -0x58), space=Space.FRAME)
    static = _ref(Addr(Space.SEGMENT, 0x12, 7), space=Space.SEGMENT)

    assert not regions.may_alias(named, local)
    assert not regions.may_alias(named, static)
    assert regions.may_alias(named, loaded), "a loaded far pointer may point anywhere"
    assert regions.may_alias(named, other), "two far objects may share a segment"
    assert regions.may_alias(loaded, local)


def test_a_declared_scalar_is_reached_only_through_its_own_type() -> None:
    """snd_mix_frame's `snd_paint[k] = 0` reloaded `int far *snd_paint` on every
    pass, so the loop never became bcc's `rep stosw`: an `int` store cannot
    change a declared pointer, but only a declared object says so -- two
    accesses of different types may be one union's members."""
    offset, selector = mir.Value(1, 1), mir.Value(2, 1)
    pointer = _ref(Addr(Space.SEGMENT, 0x18, 7), space=Space.SEGMENT, typed=("pointer4", True))
    store = _ref(Addr(Space.FAR, 0), base=offset, segment=selector, space=Space.FAR, base_width=2, typed=("int2", False))
    other = _ref(Addr(Space.FAR, 0), base=offset, segment=selector, space=Space.FAR, base_width=2, typed=("pointer4", False))
    chars = _ref(Addr(Space.FAR, 0), base=offset, segment=selector, space=Space.FAR, base_width=2)
    member = _ref(Addr(Space.FRAME, -8), width=4, space=Space.FRAME, typed=("float4", False))
    punned = _ref(Addr(Space.FRAME, -8), width=4, space=Space.FRAME, typed=("int4", False))

    assert not regions.may_alias(store, pointer)
    assert regions.may_alias(other, pointer)
    assert regions.may_alias(chars, pointer), "a char or untyped access reaches anything"
    assert regions.may_alias(member, punned), "neither is a declared object"
