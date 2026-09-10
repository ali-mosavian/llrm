"""
qbopt/backend/layout.py: a whole body emitted, and everything that moves with it.

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
from iced_x86 import Mnemonic
from iced_x86 import Instruction

import corpus
from qbopt.model import ir
from qbopt.backend import asm
from qbopt.model import mir
from qbopt.objectfile import omf
from qbopt.backend import layout
from qbopt.backend import select
from qbopt.frontend.declen import decode
from qbopt.frontend import blocks as split
from qbopt.frontend.blocks import code_map

FIXTURES = sorted(Path("fixtures/omf").glob("*.obj"))


@pytest.mark.parametrize("conditional", [False, True])
def test_reordered_block_materializes_its_cfg_fallthrough(conditional):
    """Peeled IVARM's latch fell into its own header instead of the next iteration."""
    what = ir.Semantics(ir.Operation.BRANCH, "jne", target=30) if conditional else ir.Semantics(
        ir.Operation.NOTHING, "nop")
    op = mir.Op(10, what.op, what.name, (), (), made=what, covers=(10, 11))
    body = mir.MirBody(10, (
        mir.MirBlock(10, (), (op,), (30, 40) if conditional else (40,)),
        mir.MirBlock(20, (), (), ()),
        mir.MirBlock(30, (), (), ()),
        mir.MirBlock(40, (), (), ()),
    ))
    changed = layout._fallthroughs(body)
    jump = changed.block(10).ops[-1]
    assert jump.made == ir.Semantics(ir.Operation.JUMP, "jmp", target=40)
    assert jump.covers == (10, 10)
    assert jump.node is None and jump.symbol is False
    assert layout._fallthroughs(changed) == changed


def test_empty_reordered_block_gets_its_own_jump_anchor():
    body = mir.MirBody(10, (mir.MirBlock(10, (), (), (30,)),
                            mir.MirBlock(20, (), (), ()), mir.MirBlock(30, (), (), ())))
    changed = layout._fallthroughs(body)
    assert layout._anchors(changed)[10] is changed.block(10).ops[0]
    assert changed.block(10).ops[0].made.target == 30


@pytest.mark.parametrize("tag", ["q-O", "p-g2", "v-g3"])
def test_peeled_ivarm_emission_keeps_every_iteration_reachable(monkeypatch, tag):
    """PDS's peeled latch reentered itself, leaving emitted bytes 0x57..0x70 unreachable."""
    from qbopt import wholeseg
    from qbopt.analysis import loops
    from qbopt.objectfile import module
    from qbopt.optimize import lcssa, loopclone, transform

    original = transform.applied

    def candidate(body, *args, **kwargs):
        kwargs["unswitch_"] = False
        body = lcssa.closed(original(body, *args, **kwargs))
        loop, = loops.loops(body.blocks, body.entry)
        changed = loopclone.peeled(body, loop, 2)
        assert changed is not None
        return changed

    monkeypatch.setattr(transform, "applied", candidate)
    result = wholeseg.emitted(Path(f"fixtures/regressions/ivarm-{tag}.obj").read_bytes())
    assert result.outcome is wholeseg.Emission.LIR, result.reason
    found = module.of(omf.parse(result.data))
    mapped = code_map(found)
    assert not isinstance(mapped, str), mapped


def test_pressx_has_no_jump_to_the_following_instruction():
    """PRESSX retained an unconditional jump to its exit immediately after loop elimination."""
    from qbopt import wholeseg
    result = wholeseg.emitted(Path("fixtures/omf/pressx-p-g2.obj").read_bytes())
    assert result.outcome is wholeseg.Emission.LIR, result.reason
    for block in corpus.partitioned(result.data):
        for one in block.insns:
            insn = one.insn
            if insn.mnemonic == Mnemonic.JMP and insn.op0_kind in (OpKind.NEAR_BRANCH16, OpKind.NEAR_BRANCH32):
                assert insn.near_branch_target != insn.next_ip


@pytest.mark.parametrize("shape", ["next", "chain", "self", "data"])
def test_fallthrough_relaxation_preserves_targets_and_intervening_data(shape):
    """Removing PRESSX's empty exit jump must not remove a loop or execute skipped data."""
    from types import SimpleNamespace
    def jump(at, to):
        return mir.Op(at, ir.Operation.JUMP, "jmp", (), (), (), (), None,
                      made=ir.Semantics(ir.Operation.JUMP, "jmp", target=to), covers=(at, at + 2))
    def ret(at):
        return mir.Op(at, ir.Operation.RETURN, "ret", (), (), (), (), None,
                      made=ir.Semantics(ir.Operation.RETURN, "ret"), covers=(at, at + 1))
    ops = [jump(0, 2), ret(2)]
    code = bytes.fromhex("eb00c3")
    if shape == "chain":
        ops, code = [jump(0, 2), jump(2, 4), ret(4)], bytes.fromhex("eb00eb00c3")
    elif shape == "self":
        ops = [jump(0, 0), ret(2)]
    elif shape == "data":
        ops, code = [jump(0, 3), asm.Table(2, 3), ret(3)], bytes.fromhex("eb0190c3")
    found = SimpleNamespace(code=code, absorbed={}, fixup_at={}, calls={}, refs={}, float_protocols={})
    result = asm.assemble(ops, 0, found)
    assert not isinstance(result, str), result
    if shape in {"next", "chain"}:
        assert result.code == bytes.fromhex("c3")
    elif shape == "self":
        assert result.code == bytes.fromhex("ebfec3")
    else:
        assert result.code == bytes.fromhex("eb0190c3")


def test_emulator_load_uses_the_allocated_address() -> None:
    """nbody's copied FLD still read SI after allocation moved its pointer."""
    from types import SimpleNamespace
    from iced_x86 import Register
    from qbopt.frontend import declen

    raw = bytes.fromhex("cd3504")
    original = ir.Semantics(ir.Operation.FLOAT_LOAD, "fld", (ir.St(0),),
                            (ir.Mem(None, 4, through=Register.SI),))
    changed = ir.Semantics(ir.Operation.FLOAT_LOAD, "fld", (ir.St(0),),
                           (ir.Mem(None, 4, through=Register.DI),))
    op = mir.Op(0, ir.Operation.FLOAT_LOAD, "fld", (), (), (), (),
                ir.Opaque(declen.decode(raw, 0), ir.NO_EFFECT, original),
                made=changed, covers=(0, 3), kind=mir.Kind.FLOAD)
    found = SimpleNamespace(code=raw, absorbed={}, fixup_at={}, calls={}, refs={}, float_protocols={})
    done = asm.assemble([op], 0, found)
    assert not isinstance(done, str), done
    assert done.code == bytes.fromhex("cd3505")


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


def original(op: mir.Op) -> "Instruction | None":
    """The instruction an op came from, or None where it came from none.

    An idiom is several instructions behind one node and there is no one
    of them this could name: the restore that hands a long back to BC's
    halves is three, and an operation the raise made to say what a site
    hands back has no original instruction at all. Every caller here is
    comparing an op against the instruction it was raised from, so those
    are skipped rather than asserted about.
    """
    node = op.node
    assert node is not None
    if isinstance(node, ir.Restore):
        return None
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
    out, at = [], 0
    for op in ops:
        folded = found.absorbed.get(getattr(op, "id", None)) if found is not None else None
        made = None
        if folded is not None:
            made = asm._absorbed(*folded)
            assert made is not None, f"{op.at:#x}: the absorbed call emits nothing"
        elif isinstance(getattr(op, "node", None), ir.Restore):
            # An operation is not always an instruction the other way
            # round either: the idiom that hands a long back to BC's two
            # halves is `push eax / pop ax / pop dx` behind one node, and
            # counting one left its other two belonging to nothing.
            made = select.restore(op.node.pair)
            assert made is not None, f"{op.at:#x}: the restore idiom emits nothing"
        if made is None:
            out.append((op, back[at : at + 1]))
            at += 1
            continue
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
            if want is None:
                continue  # an idiom is not the instruction it came from
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
            if want is None or want.op0_kind != OpKind.NEAR_BRANCH16:
                continue
            landed = got.moved.get(want.near_branch16)
            assert landed is not None, f"{obj.stem} {op.at:#x}: target left this body"
            assert made[0].near_branch16 == landed, f"{obj.stem} {op.at:#x}: {made[0]} should reach {landed:#x}"


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
            if want is None or want.op0_kind != OpKind.NEAR_BRANCH16:
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
        for body, got, found in laid(obj):
            was += sum(asm._length_of(op, found) or 0 for op in layout._ordered(body))
            now += len(got.code)
    assert now <= was, f"{now} against BC's {was}"


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
    from qbopt.model import mir
    from qbopt.objectfile import omf
    from qbopt.objectfile import module
    from qbopt.optimize import transform
    from qbopt.frontend import blocks as split
    from qbopt.frontend.blocks import code_map

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

    from qbopt.model import ir
    from qbopt.model import mir
    from qbopt.objectfile import omf
    from qbopt.objectfile import module
    from qbopt.frontend.blocks import code_map

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


def test_a_relocation_belongs_to_the_operand_and_not_to_a_place(obj: Path) -> None:
    """Which fixup an operation carries is settled at the raise.

    It used to be found by searching BC's own byte span for one, every time
    layout asked -- so an operation could only be relocated correctly while
    it still stood where BC wrote it, and `covers` had to keep saying which
    bytes it stood for. A relocation belongs to an operand.

    Checked as: the answer from the operation matches the answer the search
    gave, on every operation in the corpus that has one.
    """
    from qbopt.model import ir
    from qbopt.model import mir
    from qbopt.frontend import blocks as split
    from qbopt.frontend.blocks import code_map

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
                # Ordinarily the site's own start..end, one interval that
                # already covers every push. A site frames() found may
                # have pushed its arguments apart from its call -- lngmix
                # re-pushes v and 7 for its second divide, with a real
                # store between the pushes and the call -- and there
                # found.coverage carries every range the fold actually
                # stands for, pushes included.
                ranges = found.coverage.get(op.id, ((site.start, site.end),))
                assert all(any(lo <= one < hi for lo, hi in ranges) for one in said), (
                    f"{obj.stem} {op.at:#06x}: {said} is outside {ranges}"
                )
                seen += 1
                continue
            ref = said[0] if said else None
            assert said is None or len(said) == 1, f"{obj.stem} {op.at:#06x}: {said}"
            assert ref == want, f"{obj.stem} {op.at:#06x}: says {ref}, the bytes say {want}"
            seen += ref is not None
    assert seen or not fields, f"{obj.stem}: nothing carries a fixup, so this proves nothing"


def test_a_tangled_class_is_split_on_the_phi_edge() -> None:
    """A value arriving at a phi and still wanted after it is the lost-copy
    shape: the phi's result and its own argument are one congruence class
    and are live at the same moment, so no single register holds both and
    `colour` refuses to move the class. `untangled` puts `v' = v` at the
    end of the predecessor and the phi takes `v'`, which is the live-range
    split done where the split belongs.

    Constructed, because BC's own code never has one -- measured, zero
    across the corpus -- and a tangle only appears once a pass has moved a
    definition. This checks the three functions directly; that
    `layout.rebuild` calls them is not asserted here.
    """
    from dataclasses import replace

    from iced_x86 import Register

    from qbopt.model import ir
    from qbopt.model import mir
    from qbopt.legacy import regalloc

    made, carried = mir.Value(1, 0x10, 0, 1, 1), mir.Value(2, 0x20, 0, 1, 2)
    other = mir.Value(5, 0x11, 0, 4, 1)

    def op(at, kind, defines, uses, args, results):
        return mir.Op(
            at,
            ir.Operation.MOVE,
            "mov",
            defines=defines,
            uses=uses,
            loads=(),
            stores=(),
            node=None,
            kind=kind,
            args=args,
            results=results,
            covers=(at, at + 3),
        )

    # `made` is defined before the join, arrives at the phi, and is read
    # again after it -- so it overlaps the phi's own result.
    body = mir.MirBody(
        0x10,
        (
            mir.MirBlock(
                0x10,
                (),
                (
                    op(0x10, mir.Kind.COPY, (made,), (), (mir.Const(1, 2),), (mir.Held(made, 2),)),
                    op(0x13, mir.Kind.COPY, (other,), (), (mir.Const(2, 2),), (mir.Held(other, 2),)),
                ),
                (0x20,),
            ),
            mir.MirBlock(
                0x20,
                (mir.Phi(carried, {0x10: made}),),
                (
                    op(
                        0x20,
                        mir.Kind.COPY,
                        (mir.Value(3, 0x20, 0, 2, 1),),
                        (carried,),
                        (mir.Held(carried, 2),),
                        (mir.Held(mir.Value(3, 0x20, 0, 2, 1), 2),),
                    ),
                    op(
                        0x23,
                        mir.Kind.COPY,
                        (mir.Value(4, 0x23, 0, 3, 1),),
                        (made,),
                        (mir.Held(made, 2),),
                        (mir.Held(mir.Value(4, 0x23, 0, 3, 1), 2),),
                    ),
                    op(
                        0x26,
                        mir.Kind.COPY,
                        (mir.Value(6, 0x26, 0, 5, 1),),
                        (other,),
                        (mir.Held(other, 2),),
                        (mir.Held(mir.Value(6, 0x26, 0, 5, 1), 2),),
                    ),
                ),
                (),
            ),
        ),
        # The class starts in ax, and `other` is held there too -- so the
        # class has to move, and while it is tangled nothing can move it.
        {made: Register.EAX, carried: Register.EAX, other: Register.EAX},
        {},
    )

    body = replace(body, pins={other: Register.EAX})
    tangled = regalloc._tangled(body)
    assert tangled, "the constructed body has no overlapping class; it witnesses nothing"
    assert isinstance(regalloc.colour(body, body.pins), str), "it coloured while tangled"

    fixed = regalloc.untangled(body)
    assert not isinstance(regalloc.colour(fixed, fixed.pins), str), "it does not colour after"
    # The copy goes on the edge, in the predecessor -- not anywhere that
    # would make this pass through an unrelated rewrite.
    before = next(block for block in fixed.blocks if block.at == 0x10)
    assert len(before.ops) == len(body.blocks[0].ops) + 1, "no copy was put on the edge"
    added = before.ops[-1]
    assert added.kind is mir.Kind.COPY and made.id in {one.id for one in added.uses}, (
        f"the copy on the edge reads {added.uses}"
    )
    was = {one.id for block in body.blocks for op in block.ops for one in (*op.defines, *op.uses)}
    assert {one.id for one in added.defines}.isdisjoint(was), "the edge copy reuses an existing value"
    join = next(block for block in fixed.blocks if block.at == 0x20)
    assert {one.id for phi in join.phis for one in phi.incoming.values()} == {one.id for one in added.defines}, (
        "the phi does not take the copy"
    )


def test_a_moved_operation_keeps_its_fixup() -> None:
    """Why the side table is keyed by the operation and not by its address.

    `ref` used to sit on the op, which is what made it survive the hoist
    re-seating something. Off the op it has to be keyed by an identity that
    moves with the operation; keyed by `at` the relocation stays behind and
    the address comes out a bare zero.
    """
    from dataclasses import replace

    from qbopt.model import mir
    from qbopt.objectfile import omf
    from qbopt.objectfile import module
    from qbopt.frontend import blocks as split
    from qbopt.frontend.blocks import code_map

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
    from qbopt.model import ir
    from qbopt.model import mir
    from qbopt.objectfile import omf
    from qbopt.objectfile import module
    from qbopt.optimize import transform
    from qbopt.frontend import blocks as split
    from qbopt.frontend.blocks import code_map

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
                if was is None or not any(isinstance(one, (ir.Mem, ir.Address)) for one in (*was.dests, *was.sources)):
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
    from qbopt.backend import select

    # However it was built: _assemble reads the two offsets iced gives back,
    # an idiom says where its own fields are, and both answer `places`.
    plain = select.Emitted(b"\x8b\x06\x00\x00", displacement_at=2)
    assert plain.places == (2,) and plain.relocated_at == 2

    idiom = select.Emitted(b"\x66\xa1\x00\x00\x66\xb9\x00\x00", fields=(2, 6))
    assert idiom.places == (2, 6), idiom.places
    assert idiom.relocated_at == 2, "the first is what a caller with one wants"

    silent = select.Emitted(b"\x99")
    assert silent.places == () and silent.relocated_at is None
